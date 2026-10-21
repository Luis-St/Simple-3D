//! Cutting a solid in two along a plane (issue 130): what lies on the plane's normal side, and the
//! rest. Each side is the solid intersected with, or less, a box standing on the plane and reaching
//! past the solid, so a side may be several bodies where the plane crosses the solid more than once.

use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// A plane through `point`, facing along the unit `normal`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CutPlane {
    pub point: Vec3,
    pub normal: Vec3,
}

impl CutPlane {
    /// How far `p` lies on the normal's side.
    pub fn depth(&self, p: Vec3) -> f64 {
        (p - self.point).dot(self.normal)
    }

    /// Two unit directions along the plane, square to each other and to the normal, with `u` level
    /// where it can be: along the ground for an upright plane, along X for a level one.
    pub fn axes(&self) -> (Vec3, Vec3) {
        let n = self.normal;
        let up = if n.z.abs() > 0.9 { Vec3::new(0.0, 1.0, 0.0) } else { Vec3::new(0.0, 0.0, 1.0) };
        let u = up.cross(n).normalized();
        (u, n.cross(u))
    }
}

/// How far from a solid's surface a corner counts as on the plane, relative to its size.
const ON_PLANE: f64 = 1e-9;

/// Where the plane meets the solid's edges: the outline of its cross-section. Empty when the plane
/// misses the solid or only touches it, since no cut would leave two sides.
pub fn cross_section_points(mesh: &Mesh, plane: &CutPlane) -> Vec<Vec3> {
    let Some((lo, hi)) = mesh.bounds() else { return Vec::new() };
    let near = ON_PLANE * (hi - lo).length().max(1.0);
    let depths: Vec<f64> = mesh.positions.iter().map(|&p| plane.depth(p)).collect();
    if !depths.iter().any(|&d| d > near) || !depths.iter().any(|&d| d < -near) {
        return Vec::new();
    }
    let mut points = Vec::new();
    for triangle in &mesh.indices {
        for k in 0..3 {
            let (a, b) = (triangle[k] as usize, triangle[(k + 1) % 3] as usize);
            let (da, db) = (depths[a], depths[b]);
            if (da > 0.0) != (db > 0.0) {
                let t = da / (da - db);
                points.push(mesh.positions[a] + (mesh.positions[b] - mesh.positions[a]) * t);
            }
        }
    }
    points
}

/// The solid cut in two: the side the normal faces, then the other. `None` when the plane leaves
/// the solid whole, or when `give_up` stops it.
pub fn cut_in_two(mesh: &Mesh, plane: &CutPlane, give_up: crate::Abandon<'_>) -> Option<[Mesh; 2]> {
    let (lo, hi) = mesh.bounds()?;
    let centre = (lo + hi) * 0.5;
    // Reaching past the solid every way along the plane, and past it along the normal.
    let reach = (hi - lo).length() + (centre - plane.point).length() + 1.0;
    let (u, v) = plane.axes();
    let n = plane.normal;
    // A right-handed frame (u x v = n), so the box keeps its outward winding.
    let block = crate::primitives::box_mesh(2.0 * reach, 2.0 * reach, reach);
    let mut side = block.clone();
    for p in &mut side.positions {
        *p = plane.point + u * p.x + v * p.y + n * (p.z + reach / 2.0);
    }
    let above = crate::csg_bsp::intersect_until(mesh, &side, give_up);
    if give_up() {
        return None;
    }
    let below = crate::csg_bsp::subtract_until(mesh, &side, give_up);
    if give_up() {
        return None;
    }
    // A grazing plane leaves a sliver with no volume, or nothing, on one side: no cut.
    let least = ON_PLANE * (hi - lo).length().powi(3).max(1e-12);
    (above.signed_volume() > least && below.signed_volume() > least).then_some([above, below])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(point: Vec3, normal: Vec3) -> CutPlane {
        CutPlane { point, normal: normal.normalized() }
    }

    #[test]
    fn a_box_cut_off_centre_splits_its_volume_by_where_the_plane_runs() {
        let solid = crate::primitives::box_mesh(10.0, 10.0, 10.0);
        let [above, below] =
            cut_in_two(&solid, &plane(Vec3::new(2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)), &crate::never)
                .expect("the plane runs through the box");
        assert!((above.signed_volume() - 300.0).abs() < 1e-6, "{}", above.signed_volume());
        assert!((below.signed_volume() - 700.0).abs() < 1e-6, "{}", below.signed_volume());
        let (lo, _) = above.bounds().unwrap();
        assert!((lo.x - 2.0).abs() < 1e-9, "the normal's side starts at the plane");
    }

    #[test]
    fn a_slanted_plane_cuts_too() {
        let solid = crate::primitives::box_mesh(10.0, 10.0, 10.0);
        let [above, below] =
            cut_in_two(&solid, &plane(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0)), &crate::never).expect("it cuts");
        assert!((above.signed_volume() - 500.0).abs() < 1e-6);
        assert!((below.signed_volume() - 500.0).abs() < 1e-6);
    }

    #[test]
    fn a_plane_missing_or_only_touching_the_solid_leaves_it_whole() {
        let solid = crate::primitives::box_mesh(10.0, 10.0, 10.0);
        let x = Vec3::new(1.0, 0.0, 0.0);
        for at in [20.0, 5.0, -5.0] {
            let plane = plane(Vec3::new(at, 0.0, 0.0), x);
            assert!(cross_section_points(&solid, &plane).is_empty(), "at {at}");
            assert!(cut_in_two(&solid, &plane, &crate::never).is_none(), "at {at}");
        }
    }

    #[test]
    fn the_cross_section_lies_on_the_plane_and_spans_the_solid() {
        let solid = crate::primitives::box_mesh(10.0, 6.0, 4.0);
        let cut = plane(Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let points = cross_section_points(&solid, &cut);
        assert!(!points.is_empty());
        assert!(points.iter().all(|p| cut.depth(*p).abs() < 1e-9));
        let widest = points.iter().fold(0.0_f64, |most, p| most.max(p.y.abs()));
        assert!((widest - 3.0).abs() < 1e-9, "{widest}");
    }

    #[test]
    fn the_plane_axes_are_square_to_each_other_and_right_handed() {
        for normal in [Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.3, -0.5, 0.8)] {
            let cut = plane(Vec3::ZERO, normal);
            let (u, v) = cut.axes();
            assert!(u.dot(v).abs() < 1e-12 && u.dot(cut.normal).abs() < 1e-12);
            assert!((u.cross(v) - cut.normal).length() < 1e-12);
        }
    }
}
