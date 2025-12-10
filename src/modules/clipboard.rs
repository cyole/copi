use crate::modules::sync::ClipboardContent;
use anyhow::Result;
use arboard::{Clipboard, ImageData};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::time::sleep;

#[cfg(target_os = "linux")]
use std::process::Command;

// 图片大小限制：10MB
const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024;
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
                    // Convert ImageData to PNG and encode as base64
                    let png_data = Self::image_data_to_png(&img)?;
                    let base64_data = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &png_data,
                    );

                    Ok(ClipboardContent::Image {
                        data: base64_data,
                        width: img.width as u32,
                        height: img.height as u32,
                    })
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
                if let Ok(img_data) = Self::wl_paste_image() {
                    Ok(img_data)
                } else {
                    // Fall back to text
                    Self::wl_paste().map(ClipboardContent::Text)
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
            Err(_) => Ok(None),
        }
    }

    fn image_data_to_png(img: &ImageData) -> Result<Vec<u8>> {
        use image::{DynamicImage, ImageBuffer, RgbaImage};
        use std::io::Cursor;

        let width = img.width as u32;
        let height = img.height as u32;

        // 检查图片尺寸
        if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
            anyhow::bail!(
                "Image dimensions too large: {}x{} (max: {}x{})",
                width,
                height,
                MAX_IMAGE_DIMENSION,
                MAX_IMAGE_DIMENSION
            );
        }

        // Convert ImageData bytes to RgbaImage
        let img_buffer: RgbaImage = ImageBuffer::from_raw(width, height, img.bytes.to_vec())
            .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

        // 如果图片太大，进行缩放
        let mut dynamic_img = DynamicImage::ImageRgba8(img_buffer);
        let estimated_size = width as usize * height as usize * 4;

        if estimated_size > MAX_IMAGE_SIZE {
            let scale = (MAX_IMAGE_SIZE as f64 / estimated_size as f64).sqrt();
            let new_width = (width as f64 * scale) as u32;
            let new_height = (height as f64 * scale) as u32;

            println!(
                "Resizing image from {}x{} to {}x{} to fit size limit",
                width, height, new_width, new_height
            );

            dynamic_img =
                dynamic_img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3);
        }

        // Encode as PNG with compression
        let mut png_data = Vec::new();
        let mut cursor = Cursor::new(&mut png_data);

        dynamic_img.write_to(&mut cursor, image::ImageFormat::Png)?;

        // 再次检查编码后的大小
        if png_data.len() > MAX_IMAGE_SIZE {
            anyhow::bail!(
                "Encoded image too large: {} bytes (max: {} bytes)",
                png_data.len(),
                MAX_IMAGE_SIZE
            );
        }

        Ok(png_data)
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
            // Decode the PNG to get dimensions
            use image::ImageReader;
            use std::io::Cursor;

            let img = ImageReader::new(Cursor::new(&output.stdout))
                .with_guessed_format()?
                .decode()?;

            let width = img.width();
            let height = img.height();

            // Encode as base64
            let base64_data =
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &output.stdout);

            Ok(ClipboardContent::Image {
                data: base64_data,
                width,
                height,
            })
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

    pub async fn monitor<F>(&mut self, mut callback: F) -> Result<()>
    where
        F: FnMut(ClipboardContent) -> Result<()>,
    {
        loop {
            if let Some(content) = self.get_clipboard_content()? {
                callback(content)?;
            }
            sleep(Duration::from_millis(500)).await;
        }
    }
}
