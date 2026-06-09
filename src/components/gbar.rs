use crate::color::{parse_color, parse_gradient, sample_gradient, with_opacity};
use crate::components::bar_common::draw_bar_shape;
use crate::layout::{GBarItem, Rect};
use crate::render::{CANVAS_H, CANVAS_W, blend, normalise};
use image::{Rgba, RgbaImage};

pub(crate) fn render_gbar(canvas: &mut RgbaImage, item: &GBarItem) {
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
