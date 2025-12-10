use crate::modules::sync::ClipboardContent;
use anyhow::Result;
use arboard::{Clipboard, ImageData};
use sha2::{Digest, Sha256};

#[cfg(target_os = "linux")]
use std::process::Command;

// 图片大小限制：5MB
const MAX_IMAGE_SIZE: usize = 5 * 1024 * 1024;
// 图片尺寸限制：4096x4096
const MAX_IMAGE_DIMENSION: u32 = 4096;

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
}

impl ClipboardMonitor {
    pub fn new() -> Result<Self> {
        // Try to detect if we're running on Wayland
        #[cfg(target_os = "linux")]
        {
            let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();

            if is_wayland {
                // Check if wl-clipboard tools are available
                if Self::check_wl_clipboard_available() {
                    println!("Detected Wayland, using wl-clipboard backend");
                    return Ok(Self {
                        clipboard: None,
                        backend: ClipboardBackend::WlClipboard,
                        last_hash: None,
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
        })
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
        }
        format!("{:x}", hasher.finalize())
    }

    pub fn get_clipboard_content(&mut self) -> Result<Option<ClipboardContent>> {
        let content_result: Result<ClipboardContent> = match self.backend {
            ClipboardBackend::Arboard => {
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
                // Try to get image first
                match Self::wl_paste_image() {
                    Ok(img_data) => Ok(img_data),
                    Err(e) => {
                        // 记录图片获取失败，但不是错误（可能剪贴板中没有图片）
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
            Ok(content) => {
                let hash = Self::hash_content(&content);

                if self.last_hash.as_ref() != Some(&hash) {
                    self.last_hash = Some(hash);
                    Ok(Some(content))
                } else {
                    Ok(None)
                }
            }
            Err(e) => {
                // 记录错误但不中断程序
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

    #[cfg(target_os = "linux")]
    fn wl_paste() -> Result<String> {
        let output = Command::new("wl-paste").arg("--no-newline").output()?;

        if output.status.success() {
            Ok(String::from_utf8(output.stdout)?)
        } else {
            anyhow::bail!("wl-paste failed")
        }
    }

    #[cfg(target_os = "linux")]
    fn wl_paste_image() -> Result<ClipboardContent> {
        let output = Command::new("wl-paste")
            .arg("--type")
            .arg("image/png")
            .output()?;

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
                        // Decode base64
                        let png_data = base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            data,
                        )?;

                        // Convert to ImageData
                        let img_data = Self::png_to_image_data(&png_data, *width, *height)?;

                        clipboard
                            .set_image(img_data)
                            .map_err(|e| anyhow::anyhow!("Failed to set clipboard image: {}", e))?;
                    }
                    ClipboardContent::Html { html: _, text } => {
                        // arboard 不直接支持 HTML，使用纯文本回退
                        clipboard.set_text(text).map_err(|e| {
                            anyhow::anyhow!("Failed to set clipboard HTML as text: {}", e)
                        })?;
                    }
                }
            }
            #[cfg(target_os = "linux")]
            ClipboardBackend::WlClipboard => match content {
                ClipboardContent::Text(text) => {
                    Self::wl_copy_text(text)?;
                }
                ClipboardContent::Image { data, .. } => {
                    Self::wl_copy_image(data)?;
                }
                ClipboardContent::Html { html, text: _ } => {
                    Self::wl_copy_html(html)?;
                }
            },
        }

        self.last_hash = Some(Self::hash_content(content));
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn wl_copy_text(content: &str) -> Result<()> {
        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new("wl-copy").stdin(Stdio::piped()).spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(content.as_bytes())?;
        }

        let status = child.wait()?;
        if status.success() {
            Ok(())
        } else {
            anyhow::bail!("wl-copy failed")
        }
    }

    #[cfg(target_os = "linux")]
    fn wl_copy_image(base64_data: &str) -> Result<()> {
        use std::io::Write;
        use std::process::Stdio;

        // Decode base64 to get PNG data
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
        if status.success() {
            Ok(())
        } else {
            anyhow::bail!("wl-copy image failed")
        }
    }

    #[cfg(target_os = "linux")]
    fn wl_copy_html(html: &str) -> Result<()> {
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
        if status.success() {
            Ok(())
        } else {
            anyhow::bail!("wl-copy html failed")
        }
    }
}
