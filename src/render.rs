//! This code should be able to render a layout onto a 200x100 canvas, it doesn't quite cover
//! everything yet, but should be good enough to get a base render

use crate::color::{GradientStop, parse_color, parse_gradient, sample_gradient, with_opacity};
use crate::layout::{
    BarCommon, BarItem, BarSubtype, CommonFields, GBarItem, Layout, LayoutItem, PixmapItem,
    PixmapSource, Range, Rect, TextAlignment, TextItem, TextOverflow,
};
use ab_glyph::{Font, FontVec, PxScale, PxScaleFont, ScaleFont, VariableFont};
use image::{ImageBuffer, Rgba, RgbaImage, imageops};
use std::error::Error;
use std::sync::{LazyLock, Mutex};

type Gradient = Vec<GradientStop>;

pub const CANVAS_W: u32 = 200;
pub const CANVAS_H: u32 = 100;

static DEFAULT_FONT: LazyLock<Mutex<FontVec>> = LazyLock::new(|| {
    static BUNDLED: &[u8] = include_bytes!("../resources/fonts/InterVariable.ttf");
    let font = FontVec::try_from_vec(BUNDLED.to_vec()).expect("Bundled font is corrupt");
    Mutex::new(font)
});

/// Render `layout` onto a fresh 200×100 black canvas and return it.
pub fn render_layout(layout: &Layout) -> Result<RgbaImage, Box<dyn Error>> {
    // Create a canvas and begin the work (black by default, should we be transparent?)
    let mut canvas: RgbaImage = ImageBuffer::from_pixel(CANVAS_W, CANVAS_H, Rgba([0, 0, 0, 255]));
    let mut items: Vec<&LayoutItem> = layout.items.iter().collect();

    // We order stuff by z-index, so things draw over things :)
    items.sort_by_key(|i| i.z_order());

    // Draw the items
    for item in items {
        if !item.enabled() {
            continue;
        }
        match item {
            LayoutItem::Text(t) => render_text(&mut canvas, t),
            LayoutItem::Pixmap(p) => render_pixmap(&mut canvas, p),
            LayoutItem::Bar(b) => render_bar(&mut canvas, b),
            LayoutItem::GBar(g) => render_gbar(&mut canvas, g),
        }
    }

    Ok(canvas)
}

/// Alpha-composite `src` over `dst`.
#[inline]
fn blend(dst: Rgba<u8>, src: Rgba<u8>) -> Rgba<u8> {
    // Short circuit this if we're fully opaque
    if src[3] == 255 {
        return src;
    }

    let src_alpha = src[3] as f32 / 255.0;
    let dst_alpha = dst[3] as f32 / 255.0;

    let out_a = src_alpha + dst_alpha * (1.0 - src_alpha);
    if out_a < 1e-5 {
        return Rgba([0, 0, 0, 0]);
    }

    // MAAAAAAAAAAATHS, blend based on opacity
    let r = (src[0] as f32 * src_alpha + dst[0] as f32 * dst_alpha * (1.0 - src_alpha)) / out_a;
    let g = (src[1] as f32 * src_alpha + dst[1] as f32 * dst_alpha * (1.0 - src_alpha)) / out_a;
    let b = (src[2] as f32 * src_alpha + dst[2] as f32 * dst_alpha * (1.0 - src_alpha)) / out_a;
    Rgba([r as u8, g as u8, b as u8, (out_a * 255.0) as u8])
}

/// Fill a rectangle with either a solid colour or a gradient.
fn fill_rect(canvas: &mut RgbaImage, rect: &Rect, style: &FillStyle) {
    if style.opacity == 0.0 {
        return;
    }

    let clip_x = (rect.x + rect.width).min(CANVAS_W);
    let clip_y = (rect.y + rect.height).min(CANVAS_H);

    let is_solid = style.gradient.len() == 1;
    let solid_colour = is_solid.then(|| with_opacity(style.gradient[0].color, style.opacity));

    for px in rect.x..clip_x {
        let colour = resolve_colour(style, px, rect, solid_colour);

        for py in rect.y..clip_y {
            put_blended(canvas, px, py, colour);
        }
    }
}

