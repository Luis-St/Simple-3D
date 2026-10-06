//! Push/pull (issue 73): the fifth tool on the rail. A flat face of the model is dragged along its
//! normal; outward adds an extrusion to the face's own object, inward cuts a reduction out of it. Both
//! are kept inside the object (see `simple3d_core::scene::FaceEdit` for which node holds them), so the
//! faces they leave flush merge with the object's own and can be pushed again whole.
//!
//! The face is found on the evaluated model, not on each object's own mesh, so what is pushed is
//! what is seen -- the overlap of two objects, a face left by a cut. Its object is read from the
//! source channel the boolean kernel carries.

mod hit;
pub(crate) use hit::FaceHit;
mod interact;
pub(crate) use interact::interact;
pub(crate) mod draw;
mod prism;
pub(crate) use draw::draw;
pub(crate) use prism::template;

/// A face found for a mesh (by address) and triangle, or why there is none to push.
type CachedFace = (usize, usize, Result<FaceHit, &'static str>);

/// The tool's state between frames.
#[derive(Default)]
pub struct PushPull {
    /// The face under the pointer, with the mesh and triangle it was found for, so a still pointer
    /// does not rebuild it every frame.
    pub(crate) hover: std::cell::RefCell<Option<CachedFace>>,
    /// The face being dragged.
    pub(crate) drag: Option<PushDrag>,
}

/// A face on its way out or in.
#[derive(Clone)]
pub(crate) struct PushDrag {
    pub(crate) hit: FaceHit,
    /// Where on the face the drag took hold.
    pub(crate) grab: simple3d_geom::Vec3,
    /// Where on the screen the drag began.
    pub(crate) press: egui::Pos2,
    /// The signed distance so far, snapped to the move step.
    pub(crate) distance: f64,
    /// The solid swept for `distance`, drawn by the renderer so walls in front hide it.
    pub(crate) prism: Option<(f64, std::sync::Arc<crate::render::Renderable>)>,
}
