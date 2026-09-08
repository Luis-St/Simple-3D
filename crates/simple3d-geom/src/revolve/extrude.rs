//! Extruding an outline along an axis, and closing what that leaves open.

use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// Extrude a (possibly tapering) polygon along Z from -h/2 to +h/2, capped at
/// both ends. `bottom` and `top` must have the same vertex count and are both
/// assumed centred on (0,0) and convex (or at least star-shaped around the
/// origin) so a fan cap is valid. Either outline may degenerate to all-zero
/// points (an apex), which is handled automatically via degenerate triangles.
pub fn extrude_frustum_polygon(bottom: &[(f64, f64)], top: &[(f64, f64)], h: f64) -> Mesh {
    debug_assert_eq!(bottom.len(), top.len());
    let n = bottom.len();
    let b: Vec<Vec3> = bottom.iter().map(|&(x, y)| Vec3::new(x, y, -h / 2.0)).collect();
    let t: Vec<Vec3> = top.iter().map(|&(x, y)| Vec3::new(x, y, h / 2.0)).collect();
    let mut mesh = Mesh::new();
    for i in 0..n {
        let i2 = (i + 1) % n;
        mesh.push_triangle(b[i], b[i2], t[i2]);
        mesh.push_triangle(b[i], t[i2], t[i]);
    }
    // A tapered end whose outline has collapsed to a point or a line (an
    // apex, or a wedge's sharp ridge) has zero area and no real cap face to
    // close -- the side walls already meet there. Only fan a genuine cap.
    let shoelace_area2 = |outline: &[(f64, f64)]| -> f64 {
        let n = outline.len();
        (0..n)
            .map(|i| {
                let (x0, y0) = outline[i];
                let (x1, y1) = outline[(i + 1) % n];
                x0 * y1 - x1 * y0
            })
            .sum()
    };
    let cb = Vec3::new(0.0, 0.0, -h / 2.0);
    let ct = Vec3::new(0.0, 0.0, h / 2.0);
    if shoelace_area2(bottom).abs() > 1e-9 {
        if flat_cap(bottom) {
            for i in 1..n - 1 {
                mesh.push_triangle(b[0], b[i + 1], b[i]);
            }
        } else {
            for i in 0..n {
                let i2 = (i + 1) % n;
                mesh.push_triangle(cb, b[i2], b[i]);
            }
        }
    }
    if shoelace_area2(top).abs() > 1e-9 {
        if flat_cap(top) {
            for i in 1..n - 1 {
                mesh.push_triangle(t[0], t[i], t[i + 1]);
            }
        } else {
            for i in 0..n {
                let i2 = (i + 1) % n;
                mesh.push_triangle(ct, t[i], t[i2]);
            }
        }
    }
    mesh
}

/// Whether a cap outline should be fanned from one of its own corners rather
/// than from a vertex added in the middle of it.
///
/// A centre vertex is what keeps a *round* cap's triangles well shaped: fanning
/// a 448-segment circle from one point on its rim gives 446 slivers, and the
/// boolean kernel's classification is only as good as the normals it computes
/// off them. A square has no such problem, and there the centre vertex is pure
/// noise: a plain box came out 16 triangles and 10 vertices rather than 12 and
/// 8, and those two invented cap centres travel into every 3MF, STL, OBJ and
/// PLY the application writes -- of the shape a reader is most likely to open a
/// file to check.
///
/// So: a convex cap of four corners or fewer, which is every flat-sided
/// extrusion in the library and no curve approximation.
pub(crate) fn flat_cap(outline: &[(f64, f64)]) -> bool {
    if outline.len() < 3 || outline.len() > 4 {
        return false;
    }
    // Convex: every turn around the outline goes the same way. A dart would
    // put the fan's triangles outside the shape.
    let n = outline.len();
    let mut sign = 0.0;
    for i in 0..n {
        let (x0, y0) = outline[i];
        let (x1, y1) = outline[(i + 1) % n];
        let (x2, y2) = outline[(i + 2) % n];
        let cross = (x1 - x0) * (y2 - y1) - (y1 - y0) * (x2 - x1);
        if cross.abs() < 1e-12 {
            continue;
        }
        if sign != 0.0 && cross.signum() != sign {
            return false;
        }
        sign = cross.signum();
    }
    true
}

/// Extrude a stack of outlines, each at its own Z, capping the first and the
/// last. Every outline needs the same vertex count, and they have to be given
/// in increasing Z. This is [`extrude_frustum_polygon`] generalised to more
/// than two levels, which is what a horizontally chamfered solid needs: its
/// wall changes direction twice on the way up.
pub fn extrude_stack(levels: &[(Vec<(f64, f64)>, f64)]) -> Mesh {
    let mut mesh = Mesh::new();
    if levels.len() < 2 {
        return mesh;
    }
    let n = levels[0].0.len();
    let ring = |level: &(Vec<(f64, f64)>, f64)| -> Vec<Vec3> {
        level.0.iter().map(|&(x, y)| Vec3::new(x, y, level.1)).collect()
    };
    let rings: Vec<Vec<Vec3>> = levels.iter().map(ring).collect();
    for pair in rings.windows(2) {
        let (lower, upper) = (&pair[0], &pair[1]);
        for i in 0..n {
            let i2 = (i + 1) % n;
            mesh.push_triangle(lower[i], lower[i2], upper[i2]);
            mesh.push_triangle(lower[i], upper[i2], upper[i]);
        }
    }
    let area2 = |outline: &[(f64, f64)]| -> f64 {
        (0..outline.len())
            .map(|i| {
                let (x0, y0) = outline[i];
                let (x1, y1) = outline[(i + 1) % outline.len()];
                x0 * y1 - x1 * y0
            })
            .sum()
    };
    let first = &levels[0];
    if area2(&first.0).abs() > 1e-9 {
        let centre = Vec3::new(0.0, 0.0, first.1);
        for i in 0..n {
            mesh.push_triangle(centre, rings[0][(i + 1) % n], rings[0][i]);
        }
    }
    let last = levels.last().expect("checked non-empty");
    if area2(&last.0).abs() > 1e-9 {
        let centre = Vec3::new(0.0, 0.0, last.1);
        let top = rings.last().expect("checked non-empty");
        for i in 0..n {
            mesh.push_triangle(centre, top[i], top[(i + 1) % n]);
        }
    }
    mesh
}
