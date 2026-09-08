//! The small pieces of wording the interface asks for.

use super::*;
use simple3d_core::config::Placement;

/// Whether a colour is one of the fixed swatches the palette rows already
/// offer.
pub(crate) fn is_preset(rgb: [u8; 3]) -> bool {
    crate::theme::PAINT_PRESETS.iter().any(|(_, colour)| [colour.r(), colour.g(), colour.b()] == rgb)
}

pub(crate) fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

pub(crate) fn children_plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "ren"
    }
}

/// Where the next shape will land, in words. The palette says this in its hint
/// line and in every tile's tooltip, so the two can never disagree about it.
pub fn insertion_hint(app: &App) -> String {
    let unit = app.unit();
    // Nothing is being added yet, so nothing has a width to clear: under
    // "beside the selection" this is the line the next shape's near side will
    // meet, and the wording below says so rather than passing it off as the
    // point the shape's origin will sit at.
    let at = app.insertion_point_world(0.0);
    let where_ = format!(
        "{}, {}, {} {}",
        simple3d_core::unit::format_length(at.x, unit),
        simple3d_core::unit::format_length(at.y, unit),
        simple3d_core::unit::format_length(at.z, unit),
        unit.suffix()
    );
    match app.settings.placement {
        Placement::Origin => format!("Lands at the origin: {where_}."),
        Placement::Cursor if app.cursor.is_some() => format!("Lands at the 3D cursor: {where_}."),
        Placement::Cursor => {
            format!("Lands at {where_}. Shift+right-click in the viewport to put the 3D cursor somewhere else.")
        }
        Placement::ViewCentre => format!("Lands at what the camera is looking at: {where_}."),
        Placement::BesideSelection => format!(
            "Lands clear of the selection, its near side at {} {} on X.",
            simple3d_core::unit::format_length(at.x, unit),
            unit.suffix()
        ),
    }
}
