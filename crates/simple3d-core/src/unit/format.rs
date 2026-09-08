//! Turning a number into the text a field shows.

use super::*;

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
    format_number(deg, ANGLE_DECIMALS)
}

/// How many decimals an angle is shown to.
pub(crate) const ANGLE_DECIMALS: usize = 4;

/// The direction a turn of `deg` leaves a body facing: the same turn brought
/// into `[0, 360)`, so 360 reads as 0 and -90 as 270 (issue 84).
///
/// A rotation is a direction, not a distance travelled to reach it: a body
/// turned a degree past a full turn stands where an untouched one does, and a
/// field that says 361 is describing the gesture rather than the model.
///
/// A hair under a full turn counts as a full turn. The field shows four
/// decimals, so 359.99999 in it is the string "360" -- a number outside the
/// range this promises, and one that would not survive being read back.
pub fn wrap_degrees(deg: f64) -> f64 {
    if !deg.is_finite() {
        return 0.0;
    }
    let wrapped = deg.rem_euclid(360.0);
    // Guarded rather than trusted: `rem_euclid` returns the divisor itself for
    // a tiny negative input, where 360 minus a hair rounds back to 360.
    if wrapped >= 360.0 - 0.5 * 0.1_f64.powi(ANGLE_DECIMALS as i32) {
        0.0
    } else {
        wrapped
    }
}
