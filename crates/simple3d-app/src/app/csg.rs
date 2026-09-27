//! A boolean drawn on the card while one of its operands is dragged.

use super::*;
use crate::render::Renderable;
use simple3d_core::scene::{Body, GroupOp, NodeId};
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;
use std::sync::Arc;

/// A drag the GPU draws as the boolean it changes: the affected group as an expression over its
/// shapes, with the dragged one moved. See `gpu/csg.rs`.
pub(crate) struct LiveCsg {
    /// The group's vertex range in the scene, hidden while drawn this way.
    pub range: std::ops::Range<u32>,
    /// The shapes from the last evaluation, in world space, each with its move (the drag's, a
    /// pattern copy's, or both).
    pub leaves: Vec<(Arc<Renderable>, Option<Xform>)>,
    /// Which node is dragged, and how far it has moved.
    pub carried: NodeId,
    pub xform: Xform,
    /// The expression in postfix: a leaf index, or an operator on the two values before it.
    pub program: Vec<i32>,
}

pub(crate) const CSG_UNION: i32 = -1;
pub(crate) const CSG_DIFFERENCE: i32 = -2;
pub(crate) const CSG_INTERSECTION: i32 = -3;

/// Most shapes an expression may have: one mask bit each, one reserved for the section.
/// See `gpu::csg::MAX_SHAPES`.
pub(crate) const CSG_LEAVES: usize = 127;

/// The resolve pass's stack depth and maximum expression length, including the section.
const CSG_STACK: usize = 32;
const CSG_PROGRAM: usize = 256;

/// One shape of the expression, as [`App::csg_plan`] lays it out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PlannedLeaf {
    pub shape: Shape,
    /// The pattern copy it is in, as one world-space move; `None` outside any pattern.
    pub copy: Option<Xform>,
    pub carried: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    /// A node's shape from the last evaluation.
    Node(NodeId),
    /// A hull stretched over the dragged body, rebuilt per frame (see [`App::csg_hull`]).
    Hull(NodeId),
}

/// A hull containing the dragged body, reduced once per drag to the hull points of the static
/// part and of the dragged body; each frame hulls those together.
pub(crate) struct HullCache {
    generation: u64,
    carried: NodeId,
    still: Vec<Vec3>,
    moving: Vec<Vec3>,
    tag: u32,
    last: Option<(Xform, Arc<Renderable>)>,
}

