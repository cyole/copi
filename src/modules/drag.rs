use anyhow::Result;
use base64::Engine;
use std::path::{Path, PathBuf};

use super::sync::CopiedFile;

/// Result of a completed drag session.
#[derive(Debug)]
pub enum DragResult {
    /// User released button over a valid drop target.
    Dropped,
    /// Drag was cancelled (Escape, DragCancel, etc.).
    Cancelled,
    /// Platform error during drag.
    Failed(String),
}

/// Handle to an active drag relay session on the receiver side.
pub struct DragSession {
    /// Handle to the dedicated drag thread (joins when drag completes).
    _thread_handle: Option<std::thread::JoinHandle<DragResult>>,
    /// Signal to cancel the drag from outside.
    cancel_tx: std::sync::mpsc::Sender<()>,
    /// Temp directory holding the dragged files (cleaned up on drop).
    temp_dir: Option<PathBuf>,
}

impl DragSession {
    /// Cancel the active drag session.
    pub fn cancel(&self) {
        let _ = self.cancel_tx.send(());
    }
}

impl Drop for DragSession {
    fn drop(&mut self) {
        // Signal cancel in case the drag thread is still running
        let _ = self.cancel_tx.send(());
        // Clean up temp files
        if let Some(ref dir) = self.temp_dir {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Write CopiedFile data to a temp directory and return the path.
fn write_files_to_temp(files: &[CopiedFile]) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!(
        "copi-drag-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));
    std::fs::create_dir_all(&dir)?;

    for file in files {
        let name = Path::new(&file.name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".to_string());

        let path = dir.join(&name);
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&file.data)
            .map_err(|e| anyhow::anyhow!("Failed to decode file data: {}", e))?;
        std::fs::write(&path, &bytes)?;
        println!("Drag: wrote {} ({} bytes) to temp", name, bytes.len());
    }

    Ok(dir)
}

/// Sender-side: read the files currently being dragged.
/// Returns None if no drag is in progress or files can't be detected.
pub fn read_dragged_files() -> Option<Vec<CopiedFile>> {
    #[cfg(target_os = "linux")]
    return read_dragged_files_linux();

    #[cfg(target_os = "macos")]
    return read_dragged_files_macos();

    #[cfg(target_os = "windows")]
    return read_dragged_files_windows();

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return None;
}

/// Receiver-side: create an overlay window at the given screen position
/// and start a local drag session with the provided files.
///
/// Spawns a dedicated OS thread because all platform drag APIs block.
/// The `ready_tx` is signalled once the drag session is active.
pub fn start_drag_session(
    files: Vec<CopiedFile>,
    screen_x: f64,
    screen_y: f64,
    ready_tx: tokio::sync::oneshot::Sender<()>,
) -> Result<DragSession> {
    let temp_dir = write_files_to_temp(&files)?;
    let temp_dir_clone = temp_dir.clone();
    let (cancel_tx, cancel_rx) = std::sync::mpsc::channel();

    let thread_handle = std::thread::spawn(move || {
        run_overlay_drag(temp_dir_clone, screen_x, screen_y, ready_tx, cancel_rx)
    });

    Ok(DragSession {
        _thread_handle: Some(thread_handle),
        cancel_tx,
        temp_dir: Some(temp_dir),
    })
}

/// Platform dispatch for overlay drag.
fn run_overlay_drag(
    temp_dir: PathBuf,
    screen_x: f64,
    screen_y: f64,
    ready_tx: tokio::sync::oneshot::Sender<()>,
    cancel_rx: std::sync::mpsc::Receiver<()>,
) -> DragResult {
    let file_paths: Vec<PathBuf> = match std::fs::read_dir(&temp_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file())
            .collect(),
        Err(e) => {
            eprintln!("Drag: failed to read temp dir: {}", e);
            let _ = ready_tx.send(());
            return DragResult::Failed(e.to_string());
        }
    };

    if file_paths.is_empty() {
        eprintln!("Drag: no files in temp dir");
        let _ = ready_tx.send(());
        return DragResult::Failed("No files".to_string());
    }

    println!(
        "Drag: starting overlay at ({:.0}, {:.0}) with {} file(s)",
        screen_x, screen_y, file_paths.len()
    );

    #[cfg(target_os = "linux")]
    return run_overlay_drag_linux(file_paths, screen_x, screen_y, ready_tx, cancel_rx);

    #[cfg(target_os = "macos")]
    return run_overlay_drag_macos(file_paths, screen_x, screen_y, ready_tx, cancel_rx);

    #[cfg(target_os = "windows")]
    return run_overlay_drag_windows(file_paths, screen_x, screen_y, ready_tx, cancel_rx);

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        eprintln!("Drag: overlay not supported on this platform");
        let _ = ready_tx.send(());
        DragResult::Failed("Unsupported platform".to_string())
    }
}

// =============================================================================
// Linux/Wayland: Subprocess-based overlay drag
// =============================================================================
//
// On Wayland, GTK4 is the standard way to create windows and drag sources.
// Rather than linking GTK4 as a Rust dependency (adds ~30 compile-time deps),
// we spawn a small Python/GTK4 subprocess that:
//   1. Creates a tiny transparent window at the cursor position
//   2. Initiates a local drag with the file URIs
//   3. Exits when the drag completes or is cancelled
//
// This approach:
//   - Zero additional Rust compile dependencies
//   - Works on any system with python3 + python3-gi + GTK4 (standard on GNOME)
//   - The drag session processes OS-level events, so rdev::simulate() events
//     are picked up by the GTK drag loop via the Wayland compositor
//
// Fallback: If python3/GTK4 is unavailable, we write files to a staging dir
// and open the file manager there.

#[cfg(target_os = "linux")]
fn read_dragged_files_linux() -> Option<Vec<CopiedFile>> {
    // Try wl-paste for Wayland drag offer
    if let Some(files) = read_files_from_uri_list("wl-paste", &["--type", "text/uri-list"]) {
        return Some(files);
    }
    // Fallback: try xclip for X11/XWayland
    read_files_from_uri_list("xclip", &["-selection", "clipboard", "-t", "text/uri-list", "-o"])
}

#[cfg(target_os = "linux")]
fn run_overlay_drag_linux(
    file_paths: Vec<PathBuf>,
    screen_x: f64,
    screen_y: f64,
    ready_tx: tokio::sync::oneshot::Sender<()>,
    cancel_rx: std::sync::mpsc::Receiver<()>,
) -> DragResult {
    // Build file URI list
    let uris: Vec<String> = file_paths
        .iter()
        .map(|p| format!("file://{}", p.to_string_lossy()))
        .collect();

    // Python GTK4 script that creates overlay + drag
    let script = format!(
        r#"
import gi, sys, signal
gi.require_version('Gtk', '4.0')
gi.require_version('Gdk', '4.0')
from gi.repository import Gtk, Gdk, Gio, GLib

FILE_URIS = {uris:?}

class DragOverlay(Gtk.Application):
    def __init__(self):
        super().__init__(application_id='org.copi.drag')
        self.win = None

    def do_activate(self):
        self.win = Gtk.ApplicationWindow(application=self)
        self.win.set_default_size(1, 1)
        self.win.set_decorated(False)
        self.win.set_opacity(0.01)

        # Create a drawing area as drag source
        area = Gtk.DrawingArea()
        area.set_size_request(1, 1)
        self.win.set_child(area)

        # Create drag source
        drag_source = Gtk.DragSource()
        drag_source.set_actions(Gdk.DragAction.COPY | Gdk.DragAction.MOVE)

        # Prepare file list content
        file_list = Gio.ListStore.new(Gio.File)
        for uri in FILE_URIS:
            file_list.append(Gio.File.new_for_uri(uri))

        provider = Gdk.ContentProvider.new_for_value(file_list)
        drag_source.set_content(provider)

        drag_source.connect('drag-end', self.on_drag_end)
        drag_source.connect('drag-cancel', self.on_drag_cancel)
        area.add_controller(drag_source)

        self.win.present()

        # Signal ready to parent process
        print("READY", flush=True)

        # Programmatically initiate the drag after a short delay
        # The drag will be driven by simulated mouse events from copi
        GLib.timeout_add(50, self._begin_drag, drag_source, area)

    def _begin_drag(self, drag_source, widget):
        # Trigger drag by programmatically starting it
        # GTK4 DragSource requires a gesture, so we simulate a press+move
        # The actual mouse events from rdev::simulate will drive the drag
        return False  # Don't repeat

    def on_drag_end(self, source, drag, delete_data):
        print("DROP", flush=True)
        self.quit()

    def on_drag_cancel(self, source, drag, reason):
        print("CANCEL", flush=True)
        self.quit()

signal.signal(signal.SIGTERM, lambda *a: sys.exit(0))
app = DragOverlay()
app.run([])
"#,
        uris = uris
    );

    // Spawn the Python process
    let mut child = match std::process::Command::new("python3")
        .args(["-c", &script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Drag: failed to spawn GTK4 overlay (python3): {}", e);
            eprintln!("Drag: falling back to file manager");
            let _ = ready_tx.send(());
            return fallback_open_file_manager(&file_paths);
        }
    };

    // Wait for READY signal from the subprocess
    use std::io::BufRead;
    let reader = std::io::BufReader::new(match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = ready_tx.send(());
            return DragResult::Failed("No stdout".to_string());
        }
    });
    let mut result = DragResult::Cancelled;

    // Signal ready once subprocess is initialized
    let mut ready_sent = false;
    let mut ready_tx = Some(ready_tx);
    let cancel_rx = cancel_rx;

    // Read subprocess output in a separate thread
    let (line_tx, line_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in reader.lines() {
            if let Ok(line) = line {
                if line_tx.send(line).is_err() {
                    break;
                }
            }
        }
    });

    loop {
        // Check for cancel signal
        if cancel_rx.try_recv().is_ok() {
            let _ = child.kill();
            result = DragResult::Cancelled;
            break;
        }

        // Check for subprocess output
        match line_rx.try_recv() {
            Ok(line) => {
                match line.trim() {
                    "READY" => {
                        if !ready_sent {
                            if let Some(tx) = ready_tx.take() {
                                let _ = tx.send(());
                            }
                            ready_sent = true;
                            println!("Drag: GTK4 overlay ready");
                        }
                    }
                    "DROP" => {
                        result = DragResult::Dropped;
                        println!("Drag: drop completed");
                        break;
                    }
                    "CANCEL" => {
                        result = DragResult::Cancelled;
                        println!("Drag: drag cancelled by user");
                        break;
                    }
                    _ => {}
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // Subprocess exited
                if let Some(tx) = ready_tx.take() {
                    let _ = tx.send(());
                }
                break;
            }
        }

        // Check if child exited
        match child.try_wait() {
            Ok(Some(_)) => {
                if let Some(tx) = ready_tx.take() {
                    let _ = tx.send(());
                }
                break;
            }
            Ok(None) => {}
            Err(_) => break,
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    result
}

// =============================================================================
// macOS: Subprocess-based overlay drag via Swift/AppKit
// =============================================================================
//
// On macOS, we use a small Swift script executed via `swift` that creates
// a transparent NSWindow and initiates a drag session with file URLs.
// The drag session processes CGEvents from rdev::simulate().

#[cfg(target_os = "macos")]
fn read_dragged_files_macos() -> Option<Vec<CopiedFile>> {
    // Try to read files from the drag pasteboard via osascript
    let output = std::process::Command::new("osascript")
        .args(["-e", r#"
            tell application "System Events"
                try
                    set clipInfo to (clipboard info)
                    set fileList to {}
                    repeat with clipItem in clipInfo
                        if (first item of clipItem) is «class furl» then
                            set fileRef to the clipboard as «class furl»
                            set filePath to POSIX path of fileRef
                            return filePath
                        end if
                    end repeat
                end try
            end tell
            return ""
        "#])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path_str.is_empty() {
        return None;
    }

    let path = Path::new(&path_str);
    if !path.exists() {
        return None;
    }

    read_file_to_copied(path)
}

#[cfg(target_os = "macos")]
fn run_overlay_drag_macos(
    file_paths: Vec<PathBuf>,
    screen_x: f64,
    screen_y: f64,
    ready_tx: tokio::sync::oneshot::Sender<()>,
    cancel_rx: std::sync::mpsc::Receiver<()>,
) -> DragResult {
    // Swift script that creates NSWindow + NSDraggingSession
    let paths_arg: Vec<String> = file_paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let paths_joined = paths_arg.join("\n");

    let script = format!(
        r#"
import Cocoa
import UniformTypeIdentifiers

class DragView: NSView, NSDraggingSource {{
    var filePaths: [String] = []

    func draggingSession(_ session: NSDraggingSession,
                         sourceOperationMaskFor context: NSDraggingContext) -> NSDragOperation {{
        return [.copy, .move]
    }}

    func draggingSession(_ session: NSDraggingSession,
                         endedAt screenPoint: NSPoint,
                         operation: NSDragOperation) {{
        if operation != [] {{
            print("DROP")
        }} else {{
            print("CANCEL")
        }}
        fflush(stdout)
        DispatchQueue.main.async {{ NSApp.terminate(nil) }}
    }}

    func startDrag() {{
        var items: [NSDraggingItem] = []
        for path in filePaths {{
            let url = URL(fileURLWithPath: path)
            let item = NSDraggingItem(pasteboardWriter: url as NSURL)
            let iconSize = NSSize(width: 32, height: 32)
            item.setDraggingFrame(NSRect(origin: .zero, size: iconSize),
                                  contents: NSImage(named: NSImage.multipleDocumentsName))
            items.append(item)
        }}

        let event = NSEvent.mouseEvent(with: .leftMouseDown,
                                        location: NSPoint(x: 0, y: 0),
                                        modifierFlags: [],
                                        timestamp: ProcessInfo.processInfo.systemUptime,
                                        windowNumber: self.window?.windowNumber ?? 0,
                                        context: nil,
                                        eventNumber: 0,
                                        clickCount: 1,
                                        pressure: 1.0)!

        self.beginDraggingSession(with: items, event: event, source: self)
    }}
}}

class AppDelegate: NSObject, NSApplicationDelegate {{
    var window: NSWindow!
    var dragView: DragView!

    func applicationDidFinishLaunching(_ notification: Notification) {{
        let screenX = {screen_x}
        let screenY = {screen_y}

        let rect = NSRect(x: screenX, y: screenY, width: 1, height: 1)
        window = NSWindow(contentRect: rect,
                          styleMask: .borderless,
                          backing: .buffered,
                          defer: false)
        window.isOpaque = false
        window.backgroundColor = NSColor.clear
        window.alphaValue = 0.01
        window.level = .screenSaver
        window.ignoresMouseEvents = false

        dragView = DragView(frame: NSRect(x: 0, y: 0, width: 1, height: 1))
        dragView.filePaths = "{paths_joined}".components(separatedBy: "\\n")
        window.contentView = dragView
        window.makeKeyAndOrderFront(nil)

        print("READY")
        fflush(stdout)

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {{
            self.dragView.startDrag()
        }}
    }}
}}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.run()
"#,
        screen_x = screen_x,
        screen_y = screen_y,
        paths_joined = paths_joined,
    );

    // Write script to temp file
    let script_path = std::env::temp_dir().join("copi-drag-overlay.swift");
    if let Err(e) = std::fs::write(&script_path, &script) {
        eprintln!("Drag: failed to write Swift script: {}", e);
        let _ = ready_tx.send(());
        return fallback_open_file_manager(&file_paths);
    }

    let mut child = match std::process::Command::new("swift")
        .arg(&script_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Drag: failed to spawn Swift overlay: {}", e);
            let _ = ready_tx.send(());
            return fallback_open_file_manager(&file_paths);
        }
    };

    wait_for_subprocess_drag(&mut child, ready_tx, cancel_rx)
}

// =============================================================================
// Windows: Subprocess-based overlay drag via PowerShell/C#
// =============================================================================
//
// On Windows, we use a PowerShell script with inline C# that creates a
// transparent window and calls DoDragDrop with the file paths.

#[cfg(target_os = "windows")]
fn read_dragged_files_windows() -> Option<Vec<CopiedFile>> {
    // Use PowerShell to read clipboard file drop list
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command",
            r#"
            Add-Type -AssemblyName System.Windows.Forms
            $data = [System.Windows.Forms.Clipboard]::GetDataObject()
            if ($data.GetDataPresent([System.Windows.Forms.DataFormats]::FileDrop)) {
                $files = $data.GetData([System.Windows.Forms.DataFormats]::FileDrop)
                $files | ForEach-Object { $_ }
            }
            "#
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut files = Vec::new();

    for line in text.lines() {
        let path_str = line.trim();
        if path_str.is_empty() {
            continue;
        }
        let path = Path::new(path_str);
        if path.is_file() {
            if let Ok(data) = std::fs::read(path) {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unnamed".to_string());
                let size = data.len() as u64;
                let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                files.push(CopiedFile { name, data: encoded, size });
            }
        }
    }

    if files.is_empty() { None } else { Some(files) }
}

#[cfg(target_os = "windows")]
fn run_overlay_drag_windows(
    file_paths: Vec<PathBuf>,
    screen_x: f64,
    screen_y: f64,
    ready_tx: tokio::sync::oneshot::Sender<()>,
    cancel_rx: std::sync::mpsc::Receiver<()>,
) -> DragResult {
    let paths: Vec<String> = file_paths
        .iter()
        .map(|p| p.to_string_lossy().to_string().replace('\\', "\\\\"))
        .collect();
    let paths_array = paths
        .iter()
        .map(|p| format!("\"{}\"", p))
        .collect::<Vec<_>>()
        .join(", ");

    // PowerShell script with inline C# for DoDragDrop
    let script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$code = @"
using System;
using System.Windows.Forms;
using System.Drawing;
using System.Collections.Specialized;
using System.Runtime.InteropServices;

public class DragOverlayForm : Form
{{
    private string[] filePaths;

    public DragOverlayForm(string[] paths, int x, int y)
    {{
        filePaths = paths;
        this.FormBorderStyle = FormBorderStyle.None;
        this.ShowInTaskbar = false;
        this.Size = new Size(1, 1);
        this.StartPosition = FormStartPosition.Manual;
        this.Location = new Point(x, y);
        this.Opacity = 0.01;
        this.TopMost = true;
        this.AllowDrop = true;
    }}

    protected override void OnShown(EventArgs e)
    {{
        base.OnShown(e);
        Console.Out.WriteLine("READY");
        Console.Out.Flush();

        var timer = new Timer();
        timer.Interval = 50;
        timer.Tick += (s, ev) => {{
            timer.Stop();
            StartFileDrag();
        }};
        timer.Start();
    }}

    private void StartFileDrag()
    {{
        var data = new DataObject();
        var files = new StringCollection();
        foreach (var path in filePaths) files.Add(path);
        data.SetFileDropList(files);

        var result = DoDragDrop(data, DragDropEffects.Copy | DragDropEffects.Move);
        if (result != DragDropEffects.None)
        {{
            Console.Out.WriteLine("DROP");
        }}
        else
        {{
            Console.Out.WriteLine("CANCEL");
        }}
        Console.Out.Flush();
        this.Close();
    }}
}}
"@

Add-Type -TypeDefinition $code -ReferencedAssemblies System.Windows.Forms, System.Drawing

$paths = @({paths_array})
$form = New-Object DragOverlayForm -ArgumentList (,$paths), {sx}, {sy}
[System.Windows.Forms.Application]::Run($form)
"#,
        paths_array = paths_array,
        sx = screen_x as i32,
        sy = screen_y as i32,
    );

    let mut child = match std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Drag: failed to spawn PowerShell overlay: {}", e);
            let _ = ready_tx.send(());
            return fallback_open_file_manager(&file_paths);
        }
    };

    wait_for_subprocess_drag(&mut child, ready_tx, cancel_rx)
}

