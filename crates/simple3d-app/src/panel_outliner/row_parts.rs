//! The marks a row carries: the operator badge, the glyph and the tooltip.

use crate::app::App;
use crate::icon::Glyph;
use simple3d_core::scene::{GroupOp, NodeId, Scene};

/// The operator mark a node carries: its own for a group, or the one it is subject to for a child
/// of a difference or intersection. The flag marks a cut operand, drawn in the danger colour.
pub fn operator_badge(scene: &Scene, id: NodeId) -> Option<(Glyph, bool)> {
    // The root is always a union; badging it says nothing.
    if scene.node(id).parent.is_none() {
        return None;
    }
    if let Some(op) = scene.node(id).group_op() {
        return Some((symbol(op), false));
    }
    let parent = scene.node(id).parent?;
    let op = scene.node(parent).group_op()?;
    match op {
        // A difference's base is what is cut, not a cut.
        GroupOp::Difference if scene.difference_base(parent) != Some(id) => Some((symbol(op), true)),
        GroupOp::Intersection => Some((symbol(op), false)),
        _ => None,
    }
}

/// The rail's boolean glyphs, since the UI font lacks the set-theory symbols.
pub(crate) fn symbol(op: GroupOp) -> Glyph {
    match op {
        GroupOp::Union => Glyph::Union,
        GroupOp::Difference => Glyph::Difference,
        GroupOp::Intersection => Glyph::Intersection,
        GroupOp::Hull => Glyph::Polyhedron,
        GroupOp::Assembly => Glyph::Group,
    }
}

/// A node's tree mark by body kind, in one place for the row, drag slab and palette tiles.
pub(crate) fn node_glyph(node: &simple3d_core::scene::Node) -> Glyph {
    if node.is_component() {
        Glyph::Component
    } else if node.is_mesh() {
        Glyph::Mesh
    } else if node.is_split() {
        Glyph::Split
    } else if node.is_pattern() {
        Glyph::Pattern
    } else if node.is_group() {
        Glyph::Bracket
    } else {
        Glyph::for_primitive(node.spec().map(|s| s.type_id).unwrap_or(""))
    }
}

pub(crate) fn hover_text(
    app: &App,
    id: NodeId,
    is_group: bool,
    op: Option<simple3d_core::scene::GroupOp>,
    base_child: Option<NodeId>,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(error) = app.evaluated.error_for(id) {
        lines.push(error.message.clone());
    }
    if is_group {
        if let Some(op) = op {
            lines.push(format!("{} of {} children", op.label(), app.scene.node(id).children.len()));
            if op.order_matters() {
                match base_child {
                    Some(base) => {
                        lines.push(format!("Base: {} (everything below it is subtracted)", app.scene.node(base).name))
                    }
                    None => lines.push("No visible child to use as the base".into()),
                }
            }
        }
    } else if app.scene.is_collection(id) {
        // One row stands for all the pieces, so it shows the count (issue 82).
        let node = app.scene.node(id);
        let total = node.children.len();
        let shown = app.scene.row_children(id).len();
        lines.push(match (total, shown) {
            (1, _) => "1 piece, held inside".to_string(),
            (n, 0) => format!("{n} pieces, held inside"),
            (n, out) => format!("{n} pieces, {out} of them extracted"),
        });
    } else if let Some(spec) = app.scene.node(id).spec() {
        lines.push(spec.label.to_string());
    }
    if app.evaluated.node_meshes.contains_key(&id) {
        if let Some((lo, hi)) = app.evaluated.node_world_bounds.get(&id) {
            lines.push(crate::ui::describe_size(*hi - *lo, app.unit()));
        }
    }
    lines.join("\n")
}
