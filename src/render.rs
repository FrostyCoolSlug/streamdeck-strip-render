//! This code should be able to render a layout onto a 200x100 canvas, it doesn't quite cover
//! everything yet, but should be good enough to get a base render

use crate::color::{GradientStop, sample_gradient, with_opacity};
use crate::layout::{Layout, LayoutItem, Range, Rect};

use crate::components::bar::render_bar;
use crate::components::gbar::render_gbar;
use crate::components::pixmap::render_pixmap;
use crate::components::text::render_text;
use image::{ImageBuffer, Rgba, RgbaImage};
use std::error::Error;

type Gradient = Vec<GradientStop>;

pub const CANVAS_W: u32 = 200;
pub const CANVAS_H: u32 = 100;

/// Render `layout` onto a fresh 200×100 black canvas and return it.
pub(crate) fn render_layout(layout: &Layout) -> Result<RgbaImage, Box<dyn Error>> {
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
pub(crate) fn blend(dst: Rgba<u8>, src: Rgba<u8>) -> Rgba<u8> {
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
pub(crate) fn fill_rect(canvas: &mut RgbaImage, rect: &Rect, style: &FillStyle) {
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
pub(crate) fn draw_border(canvas: &mut RgbaImage, rect: &Rect, colour: Rgba<u8>, bw: u32) {
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

/// Defines how to fill an area
pub(crate) struct FillStyle<'a> {
    pub(crate) gradient: &'a Gradient,
    pub(crate) opacity: f32,
}

/// Resolves the colour of the current gradient position
#[inline]
pub(crate) fn resolve_colour(
    style: &FillStyle,
    px: u32,
    rect: &Rect,
    solid: Option<Rgba<u8>>,
) -> Rgba<u8> {
    solid.unwrap_or_else(|| {
        let gradient_pos = px.saturating_sub(rect.x) as f32 / rect.width as f32;
        with_opacity(sample_gradient(style.gradient, gradient_pos), style.opacity)
    })
}

#[inline]
/// Blend the current pixel with the given colour, and put it back into the canvas.
pub(crate) fn put_blended(canvas: &mut RgbaImage, px: u32, py: u32, colour: Rgba<u8>) {
    if colour[3] == 255 {
        canvas.put_pixel(px, py, colour);
    } else {
        let dst = *canvas.get_pixel(px, py);
        canvas.put_pixel(px, py, blend(dst, colour));
    }
}

// Normalise a value between two points
pub(crate) fn normalise(value: f32, range: &Range) -> f32 {
    ((value - range.min) / (range.max - range.min)).clamp(0.0, 1.0)
}
