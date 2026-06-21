//! Serde-deserializable types matching the Elgato Stream Deck layout schema.
//! https://docs.elgato.com/streamdeck/sdk/references/touch-strip-layout/
//! https://schemas.elgato.com/streamdeck/plugins/layout.json
//! All fields and defaults are taken directly from the JSON schema file.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

/// Top level layout for the action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    #[serde(rename = "$schema", default = "default_schema")]
    pub schema: String,
    pub id: String,
    pub items: Vec<LayoutItem>,
}
fn default_schema() -> String {
    "https://schemas.elgato.com/streamdeck/plugins/layout.json".to_string()
}

/// This enum is used to deserialize the `type` field of the layout item, and should map and
/// parse it correctly..
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LayoutItem {
    Text(TextItem),
    Pixmap(PixmapItem),
    Bar(BarItem),
    #[serde(rename = "gbar")]
    GBar(GBarItem),
}

impl LayoutItem {
    pub fn z_order(&self) -> u32 {
        match self {
            LayoutItem::Text(i) => i.common.z_order,
            LayoutItem::Pixmap(i) => i.common.z_order,
            LayoutItem::Bar(i) => i.common.z_order,
            LayoutItem::GBar(i) => i.common.z_order,
        }
    }
    pub fn enabled(&self) -> bool {
        match self {
            LayoutItem::Text(i) => i.common.enabled,
            LayoutItem::Pixmap(i) => i.common.enabled,
            LayoutItem::Bar(i) => i.common.enabled,
            LayoutItem::GBar(i) => i.common.enabled,
        }
    }
}

/// Shared fields, these should occur on all types, and be handled appropriately.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonFields {
    /// Unique name used to identify the layout item. When calling `setFeedback` this value should
    /// be used as the key as part of the object that represents the feedback.
    /// This has no default and should ALWAYS be defined
    pub key: String,

    /// Array defining the items coordinates in the format `[x, y, width, height]`; coordinates must
    /// be within canvas size of 200 x 100, e.g. [0, 0, 200, 100]. Items with the same `zOrder`
    /// must **not** have an overlapping `rect`.
    /// This has no default and should ALWAYS be defined
    pub rect: Rect,

    /// Layering order; items with higher zOrder paint on top. Default 0.
    #[serde(rename = "zOrder")]
    #[serde(default = "default_z_order")]
    pub z_order: u32,

    /// Determines whether the item is enabled (i.e. visible)
    /// Default: true
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Opacity 0.0–1.0 (schema allows only 1-decimal steps). Default 1.
    #[serde(default = "default_opacity")]
    pub opacity: f32,

    /// Background colour represented as a named colour, hexadecimal value, or gradient.
    /// Default: black
    #[serde(default = "default_background")]
    pub background: String,
}

fn default_z_order() -> u32 {
    0
}

fn default_enabled() -> bool {
    true
}

fn default_opacity() -> f32 {
    1.0
}

fn default_background() -> String {
    "transparent".to_string()
}

/// A text item, should map things to usable structs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextItem {
    #[serde(flatten)]
    pub common: CommonFields,

    /// WARNING: Per the docs, If key == "title", this should be set based on the prop inspector.
    /// This code doesn't care, it's upstreams responsibility to set this correctly.
    ///
    /// Alignment of the text. [left | center | right]
    /// Default: center
    #[serde(default = "default_alignment")]
    pub alignment: TextAlignment,

    /// Colour of the font represented as a named colour, or hexadecimal value.
    /// Default: white
    #[serde(default = "default_white")]
    pub color: String,

    /// Text to be displayed
    #[serde(default)]
    value: Option<String>,

    /// WARNING: Per the docs, If key == "title", this should be set based on the prop inspector.
    /// This code doesn't care, it's upstreams responsibility to set this correctly.
    ///
    /// Defines how the font should be rendered.
    #[serde(default)]
    pub font: FontConfig,

    /// Defines how overflowing text should be rendered on the layout. [clip | ellipsis | fade]
    /// Default: ellipsis
    #[serde(default = "default_text_overflow")]
    #[serde(rename = "text-overflow")]
    pub text_overflow: TextOverflow,
}

