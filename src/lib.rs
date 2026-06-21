use anyhow::Result;

use crate::layout::{Layout, is_svg};
use crate::render::render_layout;
use image::codecs::png::PngEncoder;
use image::{ColorType, DynamicImage, ImageEncoder, RgbaImage};
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

mod color;
mod components;
mod layout;
mod render;

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

pub fn render_to_image(
    source: impl IntoLayoutValue,
    img_base: &Path,
    bg_image: Option<String>,
) -> Result<DynamicImage> {
    let img = render_to_rgba(source, img_base, bg_image)?;
    Ok(DynamicImage::ImageRgba8(img))
}

pub fn render_to_png(
    source: impl IntoLayoutValue,
    img_base: &Path,
    bg_image: Option<String>,
) -> Result<Vec<u8>> {
    let image = render_to_rgba(source, img_base, bg_image)?;

    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ColorType::Rgba8.into(),
    )?;
    Ok(bytes)
}

fn render_to_rgba(
    source: impl IntoLayoutValue,
    img_base: &Path,
    bg_image: Option<String>,
) -> Result<RgbaImage> {
    let mut value = source.into_layout_value()?;
    fix_relative_paths(&mut value, img_base);

    let mut layout = Layout::deserialize(value).map_err(anyhow::Error::from)?;
    render_layout(&mut layout, bg_image)
}

/// Paths are either data:, a raw SVG string, or a path. This function attempts to locate
/// the paths and fully resolve them.
fn fix_relative_paths(value: &mut Value, base: &Path) {
    let Some(items) = value["items"].as_array_mut() else {
        return;
    };
    for item in items {
        if item["type"] == "pixmap"
            && let Some(v) = item["value"].as_str()
            && !v.starts_with("data:")
            && !is_svg(v)
        {
            item["value"] = Value::String(base.join(v).to_string_lossy().into_owned());
        }
    }
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

            let img = render_to_png(
                json,
                path.parent().expect("Failed to fetch parent path"),
                None,
            )
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
