//! A straight bevel gear (issue 123), for shafts meeting at a right angle. Its teeth are those of the
//! spur gear its back cone unrolls to (Tredgold's approximation), shrinking towards the cone's apex.

use super::body::turned;
use super::{tooth, ToothProfile};
use crate::mesh::Mesh;
use crate::revolve::grid_mesh;
use crate::vec3::Vec3;

/// Everything a bevel gear is made of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BevelSpec {
    pub teeth: u32,
    /// The teeth of the gear it meshes with, which sets the cone: equal counts give a 45 degree cone.
    pub mate_teeth: u32,
    /// At the back, the wide end of the teeth.
    pub module: f64,
    pub profile: ToothProfile,
    /// How far the teeth reach from the back towards the apex.
    pub face_width: f64,
    pub bore: f64,
}

impl BevelSpec {
    /// The pitch cone's half angle, in radians.
    fn cone(&self) -> f64 {
        (self.teeth.max(3) as f64).atan2(self.mate_teeth.max(3) as f64)
    }

    /// The pitch radius at the back.
    fn pitch_radius(&self) -> f64 {
        self.module * self.teeth.max(3) as f64 / 2.0
    }

    /// The radius of the spur gear the back cone unrolls to, and its tooth count.
    fn virtual_gear(&self) -> (f64, f64) {
        let cos = self.cone().cos();
        (self.pitch_radius() / cos, self.teeth.max(3) as f64 / cos)
    }

    /// The unrolled gear's tip and root radii.
    fn virtual_tip_root(&self) -> (f64, f64) {
        let (pitch, teeth) = self.virtual_gear();
        let tip = pitch + self.profile.addendum(pitch, self.module, teeth);
        (tip, (pitch - 1.25 * self.module).clamp(tip * 0.05, tip * 0.999))
    }

    /// Half the gear's width across its tips, at the back.
    pub fn tip_radius(&self) -> f64 {
        self.virtual_tip_root().0 * self.cone().cos()
    }

    /// How far the teeth reach towards the apex as built: at most half the way.
    fn face_built(&self) -> f64 {
        let distance = self.pitch_radius() / self.cone().sin();
        self.face_width.clamp(distance * 0.01, distance * 0.5)
    }
}

/// The bevel gear `spec` describes, its back towards -Z and its teeth narrowing upwards, centred on its
/// bounds with a tooth along +X.
pub fn bevel_gear_mesh(spec: &BevelSpec) -> Mesh {
    let cone = spec.cone();
    let (cos, sin) = (cone.cos(), cone.sin());
    let (pitch, teeth) = spec.virtual_gear();
    let (tip, root) = spec.virtual_tip_root();
    let pressure = 20f64.to_radians();
    // The unrolled gear's angles, stretched to go round the real one.
    let flank: Vec<(f64, f64)> = tooth::flank(spec.profile, teeth, pitch, pressure, root, tip)
        .into_iter()
        .map(|(r, off)| (r, off / cos))
        .collect();
    let outline = tooth::outline(&flank, spec.teeth.max(3), root, tip);
    // A point a distance r out on the back cone lies that far out along it from the back cone's apex.
    let apex = Vec3::new(0.0, 0.0, spec.pitch_radius() / cone.tan());
    let back: Vec<Vec3> = outline
        .iter()
        .map(|&(x, y)| {
            let r = x.hypot(y);
            let (u, v) = (x / r * cos, y / r * cos);
            Vec3::new(u * r, v * r, (r - pitch) * sin)
        })
        .collect();
    let scale = 1.0 - spec.face_built() * sin / spec.pitch_radius();
    let front: Vec<Vec3> = back.iter().map(|&p| apex + (p - apex) * scale).collect();
    // The bore runs from the back's roots to the front's, narrowed to leave a rim under the front's.
    let low = (root - pitch) * sin;
    let high = apex.z + (low - apex.z) * scale;
    let bore = (spec.bore / 2.0).clamp(0.0, root * cos * scale * 0.9);
    let directions: Vec<(f64, f64)> = outline
        .iter()
        .map(|&(x, y)| {
            let a = y.atan2(x);
            (bore * a.cos(), bore * a.sin())
        })
        .collect();
    let rings = vec![back, front, turned(&directions, 0.0, high), turned(&directions, 0.0, low)];
    let top = apex.z + ((tip - pitch) * sin - apex.z) * scale;
    let middle = (low + top.max(high)) / 2.0;
    grid_mesh(&rings, true).translated(Vec3::new(0.0, 0.0, -middle))
}
