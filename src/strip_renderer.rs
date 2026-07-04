use crate::layout::{
    BarCommon, CommonFields, Layout, LayoutItem, PixmapItem, Range, Rect, TextItem, parse_pixmap,
};

use anyhow::Result;

use crate::components::bar::render_bar;
use crate::components::gbar::render_gbar;
use crate::components::pixmap::render_pixmap;
use crate::components::text::render_text;
use crate::render::{CANVAS_H, CANVAS_W, validate_layout};
use image::{Rgba, RgbaImage};
use log::trace;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct StripRenderer {
    layout: Layout,
    layers: HashMap<u32, RgbaImage>,

    title_override: Option<LayoutItem>,
    icon_override: Option<LayoutItem>,

    latest_image: Option<RgbaImage>,
}

impl StripRenderer {
    pub fn from(layout: Layout) -> Result<Self> {
        validate_layout(&layout)?;

        let mut instance = Self {
            layout,
            layers: HashMap::new(),

            title_override: None,
            icon_override: None,

            latest_image: None,
        };
        instance.build_initial_layers();
        Ok(instance)
    }

    fn build_initial_layers(&mut self) {
        // We need to go through every item and render it on its respective layer
        for item in self.layout.items.iter() {
            if !item.enabled() {
                continue;
            }

            // Find the canvas for this z-order item
            let canvas = self
                .layers
                .entry(item.z_order())
                .or_insert(RgbaImage::new(CANVAS_W, CANVAS_H));

            Self::paint(canvas, item);
        }
    }

    fn redraw_item(layers: &mut HashMap<u32, RgbaImage>, item: &LayoutItem) {
        let canvas = layers
            .entry(item.z_order())
            .or_insert_with(|| RgbaImage::new(CANVAS_W, CANVAS_H));

        Self::clear_rect(canvas, &item.common().rect);
        if item.enabled() {
            Self::paint(canvas, item);
        }
    }

    fn paint(canvas: &mut RgbaImage, item: &LayoutItem) {
        match item {
            LayoutItem::Text(text) => render_text(canvas, text),
            LayoutItem::Pixmap(pixmap) => render_pixmap(canvas, pixmap),
            LayoutItem::Bar(bar) => render_bar(canvas, bar),
            LayoutItem::GBar(gbar) => render_gbar(canvas, gbar),
        }
    }

