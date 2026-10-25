//! Extruding: the outlines swept along Z between two heights, walls leaning by the draft.

use super::offset::offset;
use super::{caps, degenerate, loops};
use simple3d_geom::push_pull::Outline;
use simple3d_geom::{Mesh, Vec3};

/// How far an extrusion runs and how its walls lean.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extrude {
    /// The heights it runs between, `from < to`; the sketch plane is z = 0, so a symmetric one has
    /// `from = -to`.
    pub from: f64,
    pub to: f64,
    /// How far the walls lean in, in degrees, as they leave the sketch plane either way; out when
    /// negative.
    pub draft: f64,
}

/// The outlines extruded as `how` says, walls and caps wound outward. Fails with a reason when the
/// extrusion has no length, a cap cannot be laid, or the draft folds an outline.
pub fn extrude(outlines: &[Outline], how: Extrude) -> Result<Mesh, String> {
    if outlines.is_empty() {
        return Err("No region is picked to extrude".into());
    }
    if how.to - how.from <= 1e-6 || !how.from.is_finite() || !how.to.is_finite() {
        return Err("The extrusion has no length".into());
    }
    let tan = how.draft.to_radians().tan();
    // The walls bend where the sketch plane is passed, since the draft leans away from it both ways.
    let mut heights = vec![how.from, how.to];
    if tan != 0.0 && how.from < 0.0 && how.to > 0.0 {
        heights.insert(1, 0.0);
    }
    let levels: Vec<Vec<Outline>> = heights
        .iter()
        .map(|z| outlines.iter().map(|o| offset(o, z.abs() * tan)).collect::<Option<Vec<_>>>())
        .collect::<Option<Vec<_>>>()
        .ok_or("The draft is too steep for this profile: its walls would fold over")?;
    let (bottom, top) = (&levels[0], &levels[levels.len() - 1]);
    let low = caps(bottom, how.from).ok_or("The profile cannot be filled to close the solid")?;
    let high = caps(top, how.to).ok_or("The profile cannot be filled to close the solid")?;
    let mut mesh = Mesh::new();
    for [a, b, c] in high {
        mesh.push_triangle(a, b, c);
    }
    for [a, b, c] in low {
        mesh.push_triangle(a, c, b);
    }
    for pair in 0..heights.len() - 1 {
        let (z0, z1) = (heights[pair], heights[pair + 1]);
        let (lower, upper) = (loops(&levels[pair]), loops(&levels[pair + 1]));
        for (bottom, top) in lower.iter().zip(&upper) {
            let n = bottom.len();
            for i in 0..n {
                let j = (i + 1) % n;
                let at = |p: [f64; 2], z: f64| Vec3::new(p[0], p[1], z);
                let (b0, b1) = (at(bottom[i], z0), at(bottom[j], z0));
                let (t0, t1) = (at(top[i], z1), at(top[j], z1));
                if !degenerate(b0, b1, t1) {
                    mesh.push_triangle(b0, b1, t1);
                }
                if !degenerate(b0, t1, t0) {
                    mesh.push_triangle(b0, t1, t0);
                }
            }
        }
    }
    Ok(mesh)
}
