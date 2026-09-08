//! Display units (spec section 4).
//!
//! Lengths are stored in millimetres everywhere. A `Unit` only changes how a
//! number is shown and read back, never the model: with metres selected,
//! typing `1.8` stores 1800mm and the field afterwards reads `1.8`.

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

    /// Millimetres per one of this unit.
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

    /// Decimal places worth showing so that a value entered in this unit round
    /// -trips: millimetres are stored exactly, so three places is ample; metres
    /// need six to express a single millimetre.
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
