use anyhow::Result;

use crate::layout::Layout;
use crate::render::render_layout;
use crate::strip_renderer::StripRenderer;
use image::codecs::png::PngEncoder;
use image::{ColorType, DynamicImage, ImageEncoder, RgbaImage};
use serde::Deserialize;
use serde_json::Value;

mod color;
mod components;
pub mod layout;
mod render;
pub mod strip_renderer;

pub(crate) static FONT_SANS: &[u8] = include_bytes!("../resources/fonts/noto/NotoSans.ttf");
pub(crate) static FONT_SERIF: &[u8] = include_bytes!("../resources/fonts/noto/NotoSerif.ttf");
pub(crate) static FONT_MONO: &[u8] = include_bytes!("../resources/fonts/noto/NotoSansMono.ttf");

#[cfg(feature = "strict-rendering")]
const STRICT_RENDER: bool = true;
#[cfg(not(feature = "strict-rendering"))]
const STRICT_RENDER: bool = false;

/// A value that can be used as a layout source.
/// Accepts either a JSON string or an existing `serde_json::Value`.
pub trait IntoLayoutValue {
    fn into_layout_value(self) -> Result<Value>;
}

impl IntoLayoutValue for &str {
    fn into_layout_value(self) -> Result<Value> {
        serde_json::from_str(self).map_err(anyhow::Error::from)
    }
}

impl IntoLayoutValue for String {
    fn into_layout_value(self) -> Result<Value> {
        serde_json::from_str(&self).map_err(anyhow::Error::from)
    }
}

impl IntoLayoutValue for Value {
    fn into_layout_value(self) -> Result<Value> {
        Ok(self)
    }
}

pub fn get_incremental_renderer(
    source: impl IntoLayoutValue,
    _bg_image: Option<String>,
) -> Result<StripRenderer> {
    let value = source.into_layout_value()?;
    let layout = Layout::deserialize(value).map_err(anyhow::Error::from)?;

    StripRenderer::from(layout)
}

pub fn render_to_image(
    source: impl IntoLayoutValue,
    bg_image: Option<String>,
) -> Result<DynamicImage> {
    let img = render_to_rgba(source, bg_image)?;
    Ok(DynamicImage::ImageRgba8(img))
}

pub fn render_to_png(source: impl IntoLayoutValue, bg_image: Option<String>) -> Result<Vec<u8>> {
    let image = render_to_rgba(source, bg_image)?;

    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ColorType::Rgba8.into(),
    )?;
    Ok(bytes)
}

fn render_to_rgba(source: impl IntoLayoutValue, bg_image: Option<String>) -> Result<RgbaImage> {
    let value = source.into_layout_value()?;

    let mut layout = Layout::deserialize(value).map_err(anyhow::Error::from)?;
    render_layout(&mut layout, bg_image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::info;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Once;

    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| {
            env_logger::Builder::new()
                .filter_level(log::LevelFilter::Debug)
                .is_test(true)
                .init();
        });
    }

    /// Make sure the output path exists
    fn test_output_path(file_name: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("target");
        path.push("test-output");
        fs::create_dir_all(&path).expect("failed to create test output directory");
        path.push(file_name);
        path
    }

    /// Save the returned image to a file
    fn save_test_image(file_name: &str, img: &Vec<u8>) {
        let path = test_output_path(file_name);

        fs::write(&path, img)
            .unwrap_or_else(|e| panic!("failed to save test output {:?}: {}", path, e));
    }

    #[test]
    fn parse_layouts() {
        init();

        let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let test_paths = base_path.join("test-data/");

        let paths = find_json_files(&test_paths);

        for path in paths {
            info!("Testing {:?}.. ", path);

            let json = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read file {:?}: {}", path, e));

            let img = render_to_png(json, None)
                .unwrap_or_else(|e| panic!("Failed to parse layout {:?}: {}", path, e));

            let output_file = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("Failed to get file stem");

            save_test_image(format!("{}.png", output_file).as_str(), &img);
        }
    }

    fn find_json_files(dir: &PathBuf) -> Vec<PathBuf> {
        let mut paths = vec![];

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    paths.extend(find_json_files(&path));
                } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    paths.push(path);
                }
            }
        }

        paths
    }
}