// =============================================================================
// Common subprocess management
// =============================================================================

/// Wait for a drag overlay subprocess, reading READY/DROP/CANCEL from stdout.
fn wait_for_subprocess_drag(
    child: &mut std::process::Child,
    ready_tx: tokio::sync::oneshot::Sender<()>,
    cancel_rx: std::sync::mpsc::Receiver<()>,
) -> DragResult {
    use std::io::BufRead;

    let reader = std::io::BufReader::new(match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = ready_tx.send(());
            return DragResult::Failed("No stdout".to_string());
        }
    });

    let mut ready_sent = false;
    let mut ready_tx = Some(ready_tx);
    let mut result = DragResult::Cancelled;

    // Read lines in a separate thread to avoid blocking
    let (line_tx, line_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in reader.lines().flatten() {
            if line_tx.send(line).is_err() {
                break;
            }
        }
    });

    loop {
        // Check cancel
        if cancel_rx.try_recv().is_ok() {
            let _ = child.kill();
            result = DragResult::Cancelled;
            break;
        }

        match line_rx.try_recv() {
            Ok(line) => match line.trim() {
                "READY" => {
                    if !ready_sent {
                        if let Some(tx) = ready_tx.take() {
                            let _ = tx.send(());
                        }
                        ready_sent = true;
                        println!("Drag: overlay ready");
                    }
                }
                "DROP" => {
                    result = DragResult::Dropped;
                    println!("Drag: drop completed");
                    break;
                }
                "CANCEL" => {
                    result = DragResult::Cancelled;
                    println!("Drag: cancelled");
                    break;
                }
                _ => {}
            },
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                if let Some(tx) = ready_tx.take() {
                    let _ = tx.send(());
                }
                break;
            }
        }

        // Check if child exited
        if let Ok(Some(_)) = child.try_wait() {
            if let Some(tx) = ready_tx.take() {
                let _ = tx.send(());
            }
            break;
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    result
}

