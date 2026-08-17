pub mod csg_bsp;
pub mod hull;
pub mod mesh;
pub mod polyhedra;
pub mod primitives;
pub mod repair;
pub mod revolve;
pub mod vec3;

mod tests;

pub use mesh::Mesh;
pub use vec3::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanOp {
    Union,
    Difference,
    Intersection,
    Hull,
}

/// Whether two meshes' bounding boxes overlap, with a tolerance generous enough
/// that shapes merely *touching* still go through the real kernel -- coincident
/// faces are exactly the case the BSP has to handle properly.
fn boxes_overlap(a: &Mesh, b: &Mesh) -> bool {
    let (Some((alo, ahi)), Some((blo, bhi))) = (a.bounds(), b.bounds()) else { return false };
    const SLACK: f64 = 1e-6;
    alo.x - SLACK <= bhi.x
        && blo.x - SLACK <= ahi.x
        && alo.y - SLACK <= bhi.y
        && blo.y - SLACK <= ahi.y
        && alo.z - SLACK <= bhi.z
        && blo.z - SLACK <= ahi.z
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
        BooleanOp::Union => {
            let mut iter = children.iter();
            let Some(first) = iter.next() else { return Mesh::new() };
            iter.fold(first.clone(), |acc, m| {
                if boxes_overlap(&acc, m) {
                    csg_bsp::union(&acc, m)
                } else {
                    let mut merged = acc;
                    merged.append(m);
                    merged
                }
            })
        }
        BooleanOp::Difference => {
            let mut iter = children.iter();
            let Some(first) = iter.next() else { return Mesh::new() };
            iter.fold(first.clone(), |acc, m| {
                if boxes_overlap(&acc, m) {
                    csg_bsp::subtract(&acc, m)
                } else {
                    acc
                }
            })
        }
        BooleanOp::Intersection => {
            let mut iter = children.iter();
            let Some(first) = iter.next() else { return Mesh::new() };
            iter.fold(first.clone(), |acc, m| {
                if boxes_overlap(&acc, m) {
                    csg_bsp::intersect(&acc, m)
                } else {
                    Mesh::new()
                }
            })
        }
        BooleanOp::Hull => {
            let points: Vec<Vec3> = children.iter().flat_map(|m| m.positions.iter().copied()).collect();
            hull::convex_hull(&points)
        }
    }
}
