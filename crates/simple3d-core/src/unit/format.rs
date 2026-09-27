//! Turning a number into the text a field shows.

use super::*;

/// Format a number without float noise (`1.8`, not `1.7999999999`), trimming trailing zeros.
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

/// Format a stored millimetre length in the display unit.
pub fn format_length(mm: f64, unit: Unit) -> String {
    format_number(unit.from_mm(mm), unit.decimals())
}

/// Format an angle; always degrees regardless of the length unit (spec section 4).
pub fn format_angle(deg: f64) -> String {
    format_number(deg, ANGLE_DECIMALS)
}

/// How many decimals an angle is shown to.
pub(crate) const ANGLE_DECIMALS: usize = 4;

/// The direction a turn of `deg` leaves a body facing, in `[0, 360)` (issue 84). Just under 360
/// counts as 0, since it would display as "360".
pub fn wrap_degrees(deg: f64) -> f64 {
    if !deg.is_finite() {
        return 0.0;
    }
    let wrapped = deg.rem_euclid(360.0);
    // Guarded: `rem_euclid` returns the divisor for tiny negative inputs.
    if wrapped >= 360.0 - 0.5 * 0.1_f64.powi(ANGLE_DECIMALS as i32) {
        0.0
    } else {
        wrapped
    }
}
