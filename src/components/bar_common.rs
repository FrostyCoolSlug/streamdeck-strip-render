use crate::color::{parse_color, parse_gradient, with_opacity};
use crate::layout::{BarCommon, BarSubtype, CommonFields, Rect};
use crate::render::{
    CANVAS_H, CANVAS_W, FillStyle, blend, draw_border, normalise, put_blended, resolve_colour,
};
use image::{Rgba, RgbaImage};

pub(crate) fn draw_bar_shape(
    canvas: &mut RgbaImage,
    common: &CommonFields,
    bar_common: &BarCommon,
) {
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

    // In the official app, DoubleRectangle and DoubleTrapezoid render as a single Rectangle
    // or Trapezoid respectively. The Double apparently means nothing :D

    // Get the function needed for drawing
    // TODO: Trapezoid
    let draw_fn: fn(&mut RgbaImage, &Rect, &FillStyle, u32) = match subtype {
        BarSubtype::Rectangle | BarSubtype::DoubleRectangle => draw_rect,
        BarSubtype::DoubleTrapezoid => draw_groove,
        BarSubtype::Trapezoid => draw_trapezoid,
        BarSubtype::Groove => draw_groove,
    };

    // TODO: Trapezoid
    let draw_border_fn: fn(&mut RgbaImage, &Rect, Rgba<u8>, u32) = match subtype {
        BarSubtype::Rectangle | BarSubtype::DoubleRectangle => draw_rect_border,
        BarSubtype::DoubleTrapezoid => draw_groove_border,
        BarSubtype::Trapezoid => draw_trapezoid_border,
        BarSubtype::Groove => draw_groove_border,
    };

    // Draw the bar base
    draw_fn(canvas, &rect, &bg, rect.width);

    // If we have a value > 0, draw the fill
    if fraction > 0.0 {
        let fill_w = (rect.width as f32 * fraction) as u32;
        draw_fn(canvas, &rect, &fill, fill_w);
    }

    // Finally, draw the border if needed
    if border_w > 0 {
        draw_border_fn(canvas, &rect, border_c, border_w);
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

const TRAP_START: f32 = 0.9;

fn draw_trapezoid(canvas: &mut RgbaImage, rect: &Rect, style: &FillStyle, stop: u32) {
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

        let local_x = px - rect.x;
        let top_offset = ((local_x as f32 / rect.width as f32) * rect.height as f32 * TRAP_START
            + rect.height as f32 * (1.0 - TRAP_START)) as u32;
        let top_y = (rect.y + rect.height)
            .saturating_sub(top_offset)
            .max(rect.y);

        for py in top_y..stop_y {
            put_blended(canvas, px, py, colour);
        }
    }
}

fn draw_trapezoid_border(canvas: &mut RgbaImage, rect: &Rect, colour: Rgba<u8>, border_w: u32) {
    if border_w == 0 || rect.width == 0 || rect.height == 0 || colour[3] == 0 {
        return;
    }

    let clip_x = (rect.x + rect.width).min(CANVAS_W);
    let clip_y = (rect.y + rect.height).min(CANVAS_H);

    for py in rect.y..clip_y {
        for px in rect.x..clip_x {
            if trapezoid_contains(rect, px, py)
                && !trapezoid_contains(
                    &Rect {
                        x: rect.x + border_w,
                        y: rect.y + border_w,
                        width: rect.width.saturating_sub(border_w * 2),
                        height: rect.height.saturating_sub(border_w * 2),
                    },
                    px,
                    py,
                )
            {
                let dst = *canvas.get_pixel(px, py);
                canvas.put_pixel(px, py, blend(dst, colour));
            }
        }
    }
}

fn trapezoid_contains(rect: &Rect, px: u32, py: u32) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    let local_x = px.saturating_sub(rect.x) as f32;
    let local_y = py.saturating_sub(rect.y) as f32;

    let top_edge_y = rect.height as f32 * TRAP_START * (1.0 - local_x / rect.width as f32);

    local_y >= top_edge_y && local_y < rect.height as f32
}