/// Draw a border (inset by `bw` pixels on each side) around a rect.
fn draw_border(canvas: &mut RgbaImage, rect: &Rect, colour: Rgba<u8>, bw: u32) {
    let style = FillStyle {
        gradient: &vec![GradientStop {
            color: colour,
            offset: 0.0,
        }],
        opacity: 1.0,
    };

    for b in 0..bw {
        // Top Line
        let top_rect = Rect {
            x: rect.x,
            y: rect.y + b,
            width: rect.width,
            height: 1,
        };
        fill_rect(canvas, &top_rect, &style);

        // Bottom Line
        let bottom_rect = Rect {
            x: rect.x,
            y: rect.y + rect.height - 1 - b,
            width: rect.width,
            height: 1,
        };
        fill_rect(canvas, &bottom_rect, &style);

        // Left Line
        let left_rec = Rect {
            x: rect.x + b,
            y: rect.y,
            width: 1,
            height: rect.height,
        };
        fill_rect(canvas, &left_rec, &style);

        // Right Line
        let right_rec = Rect {
            x: rect.x + rect.width - 1 - b,
            y: rect.y,
            width: 1,
            height: rect.height,
        };
        fill_rect(canvas, &right_rec, &style);
    }
}

// -------------- TEXT RENDERING ----------------------
fn render_text(canvas: &mut RgbaImage, item: &TextItem) {
    let rect = item.common.rect;

    // Fill the background
    let style = FillStyle {
        gradient: &parse_gradient(&item.common.background),
        opacity: item.common.opacity,
    };
    fill_rect(canvas, &rect, &style);

    // If there's no text, don't bother rendering
    if item.value().is_empty() {
        return;
    }

    let text = item.value();
    let color_str = &item.color;

    let fg = with_opacity(parse_color(color_str), item.common.opacity);

    let alignment = item.alignment;
    let overflow = item.text_overflow;

    // Grab and configure the font for this render
    let mut font = DEFAULT_FONT.lock().unwrap();
    font.set_variation(b"wght", item.font.weight as f32);
    let scaled = font.as_scaled(PxScale::from(item.font.size));

    // ascent/descent are now direct methods on the scaled font
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let text_height = (ascent - descent).ceil();
    let text_width = measure_text(&scaled, &text);

    let display_text = if text_width > rect.width as f32 {
        match overflow {
            TextOverflow::Ellipsis => truncate_with_ellipsis(&scaled, &text, rect.width),
            TextOverflow::Clip => text,
            TextOverflow::Fade => text,
        }
    } else {
        text
    };

    let text_x = match alignment {
        TextAlignment::Left => rect.x as f32,
        TextAlignment::Center => {
            rect.x as f32 + (rect.width as f32 - measure_text(&scaled, &display_text)) / 2.0
        }
        TextAlignment::Right => (rect.x + rect.width) as f32 - measure_text(&scaled, &display_text),
    };

    let text_y = rect.y as f32 + (rect.height as f32 - text_height) / 2.0 + ascent;

    draw_glyphs_clipped(canvas, &scaled, &display_text, text_x, text_y, fg, rect);
}

/// Calculates how much space is required to render the given text, in pixels.
fn measure_text(scaled: &PxScaleFont<&FontVec>, text: &str) -> f32 {
    let mut width = 0.0f32;
    let mut last_glyph_id = None;

    for ch in text.chars() {
        let glyph_id = scaled.glyph_id(ch);

        // Apply kerning between consecutive glyphs
        if let Some(last) = last_glyph_id {
            width += scaled.kern(last, glyph_id);
        }
        width += scaled.h_advance(glyph_id);
        last_glyph_id = Some(glyph_id);
    }
    width
}

fn truncate_with_ellipsis(scale: &PxScaleFont<&FontVec>, text: &str, max_w: u32) -> String {
    let ellipsis = "\u{2026}";
    let ellipsis_width = measure_text(scale, ellipsis);

    let budget = max_w as f32 - ellipsis_width;
    if budget <= 0.0 {
        return ellipsis.to_string();
    }

    let mut result = String::new();
    let mut width = 0.0f32;
    for ch in text.chars() {
        let cw = measure_text(scale, &ch.to_string());
        if width + cw > budget {
            break;
        }
        result.push(ch);
        width += cw;
    }
    result.push_str(ellipsis);
    result
}

