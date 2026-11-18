//! 1600x1600 封面: 裁剪场照, 三种排版, 再叠标题.

use std::fs;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, Rgba, RgbaImage};
use serde_json::Value;

use crate::error::{StudioError, StudioResult};
use crate::ffmpeg::{self, Binaries, Cancel};

pub const SIZE: u32 = 1600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Ivory,
    Dark,
    Photo,
}

impl Layout {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ivory" => Some(Self::Ivory),
            "dark" => Some(Self::Dark),
            "photo" => Some(Self::Photo),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Ivory => "ivory",
            Self::Dark => "dark",
            Self::Photo => "photo",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ivory => "象牙白底 · 衬线标题",
            Self::Dark => "深色底 · 浅色衬线标题",
            Self::Photo => "通幅场照 · 左下角文字",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CoverSpec {
    pub layout: Layout,
    pub title: String,
    pub performer: String,
    pub detail: String,
    pub zoom: f32,
    pub horizontal: f32,
    pub vertical: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverDesign {
    pub title: String,
    pub performer: String,
    pub detail: String,
    pub layout: Layout,
}

pub fn parse_design(text: &str) -> StudioResult<CoverDesign> {
    let value = design_json(text)?;
    let layout = value.get("layout").and_then(|item| item.as_str()).and_then(Layout::parse).ok_or_else(|| StudioError::new("模型没有返回有效的封面方案, 请重试."))?;
    let clip = |key: &str, max: usize| value.get(key).and_then(|item| item.as_str()).unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ").chars().take(max).collect::<String>();
    let title = clip("title", 120);
    if title.is_empty() {
        return Err(StudioError::new("模型没有返回有效的封面方案, 请重试."));
    }
    Ok(CoverDesign { title, performer: clip("performer", 120), detail: clip("detail", 150), layout })
}

pub fn prepare(path: &Path, binaries: Option<&Binaries>) -> StudioResult<(PathBuf, String)> {
    let name = path.file_name().map(|item| item.to_string_lossy().to_string()).unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    let len = path.metadata().map(|item| item.len()).unwrap_or(0);
    if len == 0 || len > 30 * 1024 * 1024 {
        return Err(StudioError::new("请选择有效的 PNG, JPG, WebP, HEIC 或 HEIF 图片, 并核对文件大小."));
    }
    if lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        let _ = load_raster(path)?;
        return Ok((path.to_path_buf(), name));
    }
    if lower.ends_with(".webp") {
        let image = load_raster(path)?;
        let stem = path.file_stem().map(|item| item.to_string_lossy().to_string()).unwrap_or_else(|| "cover".into());
        let out = crate::store::temp_dir().join(format!("{stem}.png"));
        fs::create_dir_all(out.parent().unwrap_or(Path::new(".")))?;
        write_png(&image.to_rgba8(), &out)?;
        return Ok((out, format!("{stem}.png")));
    }
    if lower.ends_with(".heic") || lower.ends_with(".heif") {
        let Some(binaries) = binaries else {
            return Err(StudioError::new("转换 HEIC / HEIF 需要裁剪版 FFmpeg."));
        };
        let stem = path.file_stem().map(|item| item.to_string_lossy().to_string()).unwrap_or_else(|| "cover".into());
        let out = crate::store::temp_dir().join(format!("{stem}-converted.png"));
        fs::create_dir_all(out.parent().unwrap_or(Path::new(".")))?;
        ffmpeg::run(
            &binaries.ffmpeg,
            &[
                "-hide_banner".into(),
                "-nostdin".into(),
                "-y".into(),
                "-v".into(),
                "error".into(),
                "-i".into(),
                path.display().to_string(),
                out.display().to_string(),
            ],
            false,
            None,
            &Cancel::default(),
        )
        .map_err(|_| StudioError::new("HEIC / HEIF 转换失败. 请确认图片可以打开, 或先另存为 PNG."))?;
        if !out.is_file() {
            return Err(StudioError::new("HEIC / HEIF 转换失败. 请确认图片可以打开, 或先另存为 PNG."));
        }
        if out.metadata().map(|item| item.len()).unwrap_or(0) > 30 * 1024 * 1024 {
            return Err(StudioError::new("转换后的图片超过 30 MB, 请缩小原图后重试."));
        }
        return Ok((out, format!("{stem}.png")));
    }
    Err(StudioError::new("请选择有效的 PNG, JPG, WebP, HEIC 或 HEIF 图片, 并核对文件大小."))
}

pub fn load_raster(path: &Path) -> StudioResult<DynamicImage> {
    image::ImageReader::open(path)
        .map_err(|error| StudioError::new(format!("无法读取图片: {error}")))?
        .with_guessed_format()
        .map_err(|error| StudioError::new(format!("无法读取图片: {error}")))?
        .decode()
        .map_err(|_| StudioError::new("无法读取这张图片. 请换 PNG, JPG 或 WebP."))
}

pub fn render(photo: &DynamicImage, spec: &CoverSpec, size: u32) -> RgbaImage {
    let size = size.max(32);
    let (paper, ink, photo_rect, align_center, shadow) = match spec.layout {
        Layout::Ivory => (Some([244, 238, 226]), [33, 27, 23], [0, 0, size, size * 1130 / 1600], true, false),
        Layout::Dark => (Some([36, 33, 31]), [255, 248, 238], [0, 0, size, size * 1130 / 1600], true, false),
        Layout::Photo => (None, [255, 250, 241], [0, 0, size, size], false, true),
    };
    let mut canvas = if let Some(color) = paper {
        RgbaImage::from_pixel(size, size, Rgba([color[0], color[1], color[2], 255]))
    } else {
        RgbaImage::from_pixel(size, size, Rgba([20, 16, 14, 255]))
    };
    paste_photo(&mut canvas, photo, photo_rect, spec.zoom, spec.horizontal, spec.vertical);
    let width = (size as f32 * 0.85) as i32;
    let x = if align_center { (size as i32 - width) / 2 } else { (size as f32 * 0.075) as i32 };
    let mut y = (size as f32 * if spec.layout == Layout::Photo { 1200.0 } else { 1255.0 } / 1600.0) as i32;
    let gap = scale(size, 5) as i32;
    y = draw_block(&mut canvas, spec.title.trim(), x, y, width, scale(size, 77), ink, true, false, shadow, align_center);
    y = draw_block(&mut canvas, spec.performer.trim(), x, y + gap, width, scale(size, 42), ink, false, true, shadow, align_center);
    let _ = draw_block(&mut canvas, spec.detail.trim(), x, y + gap, width, scale(size, 28), ink, false, false, shadow, align_center);
    canvas
}

pub fn paste_photo(canvas: &mut RgbaImage, photo: &DynamicImage, rect: [u32; 4], zoom: f32, horizontal: f32, vertical: f32) {
    let [x, y, width, height] = rect;
    if width == 0 || height == 0 || photo.width() == 0 || photo.height() == 0 {
        return;
    }
    let zoom = zoom.clamp(1.0, 3.0);
    let scale = (width as f32 / photo.width() as f32).max(height as f32 / photo.height() as f32) * zoom;
    let crop_w = ((width as f32 / scale).round() as u32).clamp(1, photo.width());
    let crop_h = ((height as f32 / scale).round() as u32).clamp(1, photo.height());
    let left = (((photo.width() - crop_w) as f32) * horizontal.clamp(0.0, 1.0)).round() as u32;
    let top = (((photo.height() - crop_h) as f32) * vertical.clamp(0.0, 1.0)).round() as u32;
    let left = left.min(photo.width() - crop_w);
    let top = top.min(photo.height() - crop_h);
    let cropped = photo.crop_imm(left, top, crop_w, crop_h);
    let scaled = cropped.resize_exact(width, height, FilterType::Triangle);
    image::imageops::overlay(canvas, &scaled.to_rgba8(), x as i64, y as i64);
}

fn scale(size: u32, value: u32) -> f32 {
    (size as f32 * value as f32 / 1600.0).max(8.0)
}

fn draw_block(canvas: &mut RgbaImage, text: &str, x: i32, y: i32, width: i32, font: f32, ink: [u8; 3], bold: bool, italic: bool, shadow: bool, center: bool) -> i32 {
    if text.is_empty() || width <= 0 {
        return y;
    }
    let line = (font * 1.15).ceil() as i32;
    if shadow {
        paint_text(canvas, text, x, y + 2, width, font, [21, 18, 14], bold, italic, center);
    }
    let lines = paint_text(canvas, text, x, y, width, font, ink, bold, italic, center).max(1);
    y + lines * line
}

fn paint_text(canvas: &mut RgbaImage, text: &str, x: i32, y: i32, width: i32, font: f32, ink: [u8; 3], bold: bool, italic: bool, center: bool) -> i32 {
    use cosmic_text::{Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, Weight, Wrap};
    let mut fonts = FontSystem::new();
    let mut cache = cosmic_text::SwashCache::new();
    let metrics = Metrics::new(font, font * 1.15);
    let mut buffer = Buffer::new(&mut fonts, metrics);
    buffer.set_size(&mut fonts, Some(width as f32), Some(font * 1.15 * 2.4));
    buffer.set_wrap(&mut fonts, Wrap::Word);
    let mut attrs = Attrs::new().family(Family::Serif);
    if bold {
        attrs = attrs.weight(Weight::BOLD);
    }
    if italic {
        attrs = attrs.style(Style::Italic);
    }
    buffer.set_text(&mut fonts, text, attrs, Shaping::Advanced);
    let align = if center { Align::Center } else { Align::Left };
    for line in &mut buffer.lines {
        line.set_align(Some(align));
    }
    buffer.shape_until_scroll(&mut fonts, false);
    let color = Color::rgba(ink[0], ink[1], ink[2], 255);
    buffer.draw(&mut fonts, &mut cache, color, |px, py, w, h, pixel| {
        for dy in 0..h {
            for dx in 0..w {
                blend(canvas, x + px + dx as i32, y + py + dy as i32, pixel);
            }
        }
    });
    buffer.lines.len().max(1) as i32
}

fn blend(canvas: &mut RgbaImage, x: i32, y: i32, color: cosmic_text::Color) {
    if x < 0 || y < 0 || x as u32 >= canvas.width() || y as u32 >= canvas.height() {
        return;
    }
    let alpha = color.a() as f32 / 255.0;
    if alpha <= 0.0 {
        return;
    }
    let pixel = canvas.get_pixel_mut(x as u32, y as u32);
    for (index, channel) in [color.r(), color.g(), color.b()].into_iter().enumerate() {
        pixel.0[index] = (channel as f32 * alpha + pixel.0[index] as f32 * (1.0 - alpha)).round() as u8;
    }
    pixel.0[3] = 255;
}

fn design_json(text: &str) -> StudioResult<Value> {
    let trimmed = text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if let Ok(value) = serde_json::from_str::<Value>(&trimmed[start..=end]) {
            return Ok(value);
        }
    }
    Err(StudioError::new("模型没有返回有效的封面方案, 请重试."))
}

