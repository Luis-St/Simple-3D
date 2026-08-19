pub mod csg_bsp;
pub mod hull;
pub mod mesh;
pub mod planar;
pub mod polyhedra;
pub mod primitives;
pub mod repair;
pub mod revolve;
pub mod vec3;

mod tests;

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
fn union_all(children: &[Mesh]) -> Mesh {
    let mut parts: Vec<(Mesh, Bounds)> = Vec::new();
    for child in children {
        let Some(child_bounds) = child.bounds() else { continue };
        let mut acc = child.clone();
        let mut bounds = child_bounds;
        while let Some(i) = parts.iter().position(|(_, b)| boxes_overlap(*b, bounds)) {
            let (other, other_bounds) = parts.remove(i);
            acc = csg_bsp::union(&other, &acc);
            bounds = merged_bounds(bounds, other_bounds);
        }
        parts.push((acc, bounds));
    }
    let mut out = Mesh::new();
    for (mesh, _) in &parts {
        out.append(mesh);
    }
    out
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
    match op {
        BooleanOp::Union => union_all(children),
        BooleanOp::Difference => {
            let mut iter = children.iter();
            let Some(first) = iter.next() else { return Mesh::new() };
            iter.fold(first.clone(), |acc, m| if meshes_overlap(&acc, m) { csg_bsp::subtract(&acc, m) } else { acc })
        }
        BooleanOp::Intersection => {
            let mut iter = children.iter();
            let Some(first) = iter.next() else { return Mesh::new() };
            iter.fold(
                first.clone(),
                |acc, m| {
                    if meshes_overlap(&acc, m) {
                        csg_bsp::intersect(&acc, m)
                    } else {
                        Mesh::new()
                    }
                },
            )
        }
        BooleanOp::Hull => {
            let points: Vec<Vec3> = children.iter().flat_map(|m| m.positions.iter().copied()).collect();
            hull::convex_hull(&points)
        }
    }
}