#[allow(clippy::too_many_arguments)]
fn draw_glyphs_clipped(
    canvas: &mut RgbaImage,
    scaled: &PxScaleFont<&FontVec>,
    text: &str,
    text_x: f32,
    text_y: f32,
    color: Rgba<u8>,
    rect: Rect,
) {
    let clip_x = (rect.x + rect.width).min(CANVAS_W) as i32;
    let clip_y = (rect.y + rect.height).min(CANVAS_H) as i32;

    let mut pen_x = text_x;
    let mut last_glyph_id = None;

    for ch in text.chars() {
        let glyph_id = scaled.glyph_id(ch);

        if let Some(last) = last_glyph_id {
            pen_x += scaled.kern(last, glyph_id);
        }

        // Grab this character with it's scale and position
        let glyph = glyph_id.with_scale_and_position(scaled.scale, ab_glyph::point(pen_x, text_y));
        pen_x += scaled.h_advance(glyph_id);
        last_glyph_id = Some(glyph_id);

        // Use the glyph outline to draw the character
        if let Some(outlined) = scaled.font.outline_glyph(glyph) {
            let bounding_box = outlined.px_bounds();
            outlined.draw(|gx, gy, gv| {
                let px = bounding_box.min.x as i32 + gx as i32;
                let py = bounding_box.min.y as i32 + gy as i32;

                if px < rect.x as i32 || px >= clip_x || py < rect.y as i32 || py >= clip_y {
                    return;
                }
                if px < 0 || py < 0 || px >= CANVAS_W as i32 || py >= CANVAS_H as i32 {
                    return;
                }

                // Make sure our alpha is applied to the glyph alpha
                let alpha = (gv * (color[3] as f32 / 255.0) * 255.0) as u8;

                let src = Rgba([color[0], color[1], color[2], alpha]);
                let dst = *canvas.get_pixel(px as u32, py as u32);

                canvas.put_pixel(px as u32, py as u32, blend(dst, src));
            });
        }
    }
}

// -------------- PIXMAP RENDERING ----------------------
fn render_pixmap(canvas: &mut RgbaImage, item: &PixmapItem) {
    let rect = item.common.rect;

    // Render the background
    let style = FillStyle {
        gradient: &parse_gradient(&item.common.background),
        opacity: item.common.opacity,
    };
    fill_rect(canvas, &rect, &style);

    // Load and draw the image, or draw the checkerboard
    match load_pixmap_source(&item.value, &rect) {
        Some(img) => blit_image(canvas, &img, &rect, item.common.opacity),
        None => draw_checkerboard(canvas, &rect, item.common.opacity),
    }
}

