use crate::STRICT_RENDER;
use crate::color::{parse_color, parse_gradient, sample_gradient, with_opacity};
use crate::components::bar_common::draw_bar_shape;
use crate::layout::{BarCommon, GBarItem, Rect};
use crate::render::{CANVAS_H, CANVAS_W, blend, is_valid_rect, normalise};
use image::{Rgba, RgbaImage};
use log::warn;

pub(crate) fn render_gbar(canvas: &mut RgbaImage, item: &GBarItem) {
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

    // bar_h is the indicator triangle height; the bar occupies the rest of the rect.
    let indicator_height = item.bar_h.min(rect.height);
    let bar_height = indicator_height;

    // Draw the bar in the upper portion
    if bar_height > 0 {
        let mut common = item.common.clone();
        common.rect = Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: bar_height,
        };

        let bar_common = BarCommon {
            value: 0.0,
            ..item.bar_common.clone()
        };
        draw_bar_shape(canvas, &common, &bar_common);
    }

    // Draw the triangle indicator in the lower portion, first get its position
    let fraction = normalise(item.bar_common.value, &item.bar_common.range);

    // Try and position the tip of the triangle vertically centered to the middle of the value bar
    let target_tip_y = rect.y + bar_height / 2;
    let max_tip_y = rect.y + rect.height.saturating_sub(indicator_height);

    // Work with best possible position (if we don't fit, move)
    let indicator_y = target_tip_y.min(max_tip_y);
    let tip_x = rect.x + (rect.width as f32 * fraction) as u32;

    // We need the background stops so we can colour the triangle
    let bg_stops = parse_gradient(&item.bar_common.bar_bg_c);

    // Get the value, and find the gradient colour at that point
    let ind_color = with_opacity(sample_gradient(&bg_stops, fraction), item.common.opacity);

    // Get the border colour
    let border_c = parse_color(&item.bar_common.bar_border_c);
    let border_w = item.bar_common.border_w;

    draw_triangle_indicator(
        canvas,
        tip_x,
        indicator_y,
        indicator_height,
        ind_color,
        border_c,
        border_w,
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
    let inner_border_c: Rgba<u8> = Rgba([128, 128, 128, 255]);
    let inner_border_w = border_w + 1; // 1px inset from outer border

    for row in 0..height {
        let lx = tip_x.saturating_sub(row);
        let rx = (tip_x + row).min(CANVAS_W - 1);
        let py = top_y + row;

        if py >= CANVAS_H {
            continue;
        }

        for px in lx..=rx {
            if px >= CANVAS_W {
                continue;
            }

            // Is this pixel attached to the 'main' border
            let on_outer_l = px < lx + border_w;
            let on_outer_r = border_w > 0 && px > rx.saturating_sub(border_w);
            let on_outer_b = row >= height.saturating_sub(border_w);
            let on_outer_border =
                border_w > 0 && border_c[3] > 0 && (on_outer_l || on_outer_r || on_outer_b);

            // Is this pixel attached to the 'forced' inner border?
            let on_inner_l = px >= lx + border_w && px < lx + inner_border_w;
            let on_inner_r =
                px <= rx.saturating_sub(border_w) && px > rx.saturating_sub(inner_border_w);
            let on_inner_b = row < height.saturating_sub(border_w)
                && row >= height.saturating_sub(inner_border_w);
            let on_inner_border = !on_outer_border && (on_inner_l || on_inner_r || on_inner_b);

            // If we're on a border, colour it.
            let pixel_color = if on_outer_border {
                border_c
            } else if on_inner_border {
                inner_border_c
            } else {
                color
            };

            let dst = *canvas.get_pixel(px, py);
            canvas.put_pixel(px, py, blend(dst, pixel_color));
        }
    }
}
