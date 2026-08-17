//! Display units (spec section 4).
//!
//! Lengths are stored in millimetres everywhere. A `Unit` only changes how a
//! number is shown and read back, never the model: with metres selected,
//! typing `1.8` stores 1800mm and the field afterwards reads `1.8`.

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

/// Format a number for display without floating-point noise: `1.8`, never
/// `1.7999999999`. Trailing zeros and a trailing point are trimmed.
pub fn format_number(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    let mut s = format!("{value:.decimals$}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

/// Format a stored millimetre length in the given display unit.
pub fn format_length(mm: f64, unit: Unit) -> String {
    format_number(unit.from_mm(mm), unit.decimals())
}

/// Format an angle in degrees. Angles are always degrees regardless of the
/// length unit (spec section 4).
pub fn format_angle(deg: f64) -> String {
    format_number(deg, 4)
}

/// Parse a number accepting either decimal separator, ignoring surrounding
/// whitespace and a trailing unit suffix the user may have typed. `None` on
/// anything unparseable -- callers restore the previous value silently rather
/// than raising a dialog (spec section 4).
pub fn parse_number(text: &str) -> Option<f64> {
    let mut s = text.trim().to_string();
    for suffix in ["mm", "cm", "m", "deg", "°"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            s = stripped.trim_end().to_string();
            break;
        }
    }
    // Both `1.8` and `1,8` mean the same thing. A thousands separator is not
    // supported and would be a parse failure, which is the safe outcome.
    let s = s.replace(',', ".");
    if s.is_empty() {
        return None;
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Parse a length typed in `unit` into stored millimetres.
pub fn parse_length(text: &str, unit: Unit) -> Option<f64> {
    parse_number(text).map(|v| unit.to_mm(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_unit_round_trips_without_rescaling() {
        // Spec acceptance criterion 6.
        let plate = [40.0, 20.0, 4.0];
        let shown: Vec<String> = plate.iter().map(|&mm| format_length(mm, Unit::Metre)).collect();
        assert_eq!(shown, vec!["0.04", "0.02", "0.004"]);
        for (&mm, s) in plate.iter().zip(shown.iter()) {
            assert_eq!(parse_length(s, Unit::Metre), Some(mm));
        }
        for &mm in &plate {
            assert_eq!(parse_length(&format_length(mm, Unit::Millimetre), Unit::Millimetre), Some(mm));
        }
    }

    #[test]
    fn typing_in_metres_stores_millimetres() {
        assert_eq!(parse_length("1.8", Unit::Metre), Some(1800.0));
        assert_eq!(format_length(1800.0, Unit::Metre), "1.8");
    }

    #[test]
    fn both_decimal_separators_are_accepted() {
        assert_eq!(parse_number("1,8"), Some(1.8));
        assert_eq!(parse_number("1.8"), Some(1.8));
        assert_eq!(parse_number(" 12 mm "), Some(12.0));
        assert_eq!(parse_number("-0,5"), Some(-0.5));
    }

    #[test]
    fn garbage_is_rejected_rather_than_guessed() {
        // Acceptance criterion 14: the caller restores the previous value.
        for bad in ["", "  ", "abc", "1.2.3", "1,2,3", "--4", "NaN", "inf", "12mmm"] {
            assert_eq!(parse_number(bad), None, "{bad:?} should not parse");
        }
    }

    #[test]
    fn no_floating_point_noise_in_output() {
        assert_eq!(format_number(0.1 + 0.2, 4), "0.3");
        assert_eq!(format_number(1.7999999999, 4), "1.8");
        assert_eq!(format_number(-0.00001, 4), "0");
        assert_eq!(format_number(5.0, 4), "5");
    }
}
