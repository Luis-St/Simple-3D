//! The small pieces of wording the interface asks for.

use simple3d_core::unit::{format_length, format_number, Unit};

/// A short description of a bounding box, for the overlay that answers "will
/// this fit" (spec section 6.1).
pub fn describe_size(size: simple3d_geom::Vec3, unit: Unit) -> String {
    format!(
        "{} x {} x {} {}",
        format_length(size.x, unit),
        format_length(size.y, unit),
        format_length(size.z, unit),
        unit.suffix()
    )
}

/// A point, in the display unit, for a message that names one.
pub fn describe_point(p: simple3d_geom::Vec3, unit: Unit) -> String {
    format!(
        "{}, {}, {} {}",
        format_length(p.x, unit),
        format_length(p.y, unit),
        format_length(p.z, unit),
        unit.suffix()
    )
}

/// The status bar's triangle and node counts. `nodes` is what the document
/// holds -- the scene itself is not one of them, and counting it made an empty
/// document report one node with nothing in it.
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

/// Format a duration for the status bar without pretending to more precision
/// than is meaningful.
pub fn describe_elapsed(elapsed: std::time::Duration) -> String {
    let millis = elapsed.as_secs_f64() * 1000.0;
    // Rounded to whole milliseconds, an evaluation that takes a fifth of one --
    // which most of them do -- reads as "0 ms", and a readout that says zero
    // reads as a readout that is not working. Two decimals under 10 ms, one
    // under 100, none above: always three significant figures of something
    // that actually happened.
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