impl App {
    /// The boolean a drag changes, when the GPU can draw it without evaluation (see [`LiveCsg`]).
    ///
    /// Only for a body that is not its own scene range, and only while every group up to the
    /// nearest one that is can be drawn on the card (union, difference, intersection, assembly,
    /// pattern, hull). A split still waits for evaluation.
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
        let (group, planned, program) = self.csg_plan(carried)?;
        let xform = self.moved_by(carried)?;
        let mut leaves = Vec::with_capacity(planned.len());
        for leaf in planned {
            let (shape, moved) = match leaf.shape {
                Shape::Node(id) => (self.csg_leaf(id)?, leaf.carried.then_some(xform)),
                Shape::Hull(id) => (self.csg_hull(id, carried, xform)?, None),
            };
            let placed = match (leaf.copy, moved) {
                (Some(copy), Some(moved)) => Some(copy.compose(&moved)),
                (copy, moved) => copy.or(moved),
            };
            leaves.push((shape, placed));
        }
        Some(LiveCsg { range: self.scene_renderable.parts.get(&group)?.clone(), leaves, carried, xform, program })
    }

    /// The shapes [`App::live_csg`] would draw for the selection, prepared before any drag starts,
    /// so the first drag frame does not hang building and uploading them.
    pub(crate) fn csg_ready(&self) -> Vec<Arc<Renderable>> {
        if self.gpu.is_none() || self.drag.is_some() || self.settling.is_some() {
            return Vec::new();
        }
        let Some(id) = self.primary() else { return Vec::new() };
        let Some((_, planned, _)) = self.csg_plan(id) else { return Vec::new() };
        let shape = |leaf: PlannedLeaf| match leaf.shape {
            Shape::Node(node) => self.csg_leaf(node),
            Shape::Hull(hull) => self.csg_hull(hull, id, Xform::IDENTITY),
        };
        let mut shapes: Vec<Arc<Renderable>> = Vec::new();
        for shape in planned.into_iter().filter_map(shape) {
            // A pattern's copies are one shape drawn several times.
            if !shapes.iter().any(|held| Arc::ptr_eq(held, &shape)) {
                shapes.push(shape);
            }
        }
        shapes
    }

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

    /// A shape from the last evaluation, cached while it is on screen, with feature edges when the
    /// display mode draws lines (`Gpu::draw_csg_edges`).
    pub(crate) fn csg_leaf(&self, id: NodeId) -> Option<Arc<Renderable>> {
        let generation = self.evaluation_generation;
        let edged = self.settings.display_mode == simple3d_core::config::DisplayMode::ShadedWithEdges;
        if let Some((made, with_edges, leaf)) = self.csg_leaves.borrow().get(&id) {
            if *made == generation && (*with_edges || !edged) {
                return Some(leaf.clone());
            }
        }
        let mesh = self.evaluated.result_mesh(id)?;
        let leaf = Arc::new(match edged {
            true => Renderable::surface_with_edges(&mesh),
            false => Renderable::surface(mesh.into_owned()),
        });
        self.csg_leaves.borrow_mut().insert(id, (generation, edged, leaf.clone()));
        Some(leaf)
    }

    /// The hull `id` makes with `carried` moved by `moved`, in world space.
    ///
    /// Built on the CPU, since a pixel cannot test hull membership. The hull of points is the hull of
    /// their hulls, so the operands are reduced to their hulls once per drag.
    pub(crate) fn csg_hull(&self, id: NodeId, carried: NodeId, moved: Xform) -> Option<Arc<Renderable>> {
        let generation = self.evaluation_generation;
        let mut hulls = self.csg_hulls.borrow_mut();
        let fresh = hulls.get(&id).is_some_and(|hull| hull.generation == generation && hull.carried == carried);
        if !fresh {
            hulls.insert(id, self.hull_cache(id, carried)?);
        }
        let hull = hulls.get_mut(&id)?;
        if let Some((at, shape)) = &hull.last {
            if *at == moved {
                return Some(shape.clone());
            }
        }
        let points: Vec<Vec3> = hull.still.iter().copied().chain(hull.moving.iter().map(|&p| moved.point(p))).collect();
        let mut mesh = simple3d_geom::hull::convex_hull(&points);
        mesh.set_tag(hull.tag);
        let shape = Arc::new(Renderable::surface(mesh));
        hull.last = Some((moved, shape.clone()));
        Some(shape)
    }

    /// The reduced hull points [`App::csg_hull`] builds from: `carried`'s and everything else's
    /// under `id`.
    fn hull_cache(&self, id: NodeId, carried: NodeId) -> Option<HullCache> {
        let shown = |node: NodeId| {
            let node = self.scene.node(node);
            node.children.iter().copied().filter(|&child| self.scene.node(child).visible).collect::<Vec<_>>()
        };
        // Only unions are allowed below a hull (`csg_plan`), so siblings off the path are operands too.
        let mut still = Vec::new();
        let mut at = carried;
        while at != id {
            let parent = self.scene.node(at).parent?;
            for sibling in shown(parent).into_iter().filter(|&sibling| sibling != at) {
                if let Some(mesh) = self.evaluated.result_mesh(sibling) {
                    still.extend(mesh.positions.iter().copied());
                }
            }
            at = parent;
        }
        let moving = self.evaluated.result_mesh(carried)?.positions.clone();
        // The hull takes its first operand's colour, as the evaluation does.
        let first = shown(id).into_iter().find_map(|child| self.evaluated.result_mesh(child));
        let tag = first.map_or(0, |mesh| mesh.tag(0));
        let corners = |points: &[Vec3]| simple3d_geom::hull::convex_hull(points).positions;
        Some(HullCache {
            generation: self.evaluation_generation,
            carried,
            still: if still.is_empty() { still } else { corners(&still) },
            moving: corners(&moving),
            tag,
            last: None,
        })
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