fn load_pixmap_source(source: &PixmapSource, rect: &Rect) -> Option<RgbaImage> {
    match source {
        PixmapSource::None => None,

        PixmapSource::File(path) => image::open(path)
            .ok()
            .map(|i| resize_to_rect(i.to_rgba8(), rect)),

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

// Bar Helper
struct FillStyle<'a> {
    gradient: &'a Gradient,
    opacity: f32,
}

// -------------- Simple Bar ----------------------
fn render_bar(canvas: &mut RgbaImage, item: &BarItem) {
    // Basic bar doesn't need anything special, just draw it with the params
    draw_bar_shape(canvas, &item.common, &item.bar_common);
}

// -------------- GBar, bar with indicator ----------------------
fn render_gbar(canvas: &mut RgbaImage, item: &GBarItem) {
    let rect = &item.common.rect;

    // bar_h is the indicator triangle height; the bar occupies the rest of the rect.
    let indicator_height = item.bar_h.min(rect.height);
    let bar_height = rect.height.saturating_sub(indicator_height);

    // We need to adjust the rect to accommodate the indicator triangle
    let mut common = item.common.clone();
    common.rect = Rect {
        x: item.common.rect.x,
        y: item.common.rect.y,
        width: item.common.rect.width,
        height: bar_height,
    };

    // Draw the bar in the upper portion
    if bar_height > 0 {
        let mut bar_common = item.bar_common.clone();
        bar_common.value = 0.0;

        draw_bar_shape(canvas, &common, &bar_common);
    }

    // Draw the triangle indicator in the lower portion, first get its position
    let fraction = normalise(item.bar_common.value, &item.bar_common.range);
    let indicator_y = item.common.rect.y + (bar_height / 2);
    let tip_x = rect.x + (rect.width as f32 * fraction) as u32;

    // We need the background stops so we can colour the triangle
    let bg_stops = parse_gradient(&item.bar_common.bar_bg_c);

    // Get the value, and find the gradient colour at that point
    let fraction = normalise(item.bar_common.value, &item.bar_common.range);
    let ind_color = with_opacity(sample_gradient(&bg_stops, fraction), item.common.opacity);

    // Get the border colour
    let border_c = parse_color(&item.bar_common.bar_border_c);

    draw_triangle_indicator(
        canvas,
        tip_x,
        indicator_y,
        indicator_height,
        ind_color,
        border_c,
        1,
    );
}

fn draw_triangle_indicator(
    canvas: &mut RgbaImage,
    tip_x: u32,
    top_y: u32,
    height: u32,
    color: Rgba<u8>,
    border_c: Rgba<u8>,
    border_w: u32,
) {
    if height == 0 {
        return;
    }

    for row in 0..height {
        let half_w = (row as f32 / 3.0_f32.sqrt()).round() as u32;

        let lx = tip_x.saturating_sub(half_w);
        let rx = (tip_x + half_w).min(CANVAS_W - 1);
        let py = top_y + row;

        if py >= CANVAS_H {
            continue;
        }

        for px in lx..=rx {
            if px >= CANVAS_W {
                continue;
            }

            let on_left_edge = px < lx + border_w;
            let on_right_edge = border_w > 0 && px > rx.saturating_sub(border_w);
            let on_bottom_edge = row >= height.saturating_sub(border_w);

            let on_edge = border_w > 0
                && border_c[3] > 0
                && (on_left_edge || on_right_edge || on_bottom_edge);

            let dst = *canvas.get_pixel(px, py);
            canvas.put_pixel(px, py, blend(dst, if on_edge { border_c } else { color }));
        }
    }
}

// ----- Shaped Bar Renderer, used by bar and gbar ---
fn draw_bar_shape(canvas: &mut RgbaImage, common: &CommonFields, bar_common: &BarCommon) {
    let rect = common.rect;
    let bg = FillStyle {
        gradient: &parse_gradient(&bar_common.bar_bg_c),
        opacity: common.opacity,
    };
    let fill = FillStyle {
        gradient: &parse_gradient(&bar_common.bar_fill_c),
        opacity: common.opacity,
    };
    let border_c = parse_color(&bar_common.bar_border_c);
    let border_w = bar_common.border_w;

    let fraction = normalise(bar_common.value, &bar_common.range);
    let subtype = bar_common.subtype;

    let rects: &[Rect] = match subtype {
        // For these, we keep the rect as-is
        BarSubtype::Rectangle | BarSubtype::Trapezoid | BarSubtype::Groove => &[rect],

        // These need the rect split into two halves with a gap in between
        BarSubtype::DoubleRectangle | BarSubtype::DoubleTrapezoid => {
            let gap = 2u32;
            let half = (rect.height.saturating_sub(gap)) / 2;
            if half == 0 {
                return;
            }
            &[
                Rect {
                    height: half,
                    ..rect
                },
                Rect {
                    y: rect.y + half + gap,
                    height: half,
                    ..rect
                },
            ]
        }
    };

    // Get the function needed for drawing
    // TODO: Trapezoid
    let draw_fn: fn(&mut RgbaImage, &Rect, &FillStyle, u32) = match subtype {
        BarSubtype::Rectangle | BarSubtype::DoubleRectangle => draw_rect,
        BarSubtype::Trapezoid | BarSubtype::DoubleTrapezoid => draw_groove,
        BarSubtype::Groove => draw_groove,
    };

    // TODO: Trapezoid
    let draw_border_fn: fn(&mut RgbaImage, &Rect, Rgba<u8>, u32) = match subtype {
        BarSubtype::Rectangle | BarSubtype::DoubleRectangle => draw_rect_border,
        BarSubtype::Trapezoid | BarSubtype::DoubleTrapezoid => draw_groove_border,
        BarSubtype::Groove => draw_groove_border,
    };

    for r in rects {
        // Draw the bar base
        draw_fn(canvas, r, &bg, r.width);

        // If we have a value > 0, draw the fill
        if fraction > 0.0 {
            let fill_w = (r.width as f32 * fraction) as u32;
            draw_fn(canvas, r, &fill, fill_w);
        }

        // Finally, draw the border if needed
        if border_w > 0 {
            draw_border_fn(canvas, r, border_c, border_w);
        }
    }
}

// These are some common function between groove and rect
#[inline]
fn resolve_colour(style: &FillStyle, px: u32, rect: &Rect, solid: Option<Rgba<u8>>) -> Rgba<u8> {
    solid.unwrap_or_else(|| {
        let gradient_pos = px.saturating_sub(rect.x) as f32 / rect.width as f32;
        with_opacity(sample_gradient(style.gradient, gradient_pos), style.opacity)
    })
}

#[inline]
fn put_blended(canvas: &mut RgbaImage, px: u32, py: u32, colour: Rgba<u8>) {
    if colour[3] == 255 {
        canvas.put_pixel(px, py, colour);
    } else {
        let dst = *canvas.get_pixel(px, py);
        canvas.put_pixel(px, py, blend(dst, colour));
    }
}

// Rectangle Drawing
fn draw_rect(canvas: &mut RgbaImage, rect: &Rect, style: &FillStyle, stop: u32) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }

    let fill_width = stop.min(rect.width);
    let stop_x = (rect.x + fill_width).min(CANVAS_W);
    let stop_y = (rect.y + rect.height).min(CANVAS_H);

    let is_solid = style.gradient.len() == 1;
    let solid_colour = is_solid.then(|| with_opacity(style.gradient[0].color, style.opacity));

    for px in rect.x..stop_x {
        let colour = resolve_colour(style, px, rect, solid_colour);
        for py in rect.y..stop_y {
            put_blended(canvas, px, py, colour);
        }
    }
}

