//! What a drag takes hold of: a flat face of the model, or a closed region of a kept sketch.

use super::*;
use crate::app::App;
use crate::sketch_nodes::RegionHit;
use crate::view::View;
use simple3d_core::scene::NodeId;
use simple3d_geom::push_pull::Outline;
use simple3d_geom::Vec3;

/// A face or a sketch region, either of which push/pull sweeps along its normal.
#[derive(Clone)]
pub(crate) enum Target {
    Face(FaceHit),
    /// A sketch region, with the object it lies on, if it lies on one: pushed in, it cuts that.
    Region(RegionHit, Option<NodeId>),
}

/// A plane in the world: origin, the outline's X and Y, and the normal it is swept along.
pub(crate) struct Plane {
    pub(crate) origin: Vec3,
    pub(crate) u: Vec3,
    pub(crate) v: Vec3,
    pub(crate) normal: Vec3,
}

impl Target {
    pub(crate) fn plane(&self) -> Plane {
        match self {
            Target::Face(hit) => {
                let c = &hit.captured;
                Plane { origin: c.origin, u: c.u, v: c.v, normal: c.normal }
            }
            Target::Region(hit, _) => {
                let f = &hit.frame;
                Plane { origin: f.t, u: f.axis(0), v: f.axis(1), normal: f.axis(2) }
            }
        }
    }

    /// The plane as a frame: the profile's X, Y and normal at its origin.
    pub(crate) fn frame(&self) -> simple3d_core::xform::Xform {
        match self {
            Target::Region(hit, _) => hit.frame,
            Target::Face(_) => {
                let Plane { origin, u, v, normal: n } = self.plane();
                simple3d_core::xform::Xform { m: [[u.x, v.x, n.x], [u.y, v.y, n.y], [u.z, v.z, n.z]], t: origin }
            }
        }
    }

    /// The object the face or region is on, if any: what an inward sweep cuts.
    pub(crate) fn owner(&self) -> Option<NodeId> {
        match self {
            Target::Face(hit) => Some(hit.owner),
            Target::Region(_, owner) => *owner,
        }
    }

    pub(crate) fn outline(&self) -> &Outline {
        match self {
            Target::Face(hit) => &hit.captured.outline,
            Target::Region(hit, _) => &hit.outline,
        }
    }

    /// Whether a drag `distance` along the normal cuts rather than adds: inward into the object
    /// the face or region is on.
    pub(crate) fn cuts(&self, distance: f64) -> bool {
        distance < 0.0 && !matches!(self, Target::Region(_, None))
    }
}

impl App {
    /// What a press at `cursor` takes hold of: a sketch region before the face it lies on. `Err`
    /// with a reason when the pointer is over the model but on nothing that can be pushed.
    pub(crate) fn push_target_under(
        &self,
        view: &View,
        cursor: egui::Pos2,
    ) -> Option<(Result<Target, &'static str>, Vec3)> {
        if let Some(hit) = self.region_under(view, cursor) {
            let at = hit.at;
            let owner = self.object_under_region(&hit);
            return Some((Ok(Target::Region(hit, owner)), at));
        }
        self.face_under(view, cursor).map(|(hit, at)| (hit.map(Target::Face), at))
    }

    /// The object whose face the region lies on: the model met straight behind it, right at the
    /// sketch plane.
    pub(crate) fn object_under_region(&self, hit: &RegionHit) -> Option<NodeId> {
        let normal = hit.frame.axis(2).normalized();
        let reach = 1e-3;
        let from = hit.at + normal * reach;
        let (t, triangle) = crate::pick::ray_mesh_triangle(&self.evaluated.mesh, from, -normal)?;
        let owner = NodeId::from(self.evaluated.mesh.source(triangle));
        (t < 2.0 * reach && owner != 0 && self.scene.contains(owner)).then_some(owner)
    }
}
