//! Display units (spec section 4). Lengths are always stored in millimetres; a `Unit` only changes
//! display and input: in metres, typing `1.8` stores 1800mm.

mod format;
pub use format::{format_angle, format_length, format_number, wrap_degrees};
mod entry;
pub use entry::{parse_entry, parse_entry_plain, parse_length, parse_number, Entry};
mod expr;
pub(crate) use expr::*;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    #[serde(rename = "mm")]
    Millimetre,
    #[serde(rename = "cm")]
    Centimetre,
    #[serde(rename = "m")]
    Metre,
}

impl Default for Unit {
    fn default() -> Self {
        Unit::Millimetre
    }
}

impl Unit {
    pub const ALL: [Unit; 3] = [Unit::Millimetre, Unit::Centimetre, Unit::Metre];

    pub fn mm_per(self) -> f64 {
        match self {
            Unit::Millimetre => 1.0,
            Unit::Centimetre => 10.0,
            Unit::Metre => 1000.0,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Unit::Millimetre => "mm",
            Unit::Centimetre => "cm",
            Unit::Metre => "m",
        }
    }

    /// Decimals needed to round-trip a value: three for millimetres, six for metres.
    pub fn decimals(self) -> usize {
        match self {
            Unit::Millimetre => 4,
            Unit::Centimetre => 5,
            Unit::Metre => 7,
        }
    }

    pub fn from_mm(self, mm: f64) -> f64 {
        mm / self.mm_per()
    }

    pub fn to_mm(self, value: f64) -> f64 {
        value * self.mm_per()
    }
}
