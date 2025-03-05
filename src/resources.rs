use anyhow::{Context, Result};

pub static APP_NAME: &str = "iRacingMonitor";
pub const ICON_BYTES: &[u8] = include_bytes!("../resources/icon.png");
pub const ICON_DISCONNECTED_BYTES: &[u8] = include_bytes!("../resources/icon_disconnected.png");

pub fn load_as_rgba(image_bytes: &[u8]) -> Result<image::RgbaImage> {
    let icon = image::ImageReader::new(std::io::Cursor::new(image_bytes));
    let icon_with_format = icon
        .with_guessed_format()
        .context("Failed to guess image format")?;
    let pixels = icon_with_format
        .decode()
        .context("Failed to decode image")?
        .to_rgba8();
    Ok(pixels)
}
