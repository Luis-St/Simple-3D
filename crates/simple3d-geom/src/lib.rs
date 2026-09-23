pub mod csg_bsp;
pub mod hull;
pub mod mesh;
pub mod planar;
pub mod polyhedra;
pub mod primitives;
pub mod reassemble;
pub mod repair;
pub mod revolve;
pub mod section;
pub mod simplify;
pub mod tiling;
pub mod vec3;

mod tests;

mod bench;

pub use mesh::{colour_tag, tag_colour, Mesh};
pub use vec3::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanOp {
    Union,
    Difference,
    Intersection,
    Hull,
}

type Bounds = (Vec3, Vec3);

/// Whether two bounding boxes overlap, with a tolerance generous enough that
/// shapes merely *touching* still go through the real kernel -- coincident faces
/// are exactly the case the BSP has to handle properly.
fn boxes_overlap(a: Bounds, b: Bounds) -> bool {
    let ((alo, ahi), (blo, bhi)) = (a, b);
    const SLACK: f64 = 1e-6;
    alo.x - SLACK <= bhi.x
        && blo.x - SLACK <= ahi.x
        && alo.y - SLACK <= bhi.y
        && blo.y - SLACK <= ahi.y
        && alo.z - SLACK <= bhi.z
        && blo.z - SLACK <= ahi.z
}

fn merged_bounds(a: Bounds, b: Bounds) -> Bounds {
    (a.0.min(b.0), a.1.max(b.1))
}

fn meshes_overlap(a: &Mesh, b: &Mesh) -> bool {
    match (a.bounds(), b.bounds()) {
        (Some(a), Some(b)) => boxes_overlap(a, b),
        _ => false,
    }
}

/// Union a list of operands while keeping the accumulated result as a set of
/// *mutually disjoint parts* rather than one growing mesh.
///
/// The distinction is the whole performance story for a scene of many separate
/// assemblies. Folding `union` over the operands makes the accumulator's
/// bounding box the box of everything unioned so far, so once that box spans the
/// build plate every later operand looks like it might overlap and goes through
/// the BSP kernel against tens of thousands of triangles -- even though each
/// assembly is physically nowhere near any other. Keeping each disjoint island's
/// own box means an operand is only ever run through the kernel against the
/// islands it can actually touch, and the concatenation the kernel would have
/// produced for the rest is done directly.
///
/// Merging two islands grows the merged box, which can bring it into contact
/// with an island that was previously clear, so the search restarts until no
/// part overlaps.
///
/// Each operand that ends up an island of its own is copied into the result
/// untouched, and those are reported: see [`Traced`].
fn union_all(children: &[Mesh], give_up: Abandon<'_>) -> Traced {
    // Each part, with the operand it is when it is one operand untouched.
    let mut parts: Vec<(Mesh, Bounds, Option<usize>)> = Vec::new();
    for (index, child) in children.iter().enumerate() {
        let Some(child_bounds) = child.bounds() else { continue };
        let mut acc = child.clone();
        let mut bounds = child_bounds;
        let mut alone = Some(index);
        while let Some(i) = parts.iter().position(|(_, b, _)| boxes_overlap(*b, bounds)) {
            if give_up() {
                return (Mesh::new(), Vec::new());
            }
            let (other, other_bounds, _) = parts.remove(i);
            acc = csg_bsp::union_until(&other, &acc, give_up);
            bounds = merged_bounds(bounds, other_bounds);
            alone = None;
        }
        parts.push((acc, bounds, alone));
    }
    let mut out = Mesh::new();
    let mut untouched = Vec::new();
    for (mesh, _, alone) in &parts {
        if let Some(index) = alone {
            untouched.push((*index, out.positions.len() as u32));
        }
        out.append(mesh);
    }
    (out, untouched)
}

/// A boolean's result, with the operands that came through it untouched: for
/// each, its index among the operands and where its first vertex landed in the
/// result. Its vertices and triangles follow on from there in their own order,
/// so a caller can tell which part of the result is which operand.
///
/// What lets the viewport move a body without waiting for the boolean to be
/// run again: a body that went through unchanged is a known stretch of the
/// result, and the rest of the result does not depend on where it is.
pub type Traced = (Mesh, Vec<(usize, u32)>);

