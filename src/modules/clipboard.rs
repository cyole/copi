use crate::modules::sync::{ClipboardContent, CopiedFile};
use anyhow::Result;
use arboard::{Clipboard, ImageData};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::Command;

// 图片大小限制：5MB
const MAX_IMAGE_SIZE: usize = 5 * 1024 * 1024;
// 图片尺寸限制：4096x4096
const MAX_IMAGE_DIMENSION: u32 = 4096;
// Default max file size for clipboard file copy: 50MB
const DEFAULT_MAX_CLIPBOARD_FILE_SIZE: u64 = 50 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
enum ClipboardBackend {
    Arboard,
    #[cfg(target_os = "linux")]
    WlClipboard,
}

pub struct ClipboardMonitor {
    clipboard: Option<Clipboard>,
    backend: ClipboardBackend,
    last_hash: Option<String>,
    max_file_size: u64,
    /// Temp dir for writing received clipboard files so Ctrl+V works
    recv_dir: PathBuf,
    /// PID of our wl-copy background process (if any), killed before reading
    /// to prevent deadlocks where wl-paste talks to our own wl-copy.
    #[cfg(target_os = "linux")]
    wl_copy_pid: Option<u32>,
}

impl ClipboardMonitor {
    pub fn new(max_file_size: Option<u64>) -> Result<Self> {
        let max_file_size = max_file_size.unwrap_or(DEFAULT_MAX_CLIPBOARD_FILE_SIZE);
        let recv_dir = Self::init_recv_dir()?;

        // Try to detect if we're running on Wayland
        #[cfg(target_os = "linux")]
        {
            let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
            let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
            let is_gnome = desktop.to_uppercase().contains("GNOME");

            if is_wayland && !is_gnome {
                // Use wl-clipboard on non-GNOME Wayland compositors (Sway, Hyprland, etc.)
                // On GNOME, wl-clipboard creates popup windows that steal focus — use arboard instead.
                if Self::check_wl_clipboard_available() {
                    println!("Detected Wayland (non-GNOME), using wl-clipboard backend");
                    return Ok(Self {
                        clipboard: None,
                        backend: ClipboardBackend::WlClipboard,
                        last_hash: None,
                        max_file_size,
                        recv_dir,
                        #[cfg(target_os = "linux")]
                        wl_copy_pid: None,
                    });
                } else {
                    println!(
                        "Wayland detected but wl-clipboard not found, falling back to arboard"
                    );
                    println!("Install wl-clipboard for better Wayland support:");
                    println!("  Ubuntu/Debian: sudo apt install wl-clipboard");
                    println!("  Fedora: sudo dnf install wl-clipboard");
                    println!("  Arch: sudo pacman -S wl-clipboard");
                }
            }
        }

        // Use arboard as default or fallback
        Ok(Self {
            clipboard: Some(Clipboard::new()?),
            backend: ClipboardBackend::Arboard,
            last_hash: None,
            max_file_size,
            recv_dir,
            #[cfg(target_os = "linux")]
            wl_copy_pid: None,
        })
    }

