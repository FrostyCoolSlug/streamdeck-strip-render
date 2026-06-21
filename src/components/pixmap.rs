use crate::STRICT_RENDER;
use crate::color::{parse_gradient, with_opacity};
use crate::layout::{PixmapItem, PixmapSource, Rect};
use crate::render::{CANVAS_H, CANVAS_W, FillStyle, blend, fill_rect, is_valid_rect};
use image::{Rgba, RgbaImage, imageops};
use log::warn;

pub(crate) fn render_pixmap(canvas: &mut RgbaImage, item: &PixmapItem) {
    let rect = &item.common.rect;

    if !is_valid_rect(rect) {
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

        PixmapSource::File(path) => {
            if path.ends_with(".svg") {
                // Use an SVG renderer here instead of image
                let opt = usvg::Options::default();
                let data = std::fs::read(path).ok()?;
                let tree = usvg::Tree::from_data(&data, &opt).ok()?;
                let (w, h) = (rect.width, rect.height);
                let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
                let transform = tiny_skia::Transform::from_scale(
                    w as f32 / tree.size().width(),
                    h as f32 / tree.size().height(),
                );
                resvg::render(&tree, transform, &mut pixmap.as_mut());
                let img = RgbaImage::from_raw(w, h, pixmap.take())?;
                return Some(img);
            }

            image::open(path)
                .ok()
                .map(|i| resize_to_rect(i.to_rgba8(), rect))
        }

        PixmapSource::Base64(data) => {
            use base64::Engine;

            let (header, payload) = data.split_once(',')?;
            if !header.contains("base64") {
                return None;
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(payload)
                .ok()?;
            let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
            Some(resize_to_rect(img, rect))
        }

        PixmapSource::Svg(svg) => {
            use resvg::tiny_skia;
            use resvg::usvg;

            let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
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

            if cx >= CANVAS_W || cy >= CANVAS_H {
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
            if cx >= CANVAS_W || cy >= CANVAS_H {
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
