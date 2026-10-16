//! The model's edges a dragged cutter quiets: those lying on its surface, which are the cut it makes.
//! Its red shell shows the cut instead (see `App::moving_cutters`).

use super::*;
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;

/// The dragged cutters' triangles in world space, with a box around them to skip far edges cheaply.
pub(crate) struct Quiet {
    triangles: Vec<[Vec3; 3]>,
    lo: Vec3,
    hi: Vec3,
    /// How near an edge must lie to count as on the surface, relative to the cutters' size.
    near: f64,
}

impl Quiet {
    pub(crate) fn new(cutters: &[(&Renderable, Xform)]) -> Quiet {
        let mut triangles = Vec::new();
        for (renderable, moved) in cutters {
            let positions = &renderable.mesh.positions;
            for tri in &renderable.mesh.indices {
                triangles.push(tri.map(|corner| moved.point(positions[corner as usize])));
            }
        }
        let (lo, hi) = triangles.iter().flatten().fold(
            (Vec3::new(f64::MAX, f64::MAX, f64::MAX), Vec3::new(f64::MIN, f64::MIN, f64::MIN)),
            |(lo, hi), &p| (lo.min(p), hi.max(p)),
        );
        let near = 1e-4 * (hi - lo).length().max(1.0);
        Quiet { triangles, lo, hi, near }
    }

    /// Whether the edge from `a` to `b` lies on a cutter: both ends and its middle.
    pub(crate) fn holds(&self, a: Vec3, b: Vec3) -> bool {
        let within = |p: Vec3| {
            (0..3).all(|axis| {
                p.get(axis) >= self.lo.get(axis) - self.near && p.get(axis) <= self.hi.get(axis) + self.near
            })
        };
        if self.triangles.is_empty() || !within(a) || !within(b) {
            return false;
        }
        let on = |p: Vec3| self.triangles.iter().any(|tri| (closest_on_triangle(p, tri) - p).length() <= self.near);
        on(a) && on(b) && on((a + b) * 0.5)
    }
}

/// The point of triangle `tri` closest to `p` (Ericson, Real-Time Collision Detection, 5.1.5).
fn closest_on_triangle(p: Vec3, [a, b, c]: &[Vec3; 3]) -> Vec3 {
    let (a, b, c) = (*a, *b, *c);
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edge_on_the_cutters_face_is_quiet_and_one_beside_it_is_not() {
        let mesh = simple3d_geom::primitives::box_mesh(10.0, 10.0, 10.0);
        let cutter = Renderable::surface(mesh);
        let quiet = Quiet::new(&[(&cutter, Xform::IDENTITY)]);
        let (lo, hi) = cutter.mesh.bounds().unwrap();
        // Along the top face, then the same edge lifted off it.
        let (a, b) = (Vec3::new(lo.x, lo.y + 2.0, hi.z), Vec3::new(hi.x, lo.y + 2.0, hi.z));
        assert!(quiet.holds(a, b));
        let up = Vec3::new(0.0, 0.0, 1.0);
        assert!(!quiet.holds(a + up, b + up));
        assert!(!Quiet::new(&[]).holds(a, b), "nothing is quiet without a cutter");
    }
}
