use crate::color::{parse_color, parse_gradient, with_opacity};
use crate::layout::{Rect, TextAlignment, TextItem, TextOverflow};
use crate::render::{blend, fill_rect, is_valid_rect};
use crate::{FONT_SANS, STRICT_RENDER};
use ab_glyph::{Font, FontVec, PxScale, PxScaleFont, ScaleFont, VariableFont};
use image::{Rgba, RgbaImage};
use log::warn;
use std::sync::{LazyLock, Mutex};

pub(crate) static DEFAULT_FONT: LazyLock<Mutex<FontVec>> = LazyLock::new(|| {
    let font = FontVec::try_from_vec(FONT_SANS.to_vec()).expect("Bundled font is corrupt");
    Mutex::new(font)
});

pub(crate) fn render_text(canvas: &mut RgbaImage, item: &TextItem) {
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

    // Fill the background
    let style = crate::render::FillStyle {
        gradient: &parse_gradient(&item.common.background),
        opacity: item.common.opacity,
    };
    fill_rect(canvas, rect, &style);

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

    // Noto renders a little smaller than Inter and DejaVu, so we'll give it a 20% nudge
    let font_size = (item.font.size * 1.20) as f32;
    let scaled = font.as_scaled(PxScale::from(font_size));

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
    rect: &Rect,
) {
    let clip_x = (rect.x + rect.width).min(canvas.width()) as i32;
    let clip_y = (rect.y + rect.height).min(canvas.height()) as i32;

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
                if px < 0 || py < 0 || px >= canvas.width() as i32 || py >= canvas.height() as i32 {
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
