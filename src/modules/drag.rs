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
    _cancel_tx: std::sync::mpsc::Sender<()>,
    /// Temp directory holding the dragged files (cleaned up on drop).
    temp_dir: Option<PathBuf>,
}

impl Drop for DragSession {
    fn drop(&mut self) {
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
        // Sanitize filename: prevent directory traversal
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
///
/// Returns None if no drag is in progress or files can't be detected.
/// Platform-specific: reads from the drag pasteboard / clipboard.
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
/// This spawns a dedicated OS thread because all platform drag APIs block.
/// The `ready_tx` is signalled once the drag session is active.
/// Mouse simulation events arriving after that point will be picked up
/// by the OS drag loop.
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
        _cancel_tx: cancel_tx,
        temp_dir: Some(temp_dir),
    })
}

/// Platform-specific overlay drag implementation.
/// This runs on a dedicated OS thread and blocks until the drag completes.
fn run_overlay_drag(
    temp_dir: PathBuf,
    screen_x: f64,
    screen_y: f64,
    ready_tx: tokio::sync::oneshot::Sender<()>,
    _cancel_rx: std::sync::mpsc::Receiver<()>,
) -> DragResult {
    // Collect file paths from temp dir
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
        screen_x,
        screen_y,
        file_paths.len()
    );

    #[cfg(target_os = "linux")]
    return run_overlay_drag_linux(file_paths, screen_x, screen_y, ready_tx, _cancel_rx);

    #[cfg(target_os = "macos")]
    return run_overlay_drag_macos(file_paths, screen_x, screen_y, ready_tx, _cancel_rx);

    #[cfg(target_os = "windows")]
    return run_overlay_drag_windows(file_paths, screen_x, screen_y, ready_tx, _cancel_rx);

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        eprintln!("Drag: overlay not supported on this platform");
        let _ = ready_tx.send(());
        DragResult::Failed("Unsupported platform".to_string())
    }
}

// =============================================================================
// Linux/Wayland implementation (GTK4 overlay)
// =============================================================================

#[cfg(target_os = "linux")]
fn read_dragged_files_linux() -> Option<Vec<CopiedFile>> {
    // Try to read dragged files via wl-paste during an active drag.
    // On Wayland, the compositor may expose the drag offer via the clipboard.
    let output = std::process::Command::new("wl-paste")
        .args(["--type", "text/uri-list"])
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
            if path.is_file() {
                if let Ok(data) = std::fs::read(path) {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unnamed".to_string());
                    let size = data.len() as u64;
                    let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                    files.push(CopiedFile {
                        name,
                        data: encoded,
                        size,
                    });
                }
            }
        }
    }

    if files.is_empty() { None } else { Some(files) }
}

#[cfg(target_os = "linux")]
fn run_overlay_drag_linux(
    _file_paths: Vec<PathBuf>,
    _screen_x: f64,
    _screen_y: f64,
    ready_tx: tokio::sync::oneshot::Sender<()>,
    _cancel_rx: std::sync::mpsc::Receiver<()>,
) -> DragResult {
    // TODO: Phase 3 — GTK4 overlay window + GtkDragSource
    // For now, signal ready immediately (stub behavior)
    println!("Drag: Linux/Wayland overlay stub — GTK4 implementation pending (Phase 3)");
    let _ = ready_tx.send(());

    // Wait for cancel signal (placeholder for GTK main loop)
    let _ = _cancel_rx.recv();
    DragResult::Cancelled
}

// =============================================================================
// macOS implementation (AppKit overlay)
// =============================================================================

#[cfg(target_os = "macos")]
fn read_dragged_files_macos() -> Option<Vec<CopiedFile>> {
    // TODO: Phase 4 — Read NSPasteboard(name: .drag) during active drag
    // For now, try reading the general pasteboard for file URLs
    let output = std::process::Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to get the clipboard as «class furl»"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    // Parse macOS file references
    let text = String::from_utf8_lossy(&output.stdout);
    let path_str = text.trim().strip_prefix("file ")?.trim_matches('"');
    let path = Path::new(path_str);
    if !path.is_file() {
        return None;
    }

    let data = std::fs::read(path).ok()?;
    let name = path.file_name()?.to_string_lossy().to_string();
    let size = data.len() as u64;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
    Some(vec![CopiedFile { name, data: encoded, size }])
}

#[cfg(target_os = "macos")]
fn run_overlay_drag_macos(
    _file_paths: Vec<PathBuf>,
    _screen_x: f64,
    _screen_y: f64,
    ready_tx: tokio::sync::oneshot::Sender<()>,
    _cancel_rx: std::sync::mpsc::Receiver<()>,
) -> DragResult {
    // TODO: Phase 4 — NSWindow + NSDraggingSource via objc2
    println!("Drag: macOS overlay stub — AppKit implementation pending (Phase 4)");
    let _ = ready_tx.send(());
    let _ = _cancel_rx.recv();
    DragResult::Cancelled
}

// =============================================================================
// Windows implementation (Win32 overlay)
// =============================================================================

#[cfg(target_os = "windows")]
fn read_dragged_files_windows() -> Option<Vec<CopiedFile>> {
    // TODO: Phase 5 — Register IDropTarget at screen edge, read CF_HDROP
    None
}

#[cfg(target_os = "windows")]
fn run_overlay_drag_windows(
    _file_paths: Vec<PathBuf>,
    _screen_x: f64,
    _screen_y: f64,
    ready_tx: tokio::sync::oneshot::Sender<()>,
    _cancel_rx: std::sync::mpsc::Receiver<()>,
) -> DragResult {
    // TODO: Phase 5 — HWND + IDataObject + DoDragDrop via windows-rs
    println!("Drag: Windows overlay stub — Win32 implementation pending (Phase 5)");
    let _ = ready_tx.send(());
    let _ = _cancel_rx.recv();
    DragResult::Cancelled
}

// =============================================================================
// Utility
// =============================================================================

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
