//! Laying out the boolean a drag changes as a postfix expression over its shapes.

use super::*;
use simple3d_core::scene::{Body, GroupOp, NodeId};
use simple3d_core::xform::Xform;

/// The resolve pass's stack depth and maximum expression length, including the section.
const CSG_STACK: usize = 32;
const CSG_PROGRAM: usize = 256;

impl App {
    /// The boolean a drag of `carried` changes: the group that is its own scene range, its leaves
    /// and the expression. `None` if `carried` is its own range, a group cannot be drawn this way,
    /// or there are too many shapes.
    pub(crate) fn csg_plan(&self, carried: NodeId) -> Option<(NodeId, Vec<PlannedLeaf>, Vec<i32>)> {
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
        path.push(group);
        // A hull is built from its operands' points only while nothing between it and the dragged body
        // removes or repeats material; and at most one hull is rebuilt per frame.
        let mut hulls = 0;
        for (index, &id) in path[1..].iter().enumerate() {
            match &self.scene.node(id).body {
                Body::Group { op: GroupOp::Hull } => {
                    let below = &path[1..=index];
                    let unions = below.iter().all(|&id| {
                        matches!(self.scene.node(id).body, Body::Group { op: GroupOp::Union | GroupOp::Assembly })
                    });
                    hulls += 1;
                    if !unions || hulls > 1 {
                        return None;
                    }
                }
                Body::Group { .. } | Body::Pattern { .. } => {}
                _ => return None,
            }
        }
        let mut plan = Plan { carried, path: &path, leaves: Vec::new(), program: Vec::new() };
        self.csg_expression(group, None, &mut plan)?;
        let Plan { leaves, program, .. } = plan;
        let fits = leaves.len() <= CSG_LEAVES && program.len() + 2 <= CSG_PROGRAM && stack_depth(&program) < CSG_STACK;
        fits.then_some((group, leaves, program))
    }

    /// `node` as an expression: expanded if it is the group or on the path to the carried body,
    /// otherwise a leaf. `copy` is the pattern copy being laid out.
    fn csg_expression(&self, node: NodeId, copy: Option<Xform>, plan: &mut Plan<'_>) -> Option<()> {
        if plan.leaves.len() > CSG_LEAVES {
            return None;
        }
        // `path` runs from the carried body up to the group, so any other node on it is an enclosing group.
        if node == plan.carried || !plan.path.contains(&node) {
            plan.program.push(plan.leaves.len() as i32);
            plan.leaves.push(PlannedLeaf { shape: Shape::Node(node), copy, carried: node == plan.carried });
            return Some(());
        }
        match &self.scene.node(node).body {
            Body::Group { op: GroupOp::Hull } => {
                plan.program.push(plan.leaves.len() as i32);
                plan.leaves.push(PlannedLeaf { shape: Shape::Hull(node), copy, carried: true });
                Some(())
            }
            Body::Group { op } => {
                let op = match op {
                    GroupOp::Union | GroupOp::Assembly => CSG_UNION,
                    GroupOp::Difference => CSG_DIFFERENCE,
                    GroupOp::Intersection => CSG_INTERSECTION,
                    GroupOp::Hull => unreachable!("taken above"),
                };
                self.csg_children(node, op, copy, plan)
            }
            // Copies side by side, unioned like the evaluation does. Each copy's world move is the pattern's
            // frame, the copy, and the frame undone.
            Body::Pattern { params } => {
                let first = self.scene.node(node).children.first()?;
                let frame = *self.evaluated.node_frames.get(first)?;
                let back = frame.inverse();
                let mut first_copy = true;
                for instance in simple3d_core::pattern::instances(params) {
                    let moved = frame.compose(&instance.xform).compose(&back);
                    let placed = Some(copy.map_or(moved, |copy| copy.compose(&moved)));
                    self.csg_children(node, CSG_UNION, placed, plan)?;
                    if !first_copy {
                        plan.program.push(CSG_UNION);
                    }
                    first_copy = false;
                }
                (!first_copy).then_some(())
            }
            _ => None,
        }
    }

    /// A group's shown children with `op` between each pair.
    fn csg_children(&self, node: NodeId, op: i32, copy: Option<Xform>, plan: &mut Plan<'_>) -> Option<()> {
        let mut first = true;
        for &child in &self.scene.node(node).children {
            if !self.scene.node(child).visible
                || self.evaluated.result_mesh(child).is_none_or(|mesh| mesh.indices.is_empty())
            {
                continue;
            }
            self.csg_expression(child, copy, plan)?;
            if !first {
                plan.program.push(op);
            }
            first = false;
        }
        // An empty group cannot be expressed here, so it waits for evaluation.
        (!first).then_some(())
    }
}

/// The expression being laid out, and what it is laid out around.
struct Plan<'a> {
    carried: NodeId,
    path: &'a [NodeId],
    leaves: Vec<PlannedLeaf>,
    program: Vec<i32>,
}

/// The maximum stack depth while evaluating a postfix expression.
fn stack_depth(program: &[i32]) -> usize {
    let (mut depth, mut deepest) = (0usize, 0usize);
    for &op in program {
        if op >= 0 {
            depth += 1;
            deepest = deepest.max(depth);
        } else {
            depth = depth.saturating_sub(1);
        }
    }
    deepest
}