    fn init_recv_dir() -> Result<PathBuf> {
        let base = std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        let dir = base.join("copi-clipboard-files");
        // Clean up old files from previous session
        if dir.exists() {
            let _ = std::fs::remove_dir_all(&dir);
        }
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    #[cfg(target_os = "linux")]
    fn check_wl_clipboard_available() -> bool {
        Command::new("wl-paste").arg("--version").output().is_ok()
    }

    fn hash_content(content: &ClipboardContent) -> String {
        let mut hasher = Sha256::new();
        match content {
            ClipboardContent::Text(text) => {
                hasher.update(b"text:");
                hasher.update(text.as_bytes());
            }
            ClipboardContent::Image {
                data,
                width,
                height,
            } => {
                hasher.update(b"image:");
                hasher.update(data.as_bytes());
                hasher.update(&width.to_le_bytes());
                hasher.update(&height.to_le_bytes());
            }
            ClipboardContent::Html { html, text } => {
                hasher.update(b"html:");
                hasher.update(html.as_bytes());
                hasher.update(text.as_bytes());
            }
            ClipboardContent::File { path, data, size } => {
                hasher.update(b"file:");
                hasher.update(path.as_bytes());
                hasher.update(data.as_bytes());
                hasher.update(&size.to_le_bytes());
            }
            ClipboardContent::FileCopy { files } => {
                hasher.update(b"filecopy:");
                for f in files {
                    hasher.update(f.name.as_bytes());
                    hasher.update(&f.size.to_le_bytes());
                }
            }
            ClipboardContent::PeerDiscovery { .. } => {
                // Control message, not clipboard content
                return "peer_discovery".to_string();
            }
        }
        format!("{:x}", hasher.finalize())
    }

    pub fn get_clipboard_content(&mut self) -> Result<Option<ClipboardContent>> {
        let content_result: Result<ClipboardContent> = match self.backend {
            ClipboardBackend::Arboard => {
                // Check for file URIs first (Ctrl+C / Cmd+C on files)
                #[cfg(target_os = "macos")]
                if let Some(files) = self.macos_paste_files() {
                    return self.dedup(ClipboardContent::FileCopy { files });
                }
                #[cfg(target_os = "linux")]
                if let Some(files) = self.xclip_paste_files() {
                    return self.dedup(ClipboardContent::FileCopy { files });
                }

                let clipboard = self
                    .clipboard
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Clipboard not initialized"))?;

                // Try to get image first
                if let Ok(img) = clipboard.get_image() {
                    match Self::image_data_to_png(&img) {
                        Ok(png_data) => {
                            let base64_data = base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                &png_data,
                            );

                            Ok(ClipboardContent::Image {
                                data: base64_data,
                                width: img.width as u32,
                                height: img.height as u32,
                            })
                        }
                        Err(e) => {
                            eprintln!("Failed to process image from clipboard: {}", e);
                            // 尝试获取文本作为备选
                            clipboard
                                .get_text()
                                .map(ClipboardContent::Text)
                                .map_err(|e| {
                                    anyhow::anyhow!("Failed to get clipboard content: {}", e)
                                })
                        }
                    }
                } else {
                    // Fall back to text
                    clipboard
                        .get_text()
                        .map(ClipboardContent::Text)
                        .map_err(|e| anyhow::anyhow!("Failed to get clipboard content: {}", e))
                }
            }
            #[cfg(target_os = "linux")]
            ClipboardBackend::WlClipboard => {
                // Kill our own wl-copy before reading to prevent deadlocks
                self.kill_own_wl_copy();

                // Check for file URIs first (Ctrl+C on files in file manager)
                if let Some(files) = self.wl_paste_files() {
                    return self.dedup(ClipboardContent::FileCopy { files });
                }
                // Try to get image
                match Self::wl_paste_image() {
                    Ok(img_data) => Ok(img_data),
                    Err(e) => {
                        if !e.to_string().contains("wl-paste image failed") {
                            eprintln!("Failed to get image from clipboard: {}", e);
                        }
                        // Fall back to text
                        Self::wl_paste().map(ClipboardContent::Text)
                    }
                }
            }
        };

        match content_result {
            Ok(content) => self.dedup(content),
            Err(e) => {
                eprintln!("Error reading clipboard: {}", e);
                Ok(None)
            }
        }
    }

    fn image_data_to_png(img: &ImageData) -> Result<Vec<u8>> {
        use image::{DynamicImage, ImageBuffer, RgbaImage};
        use std::io::Cursor;

        let width = img.width as u32;
        let height = img.height as u32;

        // 检查图片尺寸
        if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
            println!(
                "Image dimensions too large: {}x{}, resizing to {}x{}",
                width, height, MAX_IMAGE_DIMENSION, MAX_IMAGE_DIMENSION
            );
        }

        // Convert ImageData bytes to RgbaImage
        let img_buffer: RgbaImage = ImageBuffer::from_raw(width, height, img.bytes.to_vec())
            .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

        let mut dynamic_img = DynamicImage::ImageRgba8(img_buffer);

        // 计算初始缩放尺寸
        let mut target_width = width.min(MAX_IMAGE_DIMENSION);
        let mut target_height = height.min(MAX_IMAGE_DIMENSION);