fn draw_rect_border(canvas: &mut RgbaImage, rect: &Rect, border_c: Rgba<u8>, border_w: u32) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    draw_border(canvas, rect, border_c, border_w);
}

// Groove Drawing
fn draw_groove(canvas: &mut RgbaImage, rect: &Rect, style: &FillStyle, stop: u32) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }

    // Clamp the fill width to both the stop and the actual bar width
    let fill_width = stop.min(rect.width);
    let fill_rect = Rect {
        width: fill_width,
        ..*rect
    };

    // The points where we stop rendering
    let stop_x = (rect.x + stop).min(CANVAS_W);
    let stop_y = (rect.y + rect.height).min(CANVAS_H);

    // The radius of the groove (rounded edges)
    let radius = rect.height as f32 / 2.0;

    // The area where we're drawing a rectangle, without corner influence
    let mid_x_start = rect.x + radius as u32;
    let mid_x_end = (rect.x + fill_width).saturating_sub(radius as u32);

    // If we're solid, we don't need gradient sampling, so grab the colour directly
    let is_solid = style.gradient.len() == 1;
    let solid_colour = is_solid.then(|| with_opacity(style.gradient[0].color, style.opacity));

    for px in rect.x..stop_x {
        // Either use our fixed colour, or sample it from the gradient
        let colour = resolve_colour(style, px, rect, solid_colour);
        let in_mid = px >= mid_x_start && px < mid_x_end;

        for py in rect.y..stop_y {
            if !in_mid && !groove_rect_contains(&fill_rect, px, py) {
                continue;
            }
            put_blended(canvas, px, py, colour);
        }
    }
}

fn draw_groove_border(canvas: &mut RgbaImage, rect: &Rect, colour: Rgba<u8>, border_w: u32) {
    if border_w == 0 || rect.width == 0 || rect.height == 0 || colour[3] == 0 {
        return;
    }

    let inner = Rect {
        x: rect.x + border_w,
        y: rect.y + border_w,
        width: rect.width.saturating_sub(border_w * 2),
        height: rect.height.saturating_sub(border_w * 2),
    };

    let clip_x = (rect.x + rect.width).min(CANVAS_W);
    let clip_y = (rect.y + rect.height).min(CANVAS_H);

    for py in rect.y..clip_y {
        for px in rect.x..clip_x {
            if groove_rect_contains(rect, px, py) && !groove_rect_contains(&inner, px, py) {
                let dst = *canvas.get_pixel(px, py);
                canvas.put_pixel(px, py, blend(dst, colour));
            }
        }
    }
}

/// This is a simple helper function that determines whether a pixel is inside the groove bars
/// rendering area (so not outside the rounded corners)
fn groove_rect_contains(rect: &Rect, px: u32, py: u32) -> bool {
    // Shouldn't happen, but test for a bad rect
    if rect.width == 0 || rect.height == 0 {
        return false;
    }

    // Get the local x / y position and radius area
    let radius = rect.height as f32 / 2.0;
    let local_x = px.saturating_sub(rect.x) as f32 + 0.5;
    let local_y = py.saturating_sub(rect.y) as f32 + 0.5;

    // Fast Pass, if we're not inside the rounded area, we're always good
    if local_x >= radius && local_x <= rect.width as f32 - radius {
        return true;
    }

    // If we get here, we need to test the pixel inside a circle
    let center_y = rect.height as f32 / 2.0;
    let center_x = if local_x < radius {
        radius
    } else {
        rect.width as f32 - radius
    };

    // Calculate the offset from the center of the circle
    let offset_x = local_x - center_x;
    let offset_y = local_y - center_y;

    // Return whether this offset is inside the circle (and thus should be drawn)
    offset_x * offset_x + offset_y * offset_y <= radius * radius
}

// Normalise a value between two points
fn normalise(value: f32, range: &Range) -> f32 {
    ((value - range.min) / (range.max - range.min)).clamp(0.0, 1.0)
}