impl TextItem {
    pub fn value(&self) -> String {
        // TODO: Values are optional, if we don't have one, just render the key, this might not be correct
        self.value
            .clone()
            .unwrap_or_else(|| format!("{{{{{}}}}}", self.common.key))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

fn default_alignment() -> TextAlignment {
    TextAlignment::Center
}

fn default_text_overflow() -> TextOverflow {
    TextOverflow::Clip
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TextOverflow {
    Clip,
    Ellipsis,
    Fade,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    /// Size of the font, in pixels, represented as a whole number
    /// The examples use 16
    /// Default: This isn't declared in the docs, so will need review
    #[serde(default = "default_font_size")]
    pub size: f32,
    /// Weight of the font; value must be a whole `number` in the range of `100..1000`
    /// The examples use 600
    /// Default: This isn't declared in the docs, so will need review
    #[serde(default = "default_font_weight")]
    pub weight: u32,
}

impl Default for FontConfig {
    fn default() -> Self {
        FontConfig {
            size: default_font_size(),
            weight: default_font_weight(),
        }
    }
}

fn default_font_size() -> f32 {
    16.0
}
fn default_font_weight() -> u32 {
    600
}

/// PixMap handling, try to parse to a type, and a process the data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PixmapItem {
    #[serde(flatten)]
    pub common: CommonFields,
    /// Image to render; this can be either a path to a local file within the plugin's folder, a
    /// base64 encoded `string` with the mime type declared (e.g. PNG, JPEG, etc.), or an
    /// SVG `string`.
    ///
    /// The docs don't define the value as required, but it kinda negates the entire point of
    /// this type of structure, so we'll assume it's required.
    #[serde(default = "default_pixmap_source")]
    pub value: PixmapSource,
}

fn default_pixmap_source() -> PixmapSource {
    PixmapSource::None
}

#[derive(Debug, Clone, Serialize)]
pub enum PixmapSource {
    File(String),
    Bytes(Vec<u8>),
    Svg(String),
    None,
}

impl<'de> Deserialize<'de> for PixmapSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(parse_pixmap(&s))
    }
}

fn parse_pixmap(input: &str) -> PixmapSource {
    let s = input.trim();

    if s.is_empty() {
        return PixmapSource::None;
    }

    if s.starts_with("data:image/") {
        return parse_data_url(s).unwrap_or(PixmapSource::None);
    }

    if is_svg(s) {
        return PixmapSource::Svg(s.to_string());
    }

    // Try and short-circuit the SVG checks here
    if s.ends_with(".svg")
        && let Ok(contents) = std::fs::read_to_string(s)
        && is_svg(&contents)
    {
        return PixmapSource::Svg(contents);
    }

    PixmapSource::File(s.to_string())
}

fn is_svg(s: &str) -> bool {
    s.trim_start().contains("<svg")
}

fn parse_data_url(s: &str) -> Option<PixmapSource> {
    let (header, payload) = s.split_once(',')?;
    if !header.contains("base64") {
        return None;
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .ok()?;

    // If an SVG is being passed in as base64, handle it as an SVG
    if header.contains("image/svg+xml") {
        Some(PixmapSource::Svg(String::from_utf8(bytes).ok()?))
    } else {
        Some(PixmapSource::Bytes(bytes))
    }
}

/// Common fields for bar and gbar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarCommon {
    /// Bar background colour represented as a named colour, hexadecimal value, or gradient.
    /// Default is `darkGray`
    #[serde(default = "default_dark_gray")]
    pub bar_bg_c: String,

    /// Border colour represented as a named colour, or hexadecimal value.
    /// Default is `white`
    #[serde(default = "default_white")]
    pub bar_border_c: String,

    /// Fill color of the bar represented as a named color, hexadecimal value, or gradient.
    /// Default is `white`
    #[serde(default = "default_white")]
    pub bar_fill_c: String,

    /// Width of the border around the bar, as a whole number.
    /// Default is 2
    #[serde(default = "default_border_w")]
    pub border_w: u32,

    /// Defines the range of the value the bar represents.
    /// Default is 0..100
    #[serde(default = "default_range")]
    pub range: Range,

    /// 0=Rectangle, 1=DoubleRectangle, 2=Trapezoid,
    /// 3=DoubleTrapezoid, 4=Groove.
    /// Default is 'Groove'
    #[serde(default = "default_subtype")]
    pub subtype: BarSubtype,

    /// Fill value; correlates with `range`.
    /// This has no default and should ALWAYS be defined.
    #[serde(deserialize_with = "deserialize_f32_string_or_number")]
    pub value: f32,
}

