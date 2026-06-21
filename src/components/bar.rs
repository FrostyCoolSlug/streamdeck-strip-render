use crate::STRICT_RENDER;
use crate::components::bar_common::draw_bar_shape;
use crate::layout::BarItem;
use crate::render::is_valid_rect;
use image::RgbaImage;
use log::warn;

pub(crate) fn render_bar(canvas: &mut RgbaImage, item: &BarItem) {
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

    // Basic bar doesn't need anything special, just draw it with the params
    draw_bar_shape(canvas, &item.common, &item.bar_common);
}
