//! The section's kept half-space as a boolean shape.

use super::*;

impl Gpu {
    /// The section's kept half-space as a shape, rebuilt only when it changes.
    pub(in crate::gpu) fn refresh_half_space(&mut self, request: &Request<'_>) {
        use std::hash::{Hash, Hasher};
        let Some(csg) = &request.live.csg else {
            self.csg_half = None;
            return;
        };
        if request.section.is_empty() {
            self.csg_half = None;
            return;
        }
        // Sized to fit every shape and rebuilt only when outgrown, so a drag does not rebuild it every frame.
        let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = -lo;
        for (leaf, moved) in &csg.leaves {
            if let Some(resident) = self.resident.get(&leaf.id) {
                let (a, b) = resident.world_box(&Placing::moved(*moved));
                lo = lo.min(a);
                hi = hi.max(b);
            }
        }
        if lo.x > hi.x {
            self.csg_half = None;
            return;
        }
        let reach = ((hi - lo).length().max(1.0) * 2.0).log2().ceil().exp2();
        let snap = |value: f64| (value / (reach / 4.0)).round() * (reach / 4.0);
        let middle = (lo + hi) * 0.5;
        let centre = Vec3::new(snap(middle.x), snap(middle.y), snap(middle.z));
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for value in [reach, centre.x, centre.y, centre.z] {
            value.to_bits().hash(&mut hasher);
        }
        for plane in &request.section {
            for value in [plane.normal.x, plane.normal.y, plane.normal.z, plane.offset] {
                value.to_bits().hash(&mut hasher);
            }
            if let Some(window) = plane.window {
                for at in [window.centre, window.u, window.v] {
                    for value in [at.x, at.y, at.z] {
                        value.to_bits().hash(&mut hasher);
                    }
                }
                for value in window.half {
                    value.to_bits().hash(&mut hasher);
                }
            }
        }
        request.palette.cut.hash(&mut hasher);
        let key = hasher.finish();
        if self.csg_half.as_ref().is_some_and(|(held, _)| *held == key) {
            return;
        }
        let cut = request.palette.cut;
        // One shape per section, each with the operation that cuts with it.
        let shapes = request
            .section
            .iter()
            .map(|plane| {
                let op = match plane.window {
                    Some(_) => crate::app::CSG_DIFFERENCE,
                    None => crate::app::CSG_INTERSECTION,
                };
                (Renderable::surface(half_space(plane, centre, reach, [cut[0], cut[1], cut[2]])), op)
            })
            .collect();
        self.csg_half = Some((key, shapes));
    }
}

/// The kept side of `plane` as a closed box of `reach` round `centre`, one face on the plane.
/// A windowed cut instead yields the box it removes, to be subtracted.
fn half_space(plane: &Plane, centre: Vec3, reach: f64, colour: [u8; 3]) -> Mesh {
    let n = plane.normal;
    let (foot, u, v, half, depth) = match plane.window {
        Some(window) => (window.centre, window.u, window.v, window.half, 2.0 * reach),
        None => {
            let helper = if n.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
            let u = n.cross(helper).normalized();
            // Kept is where `depth` is negative: behind the plane.
            (centre - n * plane.depth(centre), u, n.cross(u), [reach, reach], -2.0 * reach)
        }
    };
    let corner = |i: usize| {
        let a = if i & 1 == 0 { -half[0] } else { half[0] };
        let b = if i & 2 == 0 { -half[1] } else { half[1] };
        let c = if i & 4 == 0 { 0.0 } else { depth };
        foot + u * a + v * b + n * c
    };
    let mut mesh = Mesh::new();
    // Corners by bit: x = 1, y = 2, back = 4. Winding does not matter to the counts or shading.
    for [a, b, c, d] in [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]] {
        mesh.push_triangle(corner(a), corner(b), corner(c));
        mesh.push_triangle(corner(a), corner(c), corner(d));
    }
    mesh.set_tag(simple3d_geom::colour_tag(colour));
    mesh
}
