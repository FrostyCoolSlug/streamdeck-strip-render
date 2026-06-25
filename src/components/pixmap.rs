use crate::STRICT_RENDER;
use crate::color::{parse_gradient, with_opacity};
use crate::layout::{PixmapItem, PixmapSource, Rect};
use crate::render::{FillStyle, blend, fill_rect, is_valid_rect};
use image::{Rgba, RgbaImage, imageops};
use log::warn;
use resvg::usvg::fontdb;
use std::sync::{Arc, OnceLock};

static FONT_DATABASE: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
fn font_database() -> Arc<fontdb::Database> {
    FONT_DATABASE
        .get_or_init(|| {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

pub(crate) fn render_pixmap(canvas: &mut RgbaImage, item: &PixmapItem) {
    let rect = &item.common.rect;

    if !is_valid_rect(rect, canvas) {
        warn!(
            "Rect Extends Outside Canvas for {} - {:?}",
            item.common.key, rect
        );

        if STRICT_RENDER {
            return;
        }
    }

    // Render the background
    let style = FillStyle {
        gradient: &parse_gradient(&item.common.background),
        opacity: item.common.opacity,
    };
    fill_rect(canvas, rect, &style);

    // Load and draw the image, or draw the checkerboard
    match load_pixmap_source(&item.value, rect) {
        Some(img) => blit_image(canvas, &img, rect, item.common.opacity),
        None => draw_checkerboard(canvas, rect, item.common.opacity),
    }
}

fn load_pixmap_source(source: &PixmapSource, rect: &Rect) -> Option<RgbaImage> {
    match source {
        PixmapSource::None => None,

        PixmapSource::File(path) => image::open(path)
            .ok()
            .map(|i| resize_to_rect(i.to_rgba8(), rect)),

        PixmapSource::Bytes(bytes) => {
            let img = image::load_from_memory(bytes).ok()?.to_rgba8();
            Some(resize_to_rect(img, rect))
        }

        PixmapSource::Svg(svg) => {
            use resvg::tiny_skia;
            use resvg::usvg;

            let opt = usvg::Options {
                fontdb: font_database(),
                ..Default::default()
            };

            let tree = usvg::Tree::from_str(svg, &opt).ok()?;
            let mut pixmap = tiny_skia::Pixmap::new(rect.width, rect.height)?;

            // We'll render the SVG at the rect size, so we don't need to do additional resizing
            // TODO: Do we need to scale to rect, or only reduce to rect?
            // TODO: Should we maintain aspect ratio?
            let sx = rect.width as f32 / tree.size().width();
            let sy = rect.height as f32 / tree.size().height();
            resvg::render(
                &tree,
                usvg::Transform::from_scale(sx, sy),
                &mut pixmap.as_mut(),
            );

            RgbaImage::from_raw(rect.width, rect.height, pixmap.data().to_vec())
        }
    }
}

/// Resizes a RGBAImage to match the given rect.
/// TODO: Do we need to scale to rect, or only reduce to rect?
/// TODO: Should we maintain aspect ratio?
fn resize_to_rect(img: RgbaImage, rect: &Rect) -> RgbaImage {
    let (w, h) = img.dimensions();

    // Are we already the correct size?
    if w == rect.width && h == rect.height {
        return img;
    }

    imageops::resize(
        &img,
        rect.width,
        rect.height,
        imageops::FilterType::Lanczos3,
    )
}

/// This blits the loaded image onto our canvas, applying opacity as it goes
fn blit_image(canvas: &mut RgbaImage, src: &RgbaImage, rect: &Rect, opacity: f32) {
    debug_assert_eq!(
        src.dimensions(),
        (rect.width, rect.height),
        "blit source dimensions mismatch — resize before blitting"
    );

    for py in 0..rect.height {
        for px in 0..rect.width {
            let cx = rect.x + px;
            let cy = rect.y + py;

            if cx >= canvas.width() || cy >= canvas.height() {
                continue;
            }

            let mut p = *src.get_pixel(px, py);
            p[3] = (p[3] as f32 * opacity) as u8;
            let dst = *canvas.get_pixel(cx, cy);
            canvas.put_pixel(cx, cy, blend(dst, p));
        }
    }
}

/// This is essentially a fallback, if an image is not defined, or we can't load the image
/// we replace it with a checkerboard in the location the image should be.
/// TODO: We should probably not draw at all, but this helps debugging
fn draw_checkerboard(canvas: &mut RgbaImage, rect: &Rect, opacity: f32) {
    let c1 = with_opacity(Rgba([80, 80, 80, 128]), opacity);
    let c2 = with_opacity(Rgba([48, 48, 48, 128]), opacity);
    let tile = 8u32;
    for py in 0..rect.height {
        for px in 0..rect.width {
            let cx = rect.x + px;
            let cy = rect.y + py;
            if cx >= canvas.width() || cy >= canvas.height() {
                continue;
            }
            let c = if (px / tile + py / tile).is_multiple_of(2) {
                c1
            } else {
                c2
            };
            let dst = *canvas.get_pixel(cx, cy);
            canvas.put_pixel(cx, cy, blend(dst, c));
        }
    }
}