    fn clear_rect(canvas: &mut RgbaImage, rect: &Rect) {
        let x_end = (rect.x + rect.width).min(canvas.width());
        let y_end = (rect.y + rect.height).min(canvas.height());

        for y in rect.y..y_end {
            for x in rect.x..x_end {
                canvas.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }

    pub fn layout(&mut self) -> &Layout {
        &self.layout
    }

    pub fn set_title_override(&mut self, text: Option<String>) {
        let Some(title) = self.layout.item("title") else {
            return;
        };

        // Nothing to do if there's no change incoming
        let current = self.title_override.as_ref().and_then(|item| match item {
            LayoutItem::Text(text) => text.value.as_ref(),
            _ => None,
        });
        if current == text.as_ref() {
            return;
        }

        // Work out what we need to draw
        let redraw = match text {
            Some(text) => {
                let replacement = match title.clone() {
                    LayoutItem::Text(mut inner) => {
                        inner.value = Some(text);
                        LayoutItem::Text(inner)
                    }
                    _ => return,
                };

                self.title_override = Some(replacement.clone());
                replacement
            }
            None => {
                self.title_override = None;
                title.clone()
            }
        };

        // Render the new title and invalidate the cache
        Self::redraw_item(&mut self.layers, &redraw);
        self.latest_image = None;
    }

    pub fn set_icon_override(&mut self, text: Option<String>) {
        let Some(icon) = self.layout.item("icon") else {
            return;
        };

        // Nothing to do if there's no change incoming
        let current = self.icon_override.as_ref().and_then(|item| match item {
            LayoutItem::Pixmap(pixmap) => Some(&pixmap.value),
            _ => None,
        });

        let incoming = text.as_deref().map(parse_pixmap);
        if current == incoming.as_ref() {
            return;
        }

        // Work out what we need to draw
        let redraw = match incoming {
            Some(value) => {
                let replacement = match icon.clone() {
                    LayoutItem::Pixmap(mut inner) => {
                        inner.value = value;
                        LayoutItem::Pixmap(inner)
                    }
                    _ => return,
                };

                self.icon_override = Some(replacement.clone());
                replacement
            }
            None => {
                self.icon_override = None;
                icon.clone()
            }
        };

        // Render the new icon and invalidate the cache
        Self::redraw_item(&mut self.layers, &redraw);
        self.latest_image = None;
    }

    pub fn get_image(&mut self) -> RgbaImage {
        if let Some(latest) = &self.latest_image {
            return latest.clone();
        }

        // Create the new Image, and grab it's raw buffer
        //let mut image = RgbaImage::new(CANVAS_W, CANVAS_H);
        let mut image = RgbaImage::from_pixel(CANVAS_W, CANVAS_H, Rgba([0, 0, 0, 255]));

        // Get all the layers sorted by z-order
        let mut entries: Vec<_> = self.layers.iter().collect();
        entries.sort_unstable_by_key(|(k, _)| *k);

        // Overlay layers in order
        for (_, layer) in entries {
            image::imageops::overlay(&mut image, layer, 0, 0);
        }

        self.latest_image = Some(image.clone());
        image
    }

    pub fn set_feedback(&mut self, feedback: Value) -> Result<Vec<String>> {
        let Value::Object(map) = feedback else {
            return Ok(Vec::new());
        };

        let mut changed_keys = Vec::new();

        for (key, payload_value) in &map {
            let Some(item) = self.layout.item_mut(key) else {
                trace!("setFeedback: no layout item found for key '{key}'");
                continue;
            };

            let changed = match (&mut *item, payload_value) {
                (LayoutItem::Text(t), Value::Object(obj)) => {
                    let a = Self::apply_common(&mut t.common, obj);
                    let b = Self::apply_text_object(t, obj);
                    a || b
                }
                (LayoutItem::Text(t), v) => Self::apply_text_scalar(t, v),

                (LayoutItem::Pixmap(p), Value::Object(obj)) => {
                    let a = Self::apply_common(&mut p.common, obj);
                    let b = Self::apply_pixmap_object(p, obj);
                    a || b
                }
                (LayoutItem::Pixmap(p), v) => Self::apply_pixmap_scalar(p, v),

                (LayoutItem::Bar(b), Value::Object(obj)) => {
                    let a = Self::apply_common(&mut b.common, obj);
                    let c = Self::apply_bar_common(&mut b.bar_common, obj);
                    a || c
                }
                (LayoutItem::Bar(b), v) => Self::apply_bar_value(&mut b.bar_common, v),

                (LayoutItem::GBar(g), Value::Object(obj)) => {
                    let a = Self::apply_common(&mut g.common, obj);
                    let c = Self::apply_bar_common(&mut g.bar_common, obj);
                    let d = if let Some(v) = obj.get("bar_h").and_then(|v| v.as_u64()) {
                        let v = v as u32;
                        if g.bar_h != v {
                            g.bar_h = v;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    a || c || d
                }
                (LayoutItem::GBar(g), v) => Self::apply_bar_value(&mut g.bar_common, v),
            };

            if !changed {
                continue;
            }

            // Only redraw a title change if we're not overriding it
            if key != "title" || self.title_override.is_none() {
                changed_keys.push(key.clone());
                Self::redraw_item(&mut self.layers, &*item);
            }
        }

        if !changed_keys.is_empty() {
            // Something's changed, invalidate our cached image
            self.latest_image = None;
        }

        Ok(changed_keys)
    }

    fn apply_common(item: &mut CommonFields, map: &Map<String, Value>) -> bool {
        let mut changed = false;

        for (key, value) in map {
            match key.as_str() {
                "enabled" => {
                    if let Value::Bool(b) = value {
                        if item.enabled != *b {
                            item.enabled = *b;
                            changed = true;
                        }
                    } else {
                        trace!("setFeedback: unexpected value for key '{key}' - {value}");
                    }
                }

                "opacity" => {
                    if let Some(v) = value.as_f64() {
                        if !(0.0..=1.0).contains(&v) {
                            trace!("setFeedback: opacity out of range for key '{key}' - {v}");
                            continue;
                        }
                        if item.opacity != v {
                            item.opacity = v;
                            changed = true;
                        }
                    } else {
                        trace!("setFeedback: unexpected value for key '{key}' - {value}");
                    }
                }

                "background" => {
                    if let Value::String(s) = value {
                        if &item.background != s {
                            item.background = s.clone();
                            changed = true;
                        }
                    } else {
                        trace!("setFeedback: unexpected value for key '{key}' - {value}");
                    }
                }

                _ => {}
            }
        }

        changed
    }

    fn apply_bar_common(bar: &mut BarCommon, map: &Map<String, Value>) -> bool {
        let mut changed = false;

        for (key, value) in map {
            match key.as_str() {
                "bar_bg_c" => {
                    if let Value::String(s) = value
                        && &bar.bar_bg_c != s
                    {
                        bar.bar_bg_c = s.clone();
                        changed = true;
                    }
                }

                "bar_border_c" => {
                    if let Value::String(s) = value
                        && &bar.bar_border_c != s
                    {
                        bar.bar_border_c = s.clone();
                        changed = true;
                    }
                }

                "bar_fill_c" => {
                    if let Value::String(s) = value
                        && &bar.bar_fill_c != s
                    {
                        bar.bar_fill_c = s.clone();
                        changed = true;
                    }
                }

                "border_w" => {
                    if let Some(v) = value.as_u64() {
                        let v = v as u32;
                        if bar.border_w != v {
                            bar.border_w = v;
                            changed = true;
                        }
                    }
                }

                "range" => {
                    if let Ok(range) = Range::deserialize(value)
                        && bar.range != range
                    {
                        bar.range = range;
                        changed = true;
                    }
                }

                "subtype" => {
                    if let Some(v) = value.as_u64() {
                        let subtype = (v as u32).into();
                        if bar.subtype != subtype {
                            bar.subtype = subtype;
                            changed = true;
                        }
                    }
                }

                "value" if Self::apply_bar_value(bar, value) => {
                    changed = true;
                }

                _ => {}
            }
        }

        changed
    }

    fn apply_bar_value(bar: &mut BarCommon, value: &Value) -> bool {
        let new_value = match value {
            Value::Number(n) => match n.as_f64() {
                Some(v) => v,
                None => {
                    trace!("setFeedback: bar value invalid - {n}");
                    return false;
                }
            },

            Value::String(s) => match s.parse::<f64>() {
                Ok(v) => v,
                Err(_) => {
                    trace!("setFeedback: bar value invalid - {s}");
                    return false;
                }
            },

            _ => {
                trace!("setFeedback: unexpected bar value - {value}");
                return false;
            }
        };

        if bar.value != new_value {
            bar.value = new_value;
            true
        } else {
            false
        }
    }

    fn apply_text_scalar(t: &mut TextItem, v: &Value) -> bool {
        let new_value = match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };

        if t.value.as_ref() != Some(&new_value) {
            t.value = Some(new_value);
            true
        } else {
            false
        }
    }

    fn apply_text_object(t: &mut TextItem, obj: &Map<String, Value>) -> bool {
        let mut changed = false;

        for (key, value) in obj {
            match key.as_str() {
                "alignment" => {
                    if let Value::String(s) = value {
                        let alignment = s.into();
                        if t.alignment != alignment {
                            t.alignment = alignment;
                            changed = true;
                        }
                    }
                }

                "color" => {
                    if let Value::String(s) = value
                        && &t.color != s
                    {
                        t.color = s.clone();
                        changed = true;
                    }
                }

                "value" if Self::apply_text_scalar(t, value) => {
                    changed = true;
                }

                "font" => {
                    if let Value::Object(font_map) = value {
                        for (k, v) in font_map {
                            match k.as_str() {
                                "size" => {
                                    if let Some(size) = v.as_f64()
                                        && t.font.size != size
                                    {
                                        t.font.size = size;
                                        changed = true;
                                    }
                                }

                                "weight" => {
                                    if let Some(w) = v.as_u64() {
                                        let w = w as u32;
                                        if t.font.weight != w {
                                            t.font.weight = w;
                                            changed = true;
                                        }
                                    }
                                }

                                _ => {}
                            }
                        }
                    }
                }

                "text_overflow" => {
                    if let Value::String(s) = value {
                        let overflow = s.into();
                        if t.text_overflow != overflow {
                            t.text_overflow = overflow;
                            changed = true;
                        }
                    }
                }

                _ => {}
            }
        }

        changed
    }

    fn apply_pixmap_scalar(p: &mut PixmapItem, v: &Value) -> bool {
        if let Value::String(s) = v {
            let new_value = parse_pixmap(s);
            if p.value != new_value {
                p.value = new_value;
                return true;
            }
        }
        false
    }

    fn apply_pixmap_object(p: &mut PixmapItem, obj: &Map<String, Value>) -> bool {
        if let Some(Value::String(s)) = obj.get("value") {
            let new_value = parse_pixmap(s);
            if p.value != new_value {
                p.value = new_value;
                return true;
            }
        }
        false
    }
}
