//! The marks a row carries: the operator badge, the glyph and the tooltip.

use crate::app::App;
use crate::icon::Glyph;
use simple3d_core::scene::{GroupOp, NodeId, Scene};

/// The operator mark a node carries in the tree: its own, for a group, or the
/// one it is subject to, for a child of a difference or intersection. Reading
/// the boolean tree at a glance is the whole reason these are inline rather
/// than in a modifier stack somewhere else.
///
/// The flag says the mark is a *cut*: an operand being removed, drawn in the
/// danger colour, which is the one distinction worth a second colour here.
pub fn operator_badge(scene: &Scene, id: NodeId) -> Option<(Glyph, bool)> {
    // The root is a union of everything by definition; badging it says nothing
    // and puts a mark on the one row that never changes.
    if scene.node(id).parent.is_none() {
        return None;
    }
    if let Some(op) = scene.node(id).group_op() {
        return Some((symbol(op), false));
    }
    let parent = scene.node(id).parent?;
    let op = scene.node(parent).group_op()?;
    match op {
        // The base of a difference is what is being cut, not a cut.
        GroupOp::Difference if scene.difference_base(parent) != Some(id) => Some((symbol(op), true)),
        GroupOp::Intersection => Some((symbol(op), false)),
        _ => None,
    }
}

/// The set-theory symbols would be the obvious mark, but the bundled UI face
/// has no glyph for any of them and would draw three identical tofu boxes.
/// These are the same three shapes the tool rail's boolean buttons carry, which
/// makes the tree and the rail read as one vocabulary.
pub(crate) fn symbol(op: GroupOp) -> Glyph {
    match op {
        GroupOp::Union => Glyph::Union,
        GroupOp::Difference => Glyph::Difference,
        GroupOp::Intersection => Glyph::Intersection,
        GroupOp::Hull => Glyph::Polyhedron,
    }
}

/// The mark a node wears in the tree, by what kind of body it is.
///
/// One function rather than the same chain of tests written out at each of the
/// three places a row's mark is drawn -- the row itself, the drag slab, and the
/// palette's own tiles -- because a body type added without a mark shows up as
/// a box in some of them and not others, which is worse than showing up as a
/// box in all three.
pub(crate) fn node_glyph(node: &simple3d_core::scene::Node) -> Glyph {
    if node.is_mesh() {
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
        // The one row stands for all of them, so it has to say how many there
        // are: the tree cannot show it and the count is the whole point of the
        // row being one (issue 82).
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
    if let Some(mesh) = app.evaluated.node_meshes.get(&id) {
        if let Some((lo, hi)) = mesh.bounds() {
            lines.push(crate::ui::describe_size(hi - lo, app.unit()));
        }
    }
    lines.join("\n")
}
