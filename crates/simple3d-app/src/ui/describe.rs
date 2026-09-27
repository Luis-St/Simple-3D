//! Small pieces of interface wording.

use simple3d_core::unit::{format_length, format_number, Unit};

/// A bounding box in words, for the "will this fit" overlay (spec section 6.1).
pub fn describe_size(size: simple3d_geom::Vec3, unit: Unit) -> String {
    format!(
        "{} x {} x {} {}",
        format_length(size.x, unit),
        format_length(size.y, unit),
        format_length(size.z, unit),
        unit.suffix()
    )
}

/// A point in the display unit.
pub fn describe_point(p: simple3d_geom::Vec3, unit: Unit) -> String {
    format!(
        "{}, {}, {} {}",
        format_length(p.x, unit),
        format_length(p.y, unit),
        format_length(p.z, unit),
        unit.suffix()
    )
}

/// The status bar's node and triangle counts; `nodes` excludes the scene root.
pub fn describe_counts(nodes: usize, triangles: usize) -> String {
    format!("{nodes} node{}  {triangles} triangle{}", plural(nodes), plural(triangles))
}

pub(crate) fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// A duration for the status bar, to meaningful precision.
pub fn describe_elapsed(elapsed: std::time::Duration) -> String {
    let millis = elapsed.as_secs_f64() * 1000.0;
    // Three significant figures: whole milliseconds made most evaluations read "0 ms", which looks broken.
    if millis < 10.0 {
        format!("{} ms", format_number(millis, 2))
    } else if millis < 100.0 {
        format!("{} ms", format_number(millis, 1))
    } else if millis < 1000.0 {
        format!("{} ms", format_number(millis, 0))
    } else {
        format!("{} s", format_number(millis / 1000.0, 2))
    }
}
