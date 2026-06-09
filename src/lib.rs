use crate::layout::Layout;
use crate::render::render_layout;
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder, RgbaImage};
use std::error::Error;

mod color;
mod components;
mod layout;
mod render;

/// If true, we'll error on overlaps and not draw items that expand passed the canvas edges
/// If false, we'll draw the items anyway, but it will be clipped to the canvas edges
const FULL_VALIDATION: bool = false;

pub fn parse_layout(json: &str) -> Result<Layout, Box<dyn Error>> {
    serde_json::from_str(json).map_err(|err| err.into())
}

pub fn render_from_layout(layout: &Layout) -> Result<RgbaImage, Box<dyn Error>> {
    render_layout(layout)
}

pub fn get_png_from_layout(json: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let layout = parse_layout(json)?;
    let image = render_layout(&layout)?;

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
    use crate::render::render_layout;
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
    fn save_test_canvas(file_name: &str, canvas: &RgbaImage) {
        let path = test_output_path(file_name);
        canvas
            .save(&path)
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
            let layout = parse_layout(&json)
                .unwrap_or_else(|e| panic!("Failed to parse layout {:?}: {}", path, e));

            let output_file = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("Failed to get file stem");

            save_test_canvas(
                format!("{}.png", output_file).as_str(),
                &render_layout(&layout).expect("Failed to render layout"),
            );
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