        // 保持宽高比
        if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
            let scale = (MAX_IMAGE_DIMENSION as f64 / width.max(height) as f64).min(1.0);
            target_width = (width as f64 * scale) as u32;
            target_height = (height as f64 * scale) as u32;
        }

        // 估算大小并预先缩放
        let estimated_size = target_width as usize * target_height as usize * 4;
        if estimated_size > MAX_IMAGE_SIZE * 2 {
            let scale = ((MAX_IMAGE_SIZE * 2) as f64 / estimated_size as f64).sqrt();
            target_width = (target_width as f64 * scale) as u32;
            target_height = (target_height as f64 * scale) as u32;

            println!(
                "Pre-scaling image from {}x{} to {}x{} for size limit",
                width, height, target_width, target_height
            );
        }

        // 如果需要缩放
        if target_width != width || target_height != height {
            dynamic_img = dynamic_img.resize(
                target_width,
                target_height,
                image::imageops::FilterType::Lanczos3,
            );
        }

        // 尝试编码，如果太大则继续缩小
        let mut attempts = 0;
        let max_attempts = 3;

        loop {
            attempts += 1;

            // Encode as PNG
            let mut png_data = Vec::new();
            let mut cursor = Cursor::new(&mut png_data);
            dynamic_img.write_to(&mut cursor, image::ImageFormat::Png)?;

            // 检查大小
            if png_data.len() <= MAX_IMAGE_SIZE {
                if attempts > 1 {
                    println!(
                        "Successfully compressed image to {} bytes after {} attempts",
                        png_data.len(),
                        attempts
                    );
                }
                return Ok(png_data);
            }

            // 如果还是太大且未超过最大尝试次数
            if attempts < max_attempts {
                let current_width = dynamic_img.width();
                let current_height = dynamic_img.height();
                let scale = 0.7; // 每次缩小到 70%
                let new_width = (current_width as f64 * scale) as u32;
                let new_height = (current_height as f64 * scale) as u32;

                println!(
                    "Image still too large ({} bytes), resizing from {}x{} to {}x{} (attempt {}/{})",
                    png_data.len(),
                    current_width,
                    current_height,
                    new_width,
                    new_height,
                    attempts,
                    max_attempts
                );

                dynamic_img = dynamic_img.resize(
                    new_width.max(100), // 最小保持 100px
                    new_height.max(100),
                    image::imageops::FilterType::Triangle, // 使用更快的算法
                );
            } else {
                anyhow::bail!(
                    "Failed to compress image to size limit after {} attempts. Final size: {} bytes (max: {} bytes)",
                    attempts,
                    png_data.len(),
                    MAX_IMAGE_SIZE
                );
            }
        }
    }

    fn png_to_image_data(png_data: &[u8], width: u32, height: u32) -> Result<ImageData<'static>> {
        use image::ImageReader;
        use std::io::Cursor;

        let img = ImageReader::new(Cursor::new(png_data))
            .with_guessed_format()?
            .decode()?;

        let rgba = img.to_rgba8();
        let bytes = rgba.into_raw();

        Ok(ImageData {
            width: width as usize,
            height: height as usize,
            bytes: std::borrow::Cow::Owned(bytes),
        })
    }

    fn dedup(&mut self, content: ClipboardContent) -> Result<Option<ClipboardContent>> {
        let hash = Self::hash_content(&content);
        if self.last_hash.as_ref() != Some(&hash) {
            self.last_hash = Some(hash);
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    /// Check if clipboard contains file URIs (from Ctrl+C in file manager).
    /// Returns the files with their content if detected.
    #[cfg(target_os = "linux")]
    fn wl_paste_files(&self) -> Option<Vec<CopiedFile>> {
        // Check what types are available on the clipboard
        let types_output = Self::wl_paste_cmd(&["--list-types"]).ok()?;
        let types = String::from_utf8_lossy(&types_output.stdout);
        if !types.lines().any(|t| t.trim() == "text/uri-list") {
            return None;
        }

        // Get the URI list
        let uri_output = Self::wl_paste_cmd(&["--type", "text/uri-list"]).ok()?;
        if !uri_output.status.success() {
            return None;
        }
        let uri_text = String::from_utf8_lossy(&uri_output.stdout);

        let mut files = Vec::new();
        for line in uri_text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Parse file:// URI
            let path = if let Some(p) = line.strip_prefix("file://") {
                // URL-decode the path
                percent_decode(p)
            } else {
                continue;
            };

            let path = std::path::Path::new(&path);
            if !path.is_file() {
                continue;
            }

            let metadata = std::fs::metadata(path).ok()?;
            let size = metadata.len();

            let data = std::fs::read(path).ok()?;
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let encoded = base64::engine::general_purpose::STANDARD.encode(&data);

            files.push(CopiedFile {
                name,
                data: encoded,
                size,
            });
        }

        if files.is_empty() {
            None
        } else {
            Some(files)
        }
    }

    /// Kill our own wl-copy process before reading clipboard to prevent deadlocks.
    /// When copi sets clipboard via wl-copy, a background process stays alive to serve
    /// paste requests. If we then call wl-paste, it asks our wl-copy for data, creating
    /// a deadlock. Killing our wl-copy first forces the compositor to serve cached data.
    #[cfg(target_os = "linux")]
    fn kill_own_wl_copy(&mut self) {
        if let Some(pid) = self.wl_copy_pid.take() {
            let _ = Command::new("kill")
                .arg(pid.to_string())
                .output();
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    /// Run wl-paste with timeout to prevent hangs from unresponsive clipboard owners.
    /// Suppresses stderr (wl-paste prints errors like "not available as type" to stderr).
    #[cfg(target_os = "linux")]
    fn wl_paste_cmd(args: &[&str]) -> Result<std::process::Output> {
        use std::process::Stdio;
        let output = Command::new("timeout")
            .arg("3")
            .arg("wl-paste")
            .args(args)
            .stderr(Stdio::null())
            .output()?;
        Ok(output)
    }

    #[cfg(target_os = "linux")]
    fn wl_paste() -> Result<String> {
        let output = Self::wl_paste_cmd(&["--no-newline"])?;

        if output.status.success() {
            Ok(String::from_utf8(output.stdout)?)
        } else {
            anyhow::bail!("wl-paste failed")
        }
    }

    #[cfg(target_os = "linux")]
    fn wl_paste_image() -> Result<ClipboardContent> {
        let output = Self::wl_paste_cmd(&["--type", "image/png"])?;

        if output.status.success() && !output.stdout.is_empty() {
            let png_data = &output.stdout;

            // 检查大小
            if png_data.len() > MAX_IMAGE_SIZE {
                println!(
                    "Clipboard image too large ({} bytes), reprocessing...",
                    png_data.len()
                );

                // 解码并重新处理
                use image::ImageReader;
                use std::io::Cursor;

                let img = ImageReader::new(Cursor::new(png_data))
                    .with_guessed_format()?
                    .decode()?;

                // 转换为 ImageData 格式并使用我们的压缩逻辑
                let rgba = img.to_rgba8();
                let width = img.width();
                let height = img.height();

                let img_data = ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: std::borrow::Cow::Owned(rgba.into_raw()),
                };

                // 使用我们的压缩函数
                let compressed_png = Self::image_data_to_png(&img_data)?;
                let base64_data = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &compressed_png,
                );

                Ok(ClipboardContent::Image {
                    data: base64_data,
                    width,
                    height,
                })
            } else {
                // 大小合适，直接使用
                use image::ImageReader;
                use std::io::Cursor;

                let img = ImageReader::new(Cursor::new(png_data))
                    .with_guessed_format()?
                    .decode()?;

                let width = img.width();
                let height = img.height();

                // Encode as base64
                let base64_data =
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png_data);

                Ok(ClipboardContent::Image {
                    data: base64_data,
                    width,
                    height,
                })
            }
        } else {
            anyhow::bail!("wl-paste image failed")
        }
    }

    pub fn set_clipboard_content(&mut self, content: &ClipboardContent) -> Result<()> {
        // Handle FileCopy before borrowing clipboard to avoid borrow conflicts
        if let ClipboardContent::FileCopy { files } = content {
            let paths = self.write_files_to_recv_dir(files)?;
            match self.backend {
                ClipboardBackend::Arboard => {
                    #[cfg(target_os = "macos")]
                    {
                        Self::macos_copy_files(&paths)?;
                    }
                    #[cfg(target_os = "linux")]
                    {
                        Self::xclip_copy_file_uris(&paths)?;
                    }
                }
                #[cfg(target_os = "linux")]
                ClipboardBackend::WlClipboard => {
                    self.kill_own_wl_copy();
                    let pid = Self::wl_copy_file_uris(&paths)?;
                    self.wl_copy_pid = Some(pid);
                }
            }
            self.last_hash = Some(Self::hash_content(content));
            return Ok(());
        }

        match self.backend {
            ClipboardBackend::Arboard => {
                let clipboard = self
                    .clipboard
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Clipboard not initialized"))?;

                match content {
                    ClipboardContent::Text(text) => {
                        clipboard
                            .set_text(text)
                            .map_err(|e| anyhow::anyhow!("Failed to set clipboard text: {}", e))?;
                    }
                    ClipboardContent::Image {
                        data,
                        width,
                        height,
                    } => {
                        let png_data = base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            data,
                        )?;
                        let img_data = Self::png_to_image_data(&png_data, *width, *height)?;
                        clipboard
                            .set_image(img_data)
                            .map_err(|e| anyhow::anyhow!("Failed to set clipboard image: {}", e))?;
                    }
                    ClipboardContent::Html { html: _, text } => {
                        clipboard.set_text(text).map_err(|e| {
                            anyhow::anyhow!("Failed to set clipboard HTML as text: {}", e)
                        })?;
                    }
                    ClipboardContent::File { .. } | ClipboardContent::FileCopy { .. } | ClipboardContent::PeerDiscovery { .. } => {
                        return Ok(());
                    }
                }
            }
            #[cfg(target_os = "linux")]
            ClipboardBackend::WlClipboard => {
                // Kill previous wl-copy before setting new content
                self.kill_own_wl_copy();
                let pid = match content {
                    ClipboardContent::Text(text) => Self::wl_copy_text(text)?,
                    ClipboardContent::Image { data, .. } => Self::wl_copy_image(data)?,
                    ClipboardContent::Html { html, text: _ } => Self::wl_copy_html(html)?,
                    ClipboardContent::File { .. } | ClipboardContent::FileCopy { .. } | ClipboardContent::PeerDiscovery { .. } => {
                        return Ok(());
                    }
                };
                self.wl_copy_pid = Some(pid);
            }
        }

        self.last_hash = Some(Self::hash_content(content));
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn wl_copy_text(content: &str) -> Result<u32> {
        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new("wl-copy")
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(content.as_bytes())?;
        }

        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("wl-copy failed");
        }
        Self::find_wl_copy_pid()
    }

    #[cfg(target_os = "linux")]
    fn wl_copy_image(base64_data: &str) -> Result<u32> {
        use std::io::Write;
        use std::process::Stdio;

        let png_data =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, base64_data)?;

        let mut child = Command::new("wl-copy")
            .arg("--type")
            .arg("image/png")
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&png_data)?;
        }

        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("wl-copy image failed");
        }
        Self::find_wl_copy_pid()
    }

    #[cfg(target_os = "linux")]
    fn wl_copy_html(html: &str) -> Result<u32> {
        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new("wl-copy")
            .arg("--type")
            .arg("text/html")
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(html.as_bytes())?;
        }

        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("wl-copy html failed");
        }
        Self::find_wl_copy_pid()
    }

    /// Find the newest wl-copy process PID (the forked background child).
    #[cfg(target_os = "linux")]
    fn find_wl_copy_pid() -> Result<u32> {
        let output = Command::new("pgrep").arg("-n").arg("wl-copy").output()?;
        let s = String::from_utf8_lossy(&output.stdout);
        s.trim()
            .parse::<u32>()
            .map_err(|_| anyhow::anyhow!("Could not find wl-copy PID"))
    }

    /// Detect files on macOS clipboard (Cmd+C in Finder).
    #[cfg(target_os = "macos")]
    fn macos_paste_files(&self) -> Option<Vec<CopiedFile>> {
        // Check if clipboard contains file references using osascript
        let output = Command::new("osascript")
            .arg("-e")
            .arg("clipboard info")
            .output()
            .ok()?;

        let info = String::from_utf8_lossy(&output.stdout);
        // Finder file copies show «class furl» in clipboard info
        if !info.contains("furl") {
            return None;
        }

        // Get file paths from clipboard
        let output = Command::new("osascript")
            .arg("-e")
            .arg(
                r#"
set output to ""
try
    set clipData to the clipboard as «class furl»
    set output to POSIX path of clipData
on error
    try
        set clipList to the clipboard as list
        repeat with f in clipList
            try
                set output to output & POSIX path of (f as «class furl») & linefeed
            end try
        end repeat
    end try
end try
return output
"#,
            )
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let paths_text = String::from_utf8_lossy(&output.stdout);
        let mut files = Vec::new();

        for line in paths_text.lines() {
            let path = line.trim();
            if path.is_empty() {
                continue;
            }
            let p = std::path::Path::new(path);
            if !p.is_file() {
                continue;
            }
            let metadata = std::fs::metadata(p).ok()?;
            let size = metadata.len();
            let data = std::fs::read(p).ok()?;
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
            files.push(CopiedFile {
                name,
                data: encoded,
                size,
            });
        }

        if files.is_empty() {
            None
        } else {
            Some(files)
        }
    }

    /// Set macOS clipboard to file references so Cmd+V in Finder pastes them.
    #[cfg(target_os = "macos")]
    fn macos_copy_files(paths: &[String]) -> Result<()> {
        // Build AppleScript to set clipboard to POSIX file references
        let file_refs: Vec<String> = paths
            .iter()
            .map(|p| format!("POSIX file \"{}\"", p.replace('\"', "\\\"")))
            .collect();

        let script = if file_refs.len() == 1 {
            format!("set the clipboard to {}", file_refs[0])
        } else {
            format!("set the clipboard to {{{}}}", file_refs.join(", "))
        };

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("osascript failed to set clipboard files: {}", err);
        }

        Ok(())
    }

    /// Detect files on clipboard via xclip (XWayland) on GNOME.
    /// xclip talks to X11 clipboard which shares with Wayland — no popup windows.
    #[cfg(target_os = "linux")]
    fn xclip_paste_files(&self) -> Option<Vec<CopiedFile>> {
        use std::process::Stdio;

        // Check available MIME types via xclip
        let output = Command::new("timeout")
            .arg("2")
            .arg("xclip")
            .args(["-selection", "clipboard", "-t", "TARGETS", "-o"])
            .stderr(Stdio::null())
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let targets = String::from_utf8_lossy(&output.stdout);
        if !targets.lines().any(|t| t.trim() == "text/uri-list") {
            return None;
        }

        // Read file URIs
        let output = Command::new("timeout")
            .arg("2")
            .arg("xclip")
            .args(["-selection", "clipboard", "-t", "text/uri-list", "-o"])
            .stderr(Stdio::null())
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let uri_text = String::from_utf8_lossy(&output.stdout);
        let mut files = Vec::new();

        for line in uri_text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let path = if let Some(p) = line.strip_prefix("file://") {
                percent_decode(p)
            } else {
                continue;
            };

            let p = std::path::Path::new(&path);
            if !p.is_file() {
                continue;
            }

            let metadata = std::fs::metadata(p).ok()?;
            let size = metadata.len();

            let data = std::fs::read(p).ok()?;
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
            files.push(CopiedFile { name, data: encoded, size });
        }

        if files.is_empty() { None } else { Some(files) }
    }

    /// Set clipboard to file URIs via xclip (XWayland) for Ctrl+V in file managers.
    #[cfg(target_os = "linux")]
    fn xclip_copy_file_uris(paths: &[String]) -> Result<()> {
        use std::io::Write;
        use std::process::Stdio;

        let uri_list: String = paths
            .iter()
            .map(|p| format!("file://{}", p))
            .collect::<Vec<_>>()
            .join("\r\n");

        let mut child = Command::new("xclip")
            .args(["-selection", "clipboard", "-t", "text/uri-list", "-i"])
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(uri_list.as_bytes())?;
            stdin.write_all(b"\r\n")?;
        }

        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("xclip set file URIs failed");
        }
        Ok(())
    }

    /// Write received files to the temp receive directory.
    /// Returns the list of full paths written.
    fn write_files_to_recv_dir(&self, files: &[CopiedFile]) -> Result<Vec<String>> {
        // Clear old files
        if self.recv_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.recv_dir);
        }
        std::fs::create_dir_all(&self.recv_dir)?;

        let mut paths = Vec::new();
        for file in files {
            // Sanitize filename
            let name = std::path::Path::new(&file.name)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if name.is_empty() {
                continue;
            }
            let dest = self.recv_dir.join(&name);
            let data = base64::engine::general_purpose::STANDARD
                .decode(&file.data)
                .map_err(|e| anyhow::anyhow!("Failed to decode file: {}", e))?;
            std::fs::write(&dest, &data)?;
            paths.push(dest.to_string_lossy().to_string());
        }
        Ok(paths)
    }

    /// Set clipboard to file URIs via wl-copy so Ctrl+V pastes the files.
    #[cfg(target_os = "linux")]
    fn wl_copy_file_uris(paths: &[String]) -> Result<u32> {
        use std::io::Write;
        use std::process::Stdio;

        let uri_list: String = paths
            .iter()
            .map(|p| format!("file://{}", p))
            .collect::<Vec<_>>()
            .join("\r\n");

        let mut child = Command::new("wl-copy")
            .arg("--type")
            .arg("text/uri-list")
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(uri_list.as_bytes())?;
            stdin.write_all(b"\r\n")?;
        }

        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("wl-copy file URIs failed");
        }
        Self::find_wl_copy_pid()
    }
}

/// Simple percent-decoding for file:// URIs.
fn percent_decode(s: &str) -> String {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) = u8::from_str_radix(
                &String::from_utf8_lossy(&bytes[i + 1..i + 3]),
                16,
            ) {
                result.push(val);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}