// This will turn a Number or String into an f32
fn deserialize_f32_string_or_number<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct F32Visitor;

    impl<'de> serde::de::Visitor<'de> for F32Visitor {
        type Value = f32;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "a float or string representing a float")
        }

        fn visit_u64<E>(self, v: u64) -> Result<f32, E>
        where
            E: serde::de::Error,
        {
            Ok(v as f32)
        }

        fn visit_f64<E>(self, v: f64) -> Result<f32, E>
        where
            E: serde::de::Error,
        {
            Ok(v as f32)
        }

        fn visit_str<E>(self, v: &str) -> Result<f32, E>
        where
            E: serde::de::Error,
        {
            v.trim()
                .parse::<f32>()
                .map_err(|_| E::custom("invalid float string"))
        }
    }

    deserializer.deserialize_any(F32Visitor)
}

/// Bar item Handler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarItem {
    #[serde(flatten)]
    pub common: CommonFields,

    #[serde(flatten)]
    pub bar_common: BarCommon,
}

/// GBar item Handler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GBarItem {
    #[serde(flatten)]
    pub common: CommonFields,

    #[serde(flatten)]
    pub bar_common: BarCommon,

    /// Height of the bar's indicator.
    /// Default: 10
    #[serde(default = "default_bar_h")]
    pub bar_h: u32,
}

fn default_bar_h() -> u32 {
    10
}

fn default_dark_gray() -> String {
    "darkGray".to_string()
}

fn default_white() -> String {
    "white".to_string()
}

fn default_border_w() -> u32 {
    0
}

fn default_range() -> Range {
    Range {
        min: 0.0,
        max: 100.0,
    }
}

fn default_subtype() -> BarSubtype {
    BarSubtype::Groove
}

/// Bar Subtype from u8
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum BarSubtype {
    Rectangle = 0,
    DoubleRectangle = 1,
    Trapezoid = 2,
    DoubleTrapezoid = 3,
    Groove = 4,
}

/// The position and size of a layout item on the 200×100 canvas. Deserializes from the JSON
/// array form `[x, y, width, height]`. We are making the active assumption that the values are
/// integers and will never be negative, because per the schema, it doesn't make sense otherwise.
///
/// With that said, these may be floats, JavaScript numbers are pretty ambiguous
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Rect {
    /// X coordinate of the rectangle.
    pub x: u32,

    /// Y coordinate of the rectangle.
    pub y: u32,

    /// Width of the rectangle.
    pub width: u32,

    /// Height of the rectangle.
    pub height: u32,
}

/// A Deserialize helper for `Rect`, ensuring all values are valid in the expected canvas.
impl<'de> Deserialize<'de> for Rect {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let [x, y, width, height] = <[u32; 4]>::deserialize(d)?;

        if !(0..=200).contains(&x) {
            return Err(serde::de::Error::custom("x out of range (0..200)"));
        }

        if !(0..=100).contains(&y) {
            return Err(serde::de::Error::custom("y out of range (0..100)"));
        }

        if !(0..=200).contains(&width) {
            return Err(serde::de::Error::custom("width out of range (0..200)"));
        }

        if !(0..=100).contains(&height) {
            return Err(serde::de::Error::custom("height out of range (0..100)"));
        }

        Ok(Rect {
            x,
            y,
            width,
            height,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub min: f32,
    pub max: f32,
}
