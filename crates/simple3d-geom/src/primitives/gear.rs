//! A spur gear (issue 123) of involute, cycloidal or circular-arc teeth, straight, helical or herringbone,
//! with a hub and a keyway; the other kinds of gear build on its teeth.

mod bevel;
mod body;
mod internal;
mod rack;
mod tooth;
mod worm;
pub use bevel::{bevel_gear_mesh, BevelSpec};
pub use body::Body;
pub use internal::internal_gear_mesh;
pub use rack::{rack_mesh, RackSpec};
pub use tooth::ToothProfile;
pub use worm::worm_mesh;

use crate::mesh::Mesh;

/// Everything a gear is made of; [`GearSpec::new`] gives a standard straight gear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GearSpec {
    pub teeth: u32,
    /// Pitch diameter over teeth.
    pub module: f64,
    /// The involute's pressure angle in degrees.
    pub pressure_angle: f64,
    pub profile: ToothProfile,
    pub thickness: f64,
    pub bore: f64,
    /// Helix angle in degrees, 0 for straight teeth.
    pub helix: f64,
    /// Whether the helix turns back halfway up, so the side forces cancel.
    pub herringbone: bool,
    /// A collar on top: its diameter and height; none while either is 0 or it is no wider than the bore.
    pub hub: (f64, f64),
    /// A keyway in the bore: its width and how far it reaches past the bore; none while either is 0.
    pub key: (f64, f64),
}

impl GearSpec {
    pub fn new(teeth: u32, module: f64, pressure_angle: f64, thickness: f64, bore: f64) -> GearSpec {
        GearSpec {
            teeth,
            module,
            pressure_angle,
            profile: ToothProfile::Involute,
            thickness,
            bore,
            helix: 0.0,
            herringbone: false,
            hub: (0.0, 0.0),
            key: (0.0, 0.0),
        }
    }

    pub fn tip_radius(&self) -> f64 {
        let pitch = self.pitch_radius();
        (pitch + self.profile.addendum(pitch, self.module, self.teeth.max(3) as f64)).max(1e-6)
    }

    pub fn pitch_radius(&self) -> f64 {
        self.module * self.teeth.max(3) as f64 / 2.0
    }

    /// The root circle, a quarter module deeper than the teeth reach, held inside the tip circle and off
    /// the middle.
    pub fn root_radius(&self) -> f64 {
        let tip = self.tip_radius();
        (self.pitch_radius() - self.module * 1.25).clamp(tip * 0.05, tip * 0.999)
    }

    /// The wheel round the teeth, the helix turning them by its tangent over the pitch radius per height.
    pub fn body(&self) -> Body {
        Body {
            thickness: self.thickness,
            bore: self.bore,
            hub: self.hub,
            key: self.key,
            twist: self.helix.to_radians().tan() / self.pitch_radius().max(1e-9),
            herringbone: self.herringbone,
        }
    }

    /// The hub's radius and height as built: none (0, 0) when it would not stand proud of the bore.
    pub fn hub_built(&self) -> (f64, f64) {
        self.body().hub_built(self.root_radius())
    }

    /// The flank of one tooth reaching from `root` to `tip`, as radii and angles off the tooth's middle.
    fn flank(&self, root: f64, tip: f64) -> Vec<(f64, f64)> {
        let pressure = self.pressure_angle.clamp(0.1, 89.0).to_radians();
        tooth::flank(self.profile, self.teeth.max(3) as f64, self.pitch_radius(), pressure, root, tip)
    }
}

/// A spur gear of `teeth` teeth of `module` (pitch diameter over teeth) with the given pressure
/// angle, `thickness` tall, with a round bore; a tooth points along +X. A bore of 0 leaves it
/// solid; one reaching the teeth is narrowed to leave a rim.
pub fn spur_gear_mesh(teeth: u32, module: f64, pressure_angle: f64, thickness: f64, bore: f64) -> Mesh {
    gear_mesh(&GearSpec::new(teeth, module, pressure_angle, thickness, bore))
}

/// The gear `spec` describes, centred on its whole height with a tooth along +X at the bottom face.
pub fn gear_mesh(spec: &GearSpec) -> Mesh {
    let (root, tip) = (spec.root_radius(), spec.tip_radius());
    let outline = tooth::outline(&spec.flank(root, tip), spec.teeth.max(3), root, tip);
    spec.body().mesh(outline, root)
}
