use screenshots::Screen;
use std::error::Error;
use image::{ColorType, ImageEncoder, RgbaImage, DynamicImage};
use image::codecs::jpeg::JpegEncoder;
use base64::{Engine as _, engine::general_purpose};

pub fn capture_primary_screen() -> Result<String, String> {
    let screens = Screen::all().map_err(|e| e.to_string())?;
    let primary = screens.first().ok_or("No screen found".to_string())?; 
    let image = primary.capture().map_err(|e| e.to_string())?;
    
    let width = image.width();
    let height = image.height();
    let rgba = image.as_raw(); 

    // Create DynamicImage to resize
    let img_buffer = RgbaImage::from_raw(width, height, rgba.clone())
        .ok_or("Failed to create image buffer".to_string())?;
    let dynamic_image = DynamicImage::ImageRgba8(img_buffer);

    // Resize to reasonable max dimension (e.g. 1280px) to avoid Payload Too Large
    // OpenAI recommends 512-2048. 1280 is a good balance.
    let resized = dynamic_image.resize(1280, 1280, image::imageops::FilterType::Triangle);
    
    // JPEG requires RGB, not RGBA
    let rgb_image = resized.to_rgb8();

    let mut buffer = Vec::new();
    // Use JPEG for better compression
    let encoder = JpegEncoder::new_with_quality(&mut buffer, 80);
    encoder.write_image(rgb_image.as_raw(), rgb_image.width(), rgb_image.height(), ColorType::Rgb8.into())
        .map_err(|e| e.to_string())?;
    
    let base64_image = general_purpose::STANDARD.encode(&buffer);
    Ok(base64_image)
}
