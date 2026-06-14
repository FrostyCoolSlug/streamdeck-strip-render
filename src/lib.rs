use anyhow::{Result, anyhow};

use crate::layout::Layout;
use crate::render::render_layout;
use image::codecs::png::PngEncoder;
use image::{ColorType, DynamicImage, ImageEncoder};
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

pub fn get_dynamic_from_layout_str(
    json: &str,
    img_base: &Path,
    bg_image: Option<String>,
) -> Result<DynamicImage> {
    let mut layout = serde_json::from_str(json).map_err(|e| anyhow!(e))?;
    let img = render_layout(&mut layout, img_base, bg_image)?;
    Ok(DynamicImage::ImageRgba8(img))
}

pub fn get_dynamic_from_layout_value(
    layout: &Value,
    img_base: &Path,
    bg_image: Option<String>,
) -> Result<DynamicImage> {
    let mut layout = Layout::deserialize(layout).map_err(|e| anyhow!(e))?;
    let img = render_layout(&mut layout, img_base, bg_image)?;
    Ok(DynamicImage::ImageRgba8(img))
}

pub fn get_png_from_layout_str(
    json: &str,
    img_base: &Path,
    bg_image: Option<String>,
) -> Result<Vec<u8>> {
    let mut layout = serde_json::from_str(json).map_err(|e| anyhow!(e))?;
    get_png_from_layout(&mut layout, img_base, bg_image)
}

pub fn get_png_from_layout_value(
    layout: &Value,
    img_base: &Path,
    bg_image: Option<String>,
) -> Result<Vec<u8>> {
    let mut layout = Layout::deserialize(layout).map_err(|e| anyhow!(e))?;
    get_png_from_layout(&mut layout, img_base, bg_image)
}

fn get_png_from_layout(
    layout: &mut Layout,
    img_base: &Path,
    bg_image: Option<String>,
) -> Result<Vec<u8>> {
    let image = render_layout(layout, img_base, bg_image)?;

    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ColorType::Rgba8.into(),
    )?;
    Ok(bytes)
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

            let img = get_png_from_layout_str(
                &json,
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
