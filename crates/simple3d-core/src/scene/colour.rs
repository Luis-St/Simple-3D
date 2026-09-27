//! The colour a node carries, and the tag the mesh records it under.

/// A node's paint colour as sRGB bytes, saved as `#rrggbb` to stay readable and diffable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colour(pub [u8; 3]);

impl Colour {
    /// Tag 0, which generators produce, means unpainted; the top byte marks painted tags.
    pub const UNPAINTED: u32 = 0;

    pub fn tag(self) -> u32 {
        simple3d_geom::colour_tag(self.0)
    }

    /// The colour a tag stands for, or `None` if unpainted.
    pub fn from_tag(tag: u32) -> Option<Colour> {
        simple3d_geom::tag_colour(tag).map(Colour)
    }

    pub fn to_hex(self) -> String {
        let [r, g, b] = self.0;
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    /// Parse `#rrggbb` or `rrggbb`; anything else loads as unpainted rather than failing the file.
    pub fn from_hex(text: &str) -> Option<Colour> {
        let digits = text.strip_prefix('#').unwrap_or(text);
        if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
        Some(Colour([byte(0)?, byte(2)?, byte(4)?]))
    }
}

/// The tag for a node's effective colour.
pub fn colour_tag(colour: Option<Colour>) -> u32 {
    colour.map_or(Colour::UNPAINTED, Colour::tag)
}
