pub mod csg_bsp;
pub mod hull;
pub mod mesh;
pub mod polyhedra;
pub mod primitives;
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

/// Combine already-evaluated child meshes according to a group's boolean
/// operation. `Difference` treats the first mesh as the base and subtracts
/// every subsequent one from it, matching the spec's child-order semantics.
pub fn evaluate_boolean(op: BooleanOp, children: &[Mesh]) -> Mesh {
    match op {
        BooleanOp::Union => {
            let mut iter = children.iter();
            let Some(first) = iter.next() else { return Mesh::new() };
            iter.fold(first.clone(), |acc, m| csg_bsp::union(&acc, m))
        }
        BooleanOp::Difference => {
            let mut iter = children.iter();
            let Some(first) = iter.next() else { return Mesh::new() };
            iter.fold(first.clone(), |acc, m| csg_bsp::subtract(&acc, m))
        }
        BooleanOp::Intersection => {
            let mut iter = children.iter();
            let Some(first) = iter.next() else { return Mesh::new() };
            iter.fold(first.clone(), |acc, m| csg_bsp::intersect(&acc, m))
        }
        BooleanOp::Hull => {
            let points: Vec<Vec3> = children.iter().flat_map(|m| m.positions.iter().copied()).collect();
            hull::convex_hull(&points)
        }
    }
}
