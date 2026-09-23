//! A boolean drawn on the card while one of its operands is dragged.

use super::*;
use crate::render::Renderable;
use simple3d_core::scene::{Body, GroupOp, NodeId};
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;
use std::sync::Arc;

/// A drag the GPU draws as the boolean it changes: the group whose result is
/// in the scene as it came, as an expression over the shapes that go into it,
/// with the dragged one where the drag has got to. See `gpu/csg.rs`.
pub(crate) struct LiveCsg {
    /// The group's stretch of the scene's vertices, left out while it is
    /// drawn this way.
    pub range: std::ops::Range<u32>,
    /// The shapes, each as the last evaluation made it, in world space, with
    /// the move that puts it where it is drawn: the drag's for the dragged
    /// one, a pattern copy's for a shape inside a pattern, both for the
    /// dragged one in a copy.
    pub leaves: Vec<(Arc<Renderable>, Option<Xform>)>,
    /// Which node is dragged, and how far it has moved.
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
/// kept back for the section plane. See `gpu::csg::MAX_SHAPES`.
pub(crate) const CSG_LEAVES: usize = 127;

/// How deep the resolve pass's stack goes, and how long an expression it
/// holds -- the section's intersection counted in.
const CSG_STACK: usize = 32;
const CSG_PROGRAM: usize = 256;

/// One shape of the expression, as [`App::csg_plan`] lays it out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PlannedLeaf {
    pub shape: Shape,
    /// The pattern copies it is in, as one move in world space; `None`
    /// outside any pattern.
    pub copy: Option<Xform>,
    /// Whether it moves with the drag.
    pub carried: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    /// A node's shape, as the last evaluation made it.
    Node(NodeId),
    /// A hull with the dragged body among what it is stretched over, made
    /// again for where the drag has got to (see [`App::csg_hull`]).
    Hull(NodeId),
}

/// A hull the dragged body is inside of, worked out once per drag down to
/// the few points that decide it: the hull of everything that stays where it
/// is, and of the dragged body itself. Each frame only takes the hull of those
/// two together, with the one moved.
pub(crate) struct HullCache {
    generation: u64,
    carried: NodeId,
    still: Vec<Vec3>,
    moving: Vec<Vec3>,
    tag: u32,
    last: Option<(Xform, Arc<Renderable>)>,
}

impl App {
    /// The boolean a drag changes, when the GPU can draw it without the
    /// scene being evaluated (see [`LiveCsg`]).
    ///
    /// For a body that is not a stretch of the scene of its own -- one inside
    /// a union it meets, a cutter, a difference's base -- and only while every
    /// group from it up to the nearest one that is such a stretch is one the
    /// card can draw: a union, a difference, an intersection or an assembly
    /// per pixel, a pattern as its copies, and a hull as a shape made again
    /// on each frame. A split of it still waits for the evaluation.
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

    /// The shapes [`App::live_csg`] would draw a drag of the selection
    /// from, made ahead of the drag while nothing is being dragged.
    ///
    /// Making them, and putting them on the card, used to be the first frame
    /// of the drag's work -- every shape of the boolean at once, and on a
    /// large model a frame that visibly hung. Made when the body is selected
    /// instead, and kept on the card for as long as it is, the drag starts
    /// with all of it already there. The same shapes, since they are kept per
    /// evaluation, so the drag finds them rather than making them again.
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

    /// The boolean a drag of `carried` changes: the group whose result is a
    /// stretch of the scene of its own, the shapes of the expression below it
    /// -- `carried` among them -- and the expression over them. `None` when
    /// `carried` is a stretch of its own (a plain live drag moves it), when a
    /// group on the way up cannot be drawn this way, or when there are more
    /// shapes than the masks hold.
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
        // A hull is made from the points of what is under it, which are the
        // points of its operands only while nothing between it and the dragged
        // body takes material away or repeats it; and one hull is as many as
        // one shape made again per frame should stand for.
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

    /// `node` as an expression: opened up into its children when it is the
    /// group or a group on the way down to the carried body, a leaf otherwise.
    /// `copy` is the pattern copy it is being laid out for.
    fn csg_expression(&self, node: NodeId, copy: Option<Xform>, plan: &mut Plan<'_>) -> Option<()> {
        if plan.leaves.len() > CSG_LEAVES {
            return None;
        }
        // `path` runs from the carried body up to the group, so a node on it
        // other than the body itself is a group it is inside of.
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
            // The copies side by side, which the evaluation unions: each the
            // pattern's children, moved the way the copy moves them. They are
            // laid out in the pattern's own frame -- where its children are
            // placed -- so a copy's move in world space is that frame, the
            // copy, and the frame undone again.
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

    /// A group's shown children with `op` between each two of them.
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
        // A group with nothing in it is nothing, which no expression here can
        // say; such a group waits for the evaluation.
        (!first).then_some(())
    }

    /// A shape as the last evaluation made it, kept for as long as that
    /// evaluation is on screen.
    pub(crate) fn csg_leaf(&self, id: NodeId) -> Option<Arc<Renderable>> {
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

    /// The hull `id` makes with `carried` moved by `moved`, in world space.
    ///
    /// No pixel can say whether it is inside a hull without the hull, so this
    /// one is made on the processor -- but not from the operands' meshes: a
    /// hull of points is the hull of their hulls, so the operands that stay
    /// put and the dragged body are each boiled down to their own hull once,
    /// when the drag first asks, and each frame takes the hull of those few
    /// points together.
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

    /// What [`App::csg_hull`] makes each frame's hull from: the points of
    /// `carried`, and of everything else under `id`, each boiled down to its
    /// hull.
    fn hull_cache(&self, id: NodeId, carried: NodeId) -> Option<HullCache> {
        let shown = |node: NodeId| {
            let node = self.scene.node(node);
            node.children.iter().copied().filter(|&child| self.scene.node(child).visible).collect::<Vec<_>>()
        };
        // Everything beside the way down from the hull to the carried body:
        // what `csg_plan` allows below a hull is unions, so those are the
        // hull's operands as much as the dragged body is.
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

/// How deep a postfix expression's stack gets while it is worked out.
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
