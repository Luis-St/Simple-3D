//! Which way the turn goes, and the solid it makes.

use super::*;

/// The middle of the profile's outer edge's corners, in its plane.
pub(crate) fn flat_middle(profile: &Target) -> [f64; 2] {
    let outer = &profile.outline().outer;
    let sum = outer.iter().fold([0.0; 2], |s, p| [s[0] + p[0], s[1] + p[1]]);
    let n = outer.len().max(1) as f64;
    [sum[0] / n, sum[1] / n]
}

/// Whether turning by `angle` sweeps the profile out of its front rather than into its back.
pub(crate) fn outward(axis: &AxisPick, angle: f64) -> bool {
    let (a, b) = axis.flat;
    let d = [b[0] - a[0], b[1] - a[1]];
    // Which side of the axis the profile lies: the middle, relative to the axis, in the plane.
    (angle > 0.0) == (side(axis, d, a) > 0.0)
}

/// Which side of the axis line the profile is on in its plane, through the profile's middle stored
/// with the pick: positive on the axis's left.
fn side(axis: &AxisPick, d: [f64; 2], a: [f64; 2]) -> f64 {
    let m = axis.middle;
    d[0] * (m[1] - a[1]) - d[1] * (m[0] - a[0])
}

/// The solid `profile` makes turned `angle` degrees about `axis`, in the world; or why none.
pub(crate) fn solid(profile: &Target, axis: &AxisPick, angle: f64) -> Result<simple3d_geom::Mesh, String> {
    let (a, b) = axis.flat;
    let line = if angle < 0.0 { (b, a) } else { (a, b) };
    let outline = profile.outline().clone();
    let mut mesh = simple3d_sketch::feature::revolve(&[outline], line, angle.abs(), crate::sketch_nodes::SEGMENTS)?;
    let frame = profile.frame();
    for p in mesh.positions.iter_mut() {
        *p = frame.point(*p);
    }
    Ok(mesh)
}

impl Revolve {
    /// Remake the solid shown for the picks and turn; nothing to remake while they have not changed.
    pub(crate) fn refresh(&mut self) {
        let (Some(profile), Some(axis)) = (&self.profile, &self.axis) else {
            self.preview = None;
            return;
        };
        let angle = self.signed_angle();
        if self.preview.as_ref().is_some_and(|(at, _)| *at == angle) {
            return;
        }
        let made =
            solid(profile, axis, angle).map(|mesh| Arc::new(crate::render::Renderable::surface_with_edges(&mesh)));
        self.preview = Some((angle, made));
        self.generation += 1;
    }

    /// Whether the solid shown cuts into the object the profile lies on rather than adding to it.
    pub(crate) fn cuts(&self) -> bool {
        let (Some(profile), Some(axis)) = (&self.profile, &self.axis) else { return false };
        !outward(axis, self.signed_angle()) && profile.owner().is_some()
    }
}
