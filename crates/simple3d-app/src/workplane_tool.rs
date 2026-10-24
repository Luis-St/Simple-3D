//! Putting a workplane down on a face, or changing one (issue 125).
//!
//! A face is picked in the viewport, and the workplane's grid shows on it at once, in place of the
//! ground's, while the window slides it along its own X and Y, lifts it along the face's normal,
//! turns it about the normal and sets its spacing. Nothing is kept until Place, which also makes it
//! the active workplane; the panel lists them ([`crate::panel_workplanes`]). A non-modal [in-place
//! popup](crate::popup).

mod open;
mod pointer;
pub(crate) use pointer::*;
mod overlay;
pub(crate) use overlay::*;
mod window;
pub(crate) use window::*;
#[cfg(test)]
mod tests;

use simple3d_core::scene::{Workplane, WorkplaneId};
use simple3d_core::xform::Xform;
use simple3d_geom::plane_cut::CutPlane;
use simple3d_geom::Vec3;

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "workplane-tool";

/// The window width: a name and three offsets with their units.
const WIDTH: f32 = 320.0;

/// What the tool is working on while its window is open.
pub struct WorkplaneTool {
    /// The workplane being changed, or `None` while a new one is put down.
    pub editing: Option<WorkplaneId>,
    /// The workplane as set up so far.
    pub plane: Workplane,
    /// Whether it has a face yet; a new one has none until a click picks it.
    pub picked: bool,
}

impl WorkplaneTool {
    /// A new workplane named `name` with grid lines `spacing` apart, waiting for its face.
    pub fn new(name: String, spacing: f64) -> WorkplaneTool {
        let plane = Workplane {
            id: 0,
            name,
            base: Vec3::ZERO,
            base_rotation: Vec3::ZERO,
            offset: [0.0; 3],
            spin: 0.0,
            spacing,
        };
        WorkplaneTool { editing: None, plane, picked: false }
    }

    /// The tool on an existing workplane, its face already known.
    pub fn editing(plane: Workplane) -> WorkplaneTool {
        WorkplaneTool { editing: Some(plane.id), plane, picked: true }
    }

    /// The workplane to draw while it is set up, once it has a face.
    pub fn preview(&self) -> Option<Workplane> {
        self.picked.then(|| self.plane.clone())
    }

    /// Put the workplane on `face`, keeping its slide, lift, turn and spacing. Its X runs level where
    /// the face allows ([`CutPlane::axes`]), so a top face keeps the world's X and Y.
    pub fn pick(&mut self, face: CutPlane) {
        let (u, v) = face.axes();
        self.plane.base = face.point;
        self.plane.base_rotation = Xform::rotation_of_axes(u, v, face.normal);
        self.picked = true;
    }
}
