//! A boolean drawn on the card while one of its operands is dragged.

use super::*;
use crate::render::Renderable;
use simple3d_core::scene::{Body, GroupOp, NodeId};
use simple3d_core::xform::Xform;
use std::sync::Arc;

/// A drag the GPU draws as the boolean it changes: the group whose result is
/// in the scene as it came, as an expression over the shapes that go into it,
/// with the dragged one where the drag has got to. See `gpu/csg.rs`.
pub(crate) struct LiveCsg {
    /// The group's stretch of the scene's vertices, left out while it is
    /// drawn this way.
    pub range: std::ops::Range<u32>,
    /// The shapes, each as the last evaluation made it, in world space.
    pub leaves: Vec<Arc<Renderable>>,
    /// Which of them is the dragged one, which node that is, and how far it
    /// has moved.
    pub moved: usize,
    pub carried: NodeId,
    pub xform: Xform,
    /// The expression, in postfix: a leaf by its index, or one of the
    /// operators below applied to the two values before it.
    pub program: Vec<i32>,
}

pub(crate) const CSG_UNION: i32 = -1;
pub(crate) const CSG_DIFFERENCE: i32 = -2;
pub(crate) const CSG_INTERSECTION: i32 = -3;

/// How many shapes an expression may have: one bit of a mask each, with one
/// kept back for the section plane.
pub(crate) const CSG_LEAVES: usize = 31;

impl App {
    /// The boolean a drag changes, when the GPU can draw it without the
    /// scene being evaluated (see [`LiveCsg`]).
    ///
    /// For a body that is not a stretch of the scene of its own -- one inside
    /// a union it meets, a cutter, a difference's base -- and only while every
    /// group from it up to the nearest one that is such a stretch is a union,
    /// a difference, an intersection or an assembly: a hull, a pattern or a
    /// split of it is a shape the card cannot work out per pixel, and those
    /// still wait for the evaluation.
    pub(crate) fn live_csg(&self) -> Option<LiveCsg> {
        self.gpu.as_ref()?;
        let carried = match &self.drag {
            Some(drag) => {
                let node = self.scene.get(drag.node)?;
                if node.params().cloned().unwrap_or_default() != drag.start_params {
                    return None;
                }
                drag.node
            }
            None => self.settling?,
        };
        let (group, leaves, program) = self.csg_plan(carried)?;
        let moved = leaves.iter().position(|&leaf| leaf == carried)?;
        let leaves = leaves.into_iter().map(|leaf| self.csg_leaf(leaf)).collect::<Option<Vec<_>>>()?;
        Some(LiveCsg {
            range: self.scene_renderable.parts.get(&group)?.clone(),
            leaves,
            moved,
            carried,
            xform: self.moved_by(carried)?,
            program,
        })
    }

    /// The boolean a drag of `carried` changes: the group whose result is a
    /// stretch of the scene of its own, the shapes of the expression below it
    /// -- `carried` among them -- and the expression over them. `None` when
    /// `carried` is a stretch of its own (a plain live drag moves it), when a
    /// group on the way up cannot be drawn per pixel, or when there are more
    /// shapes than the masks hold.
    pub(crate) fn csg_plan(&self, carried: NodeId) -> Option<(NodeId, Vec<NodeId>, Vec<i32>)> {
        let parts = &self.scene_renderable.parts;
        if parts.contains_key(&carried) {
            return None;
        }
        let mut path = vec![carried];
        let group = loop {
            let parent = self.scene.node(*path.last()?).parent?;
            if parts.contains_key(&parent) {
                break parent;
            }
            path.push(parent);
        };
        if path[1..].iter().chain([&group]).any(|&id| csg_op(&self.scene.node(id).body).is_none()) {
            return None;
        }
        let mut leaves = Vec::new();
        let mut program = Vec::new();
        self.csg_expression(group, carried, &path, &mut leaves, &mut program)?;
        (leaves.len() <= CSG_LEAVES).then_some((group, leaves, program))
    }

    /// `node` as an expression: opened up into its children when it is the
    /// group or a group on the way down to the carried body, a leaf otherwise.
    fn csg_expression(
        &self,
        node: NodeId,
        carried: NodeId,
        path: &[NodeId],
        leaves: &mut Vec<NodeId>,
        program: &mut Vec<i32>,
    ) -> Option<()> {
        // `path` runs from the carried body up to just below the group, so a
        // node on it other than the body itself is a group it is inside of.
        let open = node != carried && (Some(node) == self.scene.node(*path.last()?).parent || path.contains(&node));
        if !open {
            program.push(leaves.len() as i32);
            leaves.push(node);
            return Some(());
        }
        let op = csg_op(&self.scene.node(node).body)?;
        let mut first = true;
        for &child in &self.scene.node(node).children {
            if !self.scene.node(child).visible
                || self.evaluated.result_mesh(child).is_none_or(|mesh| mesh.indices.is_empty())
            {
                continue;
            }
            self.csg_expression(child, carried, path, leaves, program)?;
            if !first {
                program.push(op);
            }
            first = false;
        }
        // A group with nothing in it is nothing, which no expression here can
        // say; such a group waits for the evaluation.
        (!first).then_some(())
    }

    /// A shape as the last evaluation made it, kept for as long as that
    /// evaluation is on screen.
    fn csg_leaf(&self, id: NodeId) -> Option<Arc<Renderable>> {
        let generation = self.evaluation_generation;
        if let Some((made, leaf)) = self.csg_leaves.borrow().get(&id) {
            if *made == generation {
                return Some(leaf.clone());
            }
        }
        let mesh = self.evaluated.result_mesh(id)?.into_owned();
        let leaf = Arc::new(Renderable::surface(mesh));
        self.csg_leaves.borrow_mut().insert(id, (generation, leaf.clone()));
        Some(leaf)
    }
}

/// The operator a group applies to its children, as the expression spells
/// it, or `None` for one the card cannot draw per pixel.
fn csg_op(body: &Body) -> Option<i32> {
    match body {
        Body::Group { op } => match op {
            GroupOp::Union | GroupOp::Assembly => Some(CSG_UNION),
            GroupOp::Difference => Some(CSG_DIFFERENCE),
            GroupOp::Intersection => Some(CSG_INTERSECTION),
            GroupOp::Hull => None,
        },
        _ => None,
    }
}