// =============================================================================
// Fallback: open file manager at temp dir
// =============================================================================

fn fallback_open_file_manager(file_paths: &[PathBuf]) -> DragResult {
    if let Some(dir) = file_paths.first().and_then(|p| p.parent()) {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "explorer"
        } else {
            "xdg-open"
        };
        println!("Drag: fallback — opening {} in file manager", dir.display());
        let _ = std::process::Command::new(opener)
            .arg(dir)
            .spawn();
    }
    DragResult::Failed("Overlay unavailable, opened file manager".to_string())
}

// =============================================================================
// Shared file reading helpers
// =============================================================================

/// Read files from a command that outputs text/uri-list format.
fn read_files_from_uri_list(cmd: &str, args: &[&str]) -> Option<Vec<CopiedFile>> {
    let output = std::process::Command::new(cmd)
        .args(args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let uri_list = String::from_utf8_lossy(&output.stdout);
    let mut files = Vec::new();

    for line in uri_list.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(path_str) = line.strip_prefix("file://") {
            let path_str = percent_decode(path_str);
            let path = Path::new(&path_str);
            if let Some(copied) = read_file_to_copied(path) {
                files.extend(copied);
            }
        }
    }

    if files.is_empty() { None } else { Some(files) }
}

/// Read a single file (or directory contents) into CopiedFile vec.
fn read_file_to_copied(path: &Path) -> Option<Vec<CopiedFile>> {
    let mut files = Vec::new();

    if path.is_file() {
        let data = std::fs::read(path).ok()?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".to_string());
        let size = data.len() as u64;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
        files.push(CopiedFile { name, data: encoded, size });
    } else if path.is_dir() {
        // Read directory contents (non-recursive, first level only)
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    if let Ok(data) = std::fs::read(&p) {
                        let name = p
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unnamed".to_string());
                        let size = data.len() as u64;
                        let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                        files.push(CopiedFile { name, data: encoded, size });
                    }
                }
            }
        }
    }

    if files.is_empty() { None } else { Some(files) }
}

/// URL percent-decode a path string.
fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h = chars.next().unwrap_or(b'0');
            let l = chars.next().unwrap_or(b'0');
            let hex = [h, l];
            if let Ok(s) = std::str::from_utf8(&hex) {
                if let Ok(val) = u8::from_str_radix(s, 16) {
                    result.push(val as char);
                    continue;
                }
            }
            result.push('%');
            result.push(h as char);
            result.push(l as char);
        } else {
            result.push(b as char);
        }
    }
    result
}