/// Asked at every point the kernel can safely abandon what it is doing: is this
/// answer still wanted?
///
/// A boolean is where the time goes, and it used to be the one thing in an
/// evaluation that could not be interrupted. `Evaluator::scatter` checked its
/// `Cancel` while it was *placing* the copies and then handed the ground and all
/// of them to a union that took no cancellation at all, so the flag the
/// interface sets on the next edit could never land: the job ran to the end,
/// every newer edit queued behind it, and with a big enough scatter the
/// application could not be got out of it -- the footer still said
/// "Evaluating..." with every node deleted.
///
/// A callback rather than the core crate's `Cancel`, because this crate sits
/// below it and knows nothing of scenes or workers.
pub type Abandon<'a> = &'a dyn Fn() -> bool;

/// Never gives up. For the callers -- tests, benches, the exporter's own fixture
/// -- with nothing to cancel against.
pub fn never() -> bool {
    false
}

/// Combine already-evaluated child meshes according to a group's boolean
/// operation. `Difference` treats the first mesh as the base and subtracts
/// every subsequent one from it, matching the spec's child-order semantics.
///
/// Operands whose bounding boxes do not overlap are handled without invoking the
/// BSP kernel at all: their union is a concatenation, subtracting one from the
/// other changes nothing, and their intersection is empty. That is not a
/// micro-optimisation -- a scene of fifty separate assemblies is fifty disjoint
/// unions, and running each through a BSP tree of everything unioned so far made
/// the spec's 200-primitive target take eleven seconds instead of a fraction of
/// one. The result is identical either way; a disjoint union through the kernel
/// is a pure pass-through by construction.
pub fn evaluate_boolean(op: BooleanOp, children: &[Mesh]) -> Mesh {
    evaluate_boolean_until(op, children, &never)
}

/// The same, abandoned part-way when `give_up` says the answer is no longer
/// wanted. What comes back then is not a result and is never used: the evaluator
/// marks the whole run cancelled and the worker drops it.
pub fn evaluate_boolean_until(op: BooleanOp, children: &[Mesh], give_up: Abandon<'_>) -> Mesh {
    evaluate_boolean_traced(op, children, give_up).0
}

/// The same, saying which operands came through untouched -- see [`Traced`].
/// Only a union and a difference ever pass one through: a union each operand
/// that meets no other, a difference its first when nothing is taken out of
/// it.
pub fn evaluate_boolean_traced(op: BooleanOp, children: &[Mesh], give_up: Abandon<'_>) -> Traced {
    match op {
        BooleanOp::Union => union_all(children, give_up),
        BooleanOp::Difference => {
            let mut iter = children.iter();
            let Some(first) = iter.next() else { return (Mesh::new(), Vec::new()) };
            let mut untouched = true;
            let result = iter.fold(first.clone(), |acc, m| {
                if give_up() {
                    untouched = false;
                    Mesh::new()
                } else if meshes_overlap(&acc, m) {
                    untouched = false;
                    csg_bsp::subtract_until(&acc, m, give_up)
                } else {
                    acc
                }
            });
            (result, if untouched { vec![(0, 0)] } else { Vec::new() })
        }
        BooleanOp::Intersection => {
            let mut iter = children.iter();
            let Some(first) = iter.next() else { return (Mesh::new(), Vec::new()) };
            let result = iter.fold(first.clone(), |acc, m| {
                if give_up() || !meshes_overlap(&acc, m) {
                    Mesh::new()
                } else {
                    csg_bsp::intersect_until(&acc, m, give_up)
                }
            });
            (result, Vec::new())
        }
        BooleanOp::Hull => {
            let points: Vec<Vec3> = children.iter().flat_map(|m| m.positions.iter().copied()).collect();
            (hull::convex_hull(&points), Vec::new())
        }
    }
}
