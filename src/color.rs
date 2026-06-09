//! This is a basic colour parser which takes a string, works out what we're trying to do
//! with it, and spits out a vec of GradientStops. Single colours return as a single stop.

use image::Rgba;

/// A single stop in a Stream Deck gradient string (`offset:color[,...]`).
#[derive(Debug, Clone)]
pub struct GradientStop {
    /// Position along the gradient axis, 0.0–1.0.
    pub offset: f32,
    pub color: Rgba<u8>,
}

/// Parse a color string into a list of gradient stops.
///
/// For plain solid colors this returns a single stop at offset 0.0.
/// For gradient strings (`"0:#ff0000,0.5:yellow,1:#00ff00"`) it returns one
/// stop per segment, sorted by offset.
pub fn parse_gradient(s: &str) -> Vec<GradientStop> {
    let s = s.trim();

    if looks_like_gradient(s) {
        let mut stops: Vec<GradientStop> = s
            .split(',')
            .filter_map(|segment| {
                let segment = segment.trim();
                let colon = segment.find(':')?;
                let offset: f32 = segment[..colon].trim().parse().ok()?;
                let color = parse_color(segment[colon + 1..].trim());
                Some(GradientStop { offset, color })
            })
            .collect();

        if !stops.is_empty() {
            stops.sort_by(|a, b| a.offset.partial_cmp(&b.offset).unwrap());
            return stops;
        }
    }

    vec![GradientStop {
        offset: 0.0,
        color: parse_color(s),
    }]
}

/// Sample a gradient at position `t` (0.0–1.0), linearly interpolating between stops.
pub fn sample_gradient(stops: &Vec<GradientStop>, t: f32) -> Rgba<u8> {
    match stops.as_slice() {
        [] => Rgba([255, 255, 255, 255]),
        [only] => only.color,
        _ => {
            let t = t.clamp(0.0, 1.0);
            // Find the pair of stops that bracket t
            let hi_idx = stops
                .iter()
                .position(|s| s.offset >= t)
                .unwrap_or(stops.len() - 1);
            let hi_idx = hi_idx.max(1); // ensure there's always a lo below it
            let lo = &stops[hi_idx - 1];
            let hi = &stops[hi_idx];
            let span = hi.offset - lo.offset;
            if span < 1e-6 {
                return lo.color;
            }
            let f = (t - lo.offset) / span;
            lerp_color(lo.color, hi.color, f)
        }
    }
}

/// Parse a single (non-gradient) colour string to an Rgba
/// For a gradient string the first stop's colour is returned.
pub fn parse_color(s: &str) -> Rgba<u8> {
    let s = s.trim();

    // Empty safety
    if s.is_empty() {
        return Rgba([255, 255, 255, 255]);
    }

    // Gradient: take first stop color
    if looks_like_gradient(s)
        && let Some(first) = s.split(',').next()
        && let Some(color_part) = first.split(':').nth(1)
    {
        return parse_color(color_part.trim());
    }

    // Hex formats
    if let Some(stripped) = s.strip_prefix("#") {
        return parse_hex(stripped);
    }

    // Raw hex without '#', we check the characters to make sure they're hex so we don't
    // accidentally swallow a named colour.
    if s.len() == 3 || s.len() == 6 || s.len() == 8 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        return parse_hex(s);
    }

    // Named CSS colour fallback
    named_colour(s)
}

/// Apply opacity (0.0–1.0) to a color's alpha channel.
pub fn with_opacity(c: Rgba<u8>, opacity: f32) -> Rgba<u8> {
    Rgba([
        c[0],
        c[1],
        c[2],
        (c[3] as f32 * opacity.clamp(0.0, 1.0)).round() as u8,
    ])
}

/// A gradient string has at least one segment that starts with a decimal number
/// followed by a colon — distinguishing it from plain hex (`#rrggbb`) or named colors.
fn looks_like_gradient(s: &str) -> bool {
    s.split(',').any(|seg| {
        let seg = seg.trim();
        seg.find(':')
            .is_some_and(|colon| seg[..colon].trim().parse::<f32>().is_ok())
    })
}

fn lerp_color(a: Rgba<u8>, b: Rgba<u8>, t: f32) -> Rgba<u8> {
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Rgba([
        lerp(a[0], b[0]),
        lerp(a[1], b[1]),
        lerp(a[2], b[2]),
        lerp(a[3], b[3]),
    ])
}

fn parse_hex(hex: &str) -> Rgba<u8> {
    let hex = hex.trim();
    match hex.len() {
        3 => {
            let bytes = hex.as_bytes();

            Rgba([
                expand_hex(bytes[0]),
                expand_hex(bytes[1]),
                expand_hex(bytes[2]),
                255,
            ])
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            Rgba([r, g, b, 255])
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            Rgba([r, g, b, a])
        }
        _ => Rgba([255, 255, 255, 255]),
    }
}

fn expand_hex(c: u8) -> u8 {
    let n = parse_nibble(c);
    (n << 4) | n
}

fn parse_nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