pub fn write_png(image: &RgbaImage, path: &Path) -> StudioResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    image.save(path).map_err(|error| StudioError::new(format!("无法保存封面: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_crop_keeps_the_chosen_side() {
        let mut photo = RgbaImage::new(100, 20);
        for (x, _y, pixel) in photo.enumerate_pixels_mut() {
            *pixel = if x < 50 { Rgba([220, 20, 20, 255]) } else { Rgba([20, 20, 220, 255]) };
        }
        let photo = DynamicImage::ImageRgba8(photo);
        let mut canvas = RgbaImage::from_pixel(40, 20, Rgba([0, 0, 0, 255]));
        paste_photo(&mut canvas, &photo, [0, 0, 40, 20], 1.0, 1.0, 0.5);
        let pixel = canvas.get_pixel(20, 10);
        assert!(pixel.0[2] > pixel.0[0], "right side of the photo should fill the frame");
    }

    #[test]
    fn design_rejects_an_unknown_layout() {
        assert!(parse_design(r#"{"title":"Recital","performer":"","detail":"","layout":"neon"}"#).is_err());
        let design = parse_design(r#"{"title":"Recital","performer":"A","detail":"Hall","layout":"dark"}"#).unwrap();
        assert_eq!(design.layout, Layout::Dark);
        assert_eq!(design.title, "Recital");
    }

    #[test]
    fn render_keeps_the_square() {
        let photo = DynamicImage::ImageRgba8(RgbaImage::from_pixel(30, 20, Rgba([180, 40, 40, 255])));
        let image = render(
            &photo,
            &CoverSpec { layout: Layout::Ivory, title: "Recital".into(), performer: "Piano".into(), detail: "Hall".into(), zoom: 1.0, horizontal: 0.5, vertical: 0.5 },
            64,
        );
        assert_eq!((image.width(), image.height()), (64, 64));
        assert_eq!(image.get_pixel(2, 60).0[0], 244);
    }
}
