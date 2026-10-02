//! Small pieces of interface wording.

use super::*;
use simple3d_core::config::Placement;

pub(crate) use crate::ui::plural;

/// Whether a colour is one of the fixed palette swatches.
pub(crate) fn is_preset(rgb: [u8; 3]) -> bool {
    crate::theme::PAINT_PRESETS.iter().any(|(_, colour)| [colour.r(), colour.g(), colour.b()] == rgb)
}

pub(crate) fn children_plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "ren"
    }
}

/// The status after adding `id`: where it went too when its group combines it with the rest, since
/// a shape added into a difference silently became a cut.
pub(crate) fn added(app: &App, id: simple3d_core::scene::NodeId) -> String {
    use simple3d_core::scene::GroupOp;
    let node = app.scene.node(id);
    let Some(parent) = node.parent.and_then(|parent| app.scene.get(parent)) else {
        return format!("Added {}", node.name);
    };
    let does = match parent.group_op() {
        Some(GroupOp::Difference) if parent.children.first() == Some(&id) => "it is the base the rest cut",
        Some(GroupOp::Difference) => "it cuts the base",
        Some(GroupOp::Intersection) => "only where it overlaps the rest is kept",
        Some(GroupOp::Hull) => "it joins the hull",
        _ => return format!("Added {}", node.name),
    };
    format!("Added {} to {}: {does}", node.name, parent.name)
}

/// Where the next shape lands, in words; shared by the hint line and tile tooltips so they agree.
pub fn insertion_hint(app: &App) -> String {
    let unit = app.unit();
    // With no shape yet there is no width to clear, so "beside the selection" names the line its near side
    // meets, not where its origin sits.
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