/// These are mapped from https://www.w3.org/TR/css-color-4/#named-colors
fn named_colour(name: &str) -> Rgba<u8> {
    match name.to_lowercase().as_str() {
        "aliceblue" => Rgba([240, 248, 255, 255]),
        "antiquewhite" => Rgba([250, 235, 215, 255]),
        "aqua" => Rgba([0, 255, 255, 255]),
        "aquamarine" => Rgba([127, 255, 212, 255]),
        "azure" => Rgba([240, 255, 255, 255]),
        "beige" => Rgba([245, 245, 220, 255]),
        "bisque" => Rgba([255, 228, 196, 255]),
        "black" => Rgba([0, 0, 0, 255]),
        "blanchedalmond" => Rgba([255, 235, 205, 255]),
        "blue" => Rgba([0, 0, 255, 255]),
        "blueviolet" => Rgba([138, 43, 226, 255]),
        "brown" => Rgba([165, 42, 42, 255]),
        "burlywood" => Rgba([222, 184, 135, 255]),
        "cadetblue" => Rgba([95, 158, 160, 255]),
        "chartreuse" => Rgba([127, 255, 0, 255]),
        "chocolate" => Rgba([210, 105, 30, 255]),
        "coral" => Rgba([255, 127, 80, 255]),
        "cornflowerblue" => Rgba([100, 149, 237, 255]),
        "cornsilk" => Rgba([255, 248, 220, 255]),
        "crimson" => Rgba([220, 20, 60, 255]),
        "cyan" => Rgba([0, 255, 255, 255]),
        "darkblue" => Rgba([0, 0, 139, 255]),
        "darkcyan" => Rgba([0, 139, 139, 255]),
        "darkgoldenrod" => Rgba([184, 134, 11, 255]),
        "darkgray" => Rgba([169, 169, 169, 255]),
        "darkgreen" => Rgba([0, 100, 0, 255]),
        "darkgrey" => Rgba([169, 169, 169, 255]),
        "darkkhaki" => Rgba([189, 183, 107, 255]),
        "darkmagenta" => Rgba([139, 0, 139, 255]),
        "darkolivegreen" => Rgba([85, 107, 47, 255]),
        "darkorange" => Rgba([255, 140, 0, 255]),
        "darkorchid" => Rgba([153, 50, 204, 255]),
        "darkred" => Rgba([139, 0, 0, 255]),
        "darksalmon" => Rgba([233, 150, 122, 255]),
        "darkseagreen" => Rgba([143, 188, 143, 255]),
        "darkslateblue" => Rgba([72, 61, 139, 255]),
        "darkslategray" => Rgba([47, 79, 79, 255]),
        "darkslategrey" => Rgba([47, 79, 79, 255]),
        "darkturquoise" => Rgba([0, 206, 209, 255]),
        "darkviolet" => Rgba([148, 0, 211, 255]),
        "deeppink" => Rgba([255, 20, 147, 255]),
        "deepskyblue" => Rgba([0, 191, 255, 255]),
        "dimgray" => Rgba([105, 105, 105, 255]),
        "dimgrey" => Rgba([105, 105, 105, 255]),
        "dodgerblue" => Rgba([30, 144, 255, 255]),
        "firebrick" => Rgba([178, 34, 34, 255]),
        "floralwhite" => Rgba([255, 250, 240, 255]),
        "forestgreen" => Rgba([34, 139, 34, 255]),
        "fuchsia" => Rgba([255, 0, 255, 255]),
        "gainsboro" => Rgba([220, 220, 220, 255]),
        "ghostwhite" => Rgba([248, 248, 255, 255]),
        "gold" => Rgba([255, 215, 0, 255]),
        "goldenrod" => Rgba([218, 165, 32, 255]),
        "gray" => Rgba([128, 128, 128, 255]),
        "green" => Rgba([0, 128, 0, 255]),
        "greenyellow" => Rgba([173, 255, 47, 255]),
        "grey" => Rgba([128, 128, 128, 255]),
        "honeydew" => Rgba([240, 255, 240, 255]),
        "hotpink" => Rgba([255, 105, 180, 255]),
        "indianred" => Rgba([205, 92, 92, 255]),
        "indigo" => Rgba([75, 0, 130, 255]),
        "ivory" => Rgba([255, 255, 240, 255]),
        "khaki" => Rgba([240, 230, 140, 255]),
        "lavender" => Rgba([230, 230, 250, 255]),
        "lavenderblush" => Rgba([255, 240, 245, 255]),
        "lawngreen" => Rgba([124, 252, 0, 255]),
        "lemonchiffon" => Rgba([255, 250, 205, 255]),
        "lightblue" => Rgba([173, 216, 230, 255]),
        "lightcoral" => Rgba([240, 128, 128, 255]),
        "lightcyan" => Rgba([224, 255, 255, 255]),
        "lightgoldenrodyellow" => Rgba([250, 250, 210, 255]),
        "lightgray" => Rgba([211, 211, 211, 255]),
        "lightgreen" => Rgba([144, 238, 144, 255]),
        "lightgrey" => Rgba([211, 211, 211, 255]),
        "lightpink" => Rgba([255, 182, 193, 255]),
        "lightsalmon" => Rgba([255, 160, 122, 255]),
        "lightseagreen" => Rgba([32, 178, 170, 255]),
        "lightskyblue" => Rgba([135, 206, 250, 255]),
        "lightslategray" => Rgba([119, 136, 153, 255]),
        "lightslategrey" => Rgba([119, 136, 153, 255]),
        "lightsteelblue" => Rgba([176, 196, 222, 255]),
        "lightyellow" => Rgba([255, 255, 224, 255]),
        "lime" => Rgba([0, 255, 0, 255]),
        "limegreen" => Rgba([50, 205, 50, 255]),
        "linen" => Rgba([250, 240, 230, 255]),
        "magenta" => Rgba([255, 0, 255, 255]),
        "maroon" => Rgba([128, 0, 0, 255]),
        "mediumaquamarine" => Rgba([102, 205, 170, 255]),
        "mediumblue" => Rgba([0, 0, 205, 255]),
        "mediumorchid" => Rgba([186, 85, 211, 255]),
        "mediumpurple" => Rgba([147, 112, 219, 255]),
        "mediumseagreen" => Rgba([60, 179, 113, 255]),
        "mediumslateblue" => Rgba([123, 104, 238, 255]),
        "mediumspringgreen" => Rgba([0, 250, 154, 255]),
        "mediumturquoise" => Rgba([72, 209, 204, 255]),
        "mediumvioletred" => Rgba([199, 21, 133, 255]),
        "midnightblue" => Rgba([25, 25, 112, 255]),
        "mintcream" => Rgba([245, 255, 250, 255]),
        "mistyrose" => Rgba([255, 228, 225, 255]),
        "moccasin" => Rgba([255, 228, 181, 255]),
        "navajowhite" => Rgba([255, 222, 173, 255]),
        "navy" => Rgba([0, 0, 128, 255]),
        "oldlace" => Rgba([253, 245, 230, 255]),
        "olive" => Rgba([128, 128, 0, 255]),
        "olivedrab" => Rgba([107, 142, 35, 255]),
        "orange" => Rgba([255, 165, 0, 255]),
        "orangered" => Rgba([255, 69, 0, 255]),
        "orchid" => Rgba([218, 112, 214, 255]),
        "palegoldenrod" => Rgba([238, 232, 170, 255]),
        "palegreen" => Rgba([152, 251, 152, 255]),
        "paleturquoise" => Rgba([175, 238, 238, 255]),
        "palevioletred" => Rgba([219, 112, 147, 255]),
        "papayawhip" => Rgba([255, 239, 213, 255]),
        "peachpuff" => Rgba([255, 218, 185, 255]),
        "peru" => Rgba([205, 133, 63, 255]),
        "pink" => Rgba([255, 192, 203, 255]),
        "plum" => Rgba([221, 160, 221, 255]),
        "powderblue" => Rgba([176, 224, 230, 255]),
        "purple" => Rgba([128, 0, 128, 255]),
        "rebeccapurple" => Rgba([102, 51, 153, 255]),
        "red" => Rgba([255, 0, 0, 255]),
        "rosybrown" => Rgba([188, 143, 143, 255]),
        "royalblue" => Rgba([65, 105, 225, 255]),
        "saddlebrown" => Rgba([139, 69, 19, 255]),
        "salmon" => Rgba([250, 128, 114, 255]),
        "sandybrown" => Rgba([244, 164, 96, 255]),
        "seagreen" => Rgba([46, 139, 87, 255]),
        "seashell" => Rgba([255, 245, 238, 255]),
        "sienna" => Rgba([160, 82, 45, 255]),
        "silver" => Rgba([192, 192, 192, 255]),
        "skyblue" => Rgba([135, 206, 235, 255]),
        "slateblue" => Rgba([106, 90, 205, 255]),
        "slategray" => Rgba([112, 128, 144, 255]),
        "slategrey" => Rgba([112, 128, 144, 255]),
        "snow" => Rgba([255, 250, 250, 255]),
        "springgreen" => Rgba([0, 255, 127, 255]),
        "steelblue" => Rgba([70, 130, 180, 255]),
        "tan" => Rgba([210, 180, 140, 255]),
        "teal" => Rgba([0, 128, 128, 255]),
        "thistle" => Rgba([216, 191, 216, 255]),
        "tomato" => Rgba([255, 99, 71, 255]),
        "turquoise" => Rgba([64, 224, 208, 255]),
        "violet" => Rgba([238, 130, 238, 255]),
        "wheat" => Rgba([245, 222, 179, 255]),
        "white" => Rgba([255, 255, 255, 255]),
        "whitesmoke" => Rgba([245, 245, 245, 255]),
        "yellow" => Rgba([255, 255, 0, 255]),
        "yellowgreen" => Rgba([154, 205, 50, 255]),

        // Default to white if the colour is not recognised.
        _ => Rgba([255, 255, 255, 255]),
    }
}
