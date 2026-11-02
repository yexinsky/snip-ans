use screenshots::Screen;
use image::{ColorType, ImageEncoder, RgbaImage, DynamicImage};
use image::codecs::jpeg::JpegEncoder;
use base64::{Engine as _, engine::general_purpose};

/// 截取主显示器画面，缩放并压缩为 JPEG 的 base64 字符串。
pub fn capture_primary_screen() -> Result<String, String> {
    let screens = Screen::all().map_err(|e| format!("枚举显示器失败：{}", e))?;

    // 优先取主屏，取不到再退回第一块屏
    let primary = screens
        .iter()
        .find(|s| s.display_info.is_primary)
        .or_else(|| screens.first())
        .ok_or_else(|| "未找到任何显示器".to_string())?;

    let image = primary.capture().map_err(|e| format!("截图失败：{}", e))?;

    let width = image.width();
    let height = image.height();
    let rgba = image.as_raw();

    // 构造 DynamicImage 以便缩放
    let img_buffer = RgbaImage::from_raw(width, height, rgba.clone())
        .ok_or_else(|| "创建图像缓冲区失败".to_string())?;
    let dynamic_image = DynamicImage::ImageRgba8(img_buffer);

    // 缩放到最长边 1280px，避免请求体过大；只缩不放
    let resized = dynamic_image.resize(1280, 1280, image::imageops::FilterType::Triangle);

    // JPEG 需要 RGB
    let rgb_image = resized.to_rgb8();

    let mut buffer = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut buffer, 80);
    encoder
        .write_image(
            rgb_image.as_raw(),
            rgb_image.width(),
            rgb_image.height(),
            ColorType::Rgb8.into(),
        )
        .map_err(|e| format!("JPEG 编码失败：{}", e))?;

    let base64_image = general_purpose::STANDARD.encode(&buffer);
    Ok(base64_image)
}
