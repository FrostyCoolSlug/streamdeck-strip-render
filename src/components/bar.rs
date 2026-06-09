use crate::components::bar_common::draw_bar_shape;
use crate::layout::BarItem;
use image::RgbaImage;

pub(crate) fn render_bar(canvas: &mut RgbaImage, item: &BarItem) {
    // Basic bar doesn't need anything special, just draw it with the params
    draw_bar_shape(canvas, &item.common, &item.bar_common);
}
