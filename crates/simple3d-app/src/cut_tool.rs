//! The tool that cuts the model in two along a face's plane (issue 130).
//!
//! A face is picked in the viewport, and every shown object its plane runs through is cut in two
//! there. The plane is endless by default; limited to an area, it is a rectangle round the face's
//! middle, and an object is cut only where its whole cross-section lies inside it, so each cut still
//! leaves two clean halves. Each object cut becomes a split holding the halves, which Join puts back
//! together; a piece of a split already there is halved within it. Cutting runs on a thread
//! ([`crate::worker::CutJob`]), and nothing changes until Cut. A non-modal [in-place
//! popup](crate::popup).

mod apply;
mod open;
mod plan;
mod pointer;
pub(crate) use pointer::*;
mod overlay;
pub(crate) use overlay::*;
mod window;
pub(crate) use window::*;
#[cfg(test)]
mod tests;

use simple3d_core::scene::NodeId;
use simple3d_core::xform::Xform;
use simple3d_geom::plane_cut::CutPlane;
use simple3d_geom::{Mesh, Vec3};
use std::sync::Arc;

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "cut-tool";

/// The window width: a labelled choice and two size fields.
const WIDTH: f32 = 320.0;

/// The area's sides when first limited, in millimetres.
const AREA: [f64; 2] = [50.0, 50.0];

/// What the tool is working on while its window is open.
pub struct CutTool {
    /// The plane through the picked face, facing out of it.
    pub face: Option<CutPlane>,
    /// Whether the cut reaches only over `size` round the face's middle, rather than endlessly.
    pub limited: bool,
    /// The area's width along the plane's first axis and height along its second
    /// ([`CutPlane::axes`]), in millimetres.
    pub size: [f64; 2],
    /// What Cut would cut, worked out when the face, area or document changes.
    planned: Option<(u64, Arc<Vec<Planned>>)>,
}

/// One object the plane runs through.
#[derive(Clone)]
pub(crate) struct Planned {
    pub(crate) node: NodeId,
    /// Its shape in its own frame.
    pub(crate) mesh: Arc<Mesh>,
    /// The plane in the shape's own frame.
    pub(crate) plane: CutPlane,
    /// Its world box, for drawing the endless plane's frame round what it cuts.
    pub(crate) bounds: (Vec3, Vec3),
    /// Where the plane crosses its surface, in the world, drawn while the tool is out.
    pub(crate) outline: Vec<[Vec3; 2]>,
}

impl CutTool {
    pub fn new() -> CutTool {
        CutTool { face: None, limited: false, size: AREA, planned: None }
    }

    /// Whether `point`, on the plane, lies within the area; always so while it is endless.
    pub(crate) fn within(&self, point: Vec3) -> bool {
        let Some(face) = self.face.filter(|_| self.limited) else { return true };
        let (u, v) = face.axes();
        let offset = point - face.point;
        // A hair of slack, so a cross-section exactly as wide as the area still counts.
        let slack = 1e-6 * (self.size[0] + self.size[1]).max(1.0);
        offset.dot(u).abs() <= self.size[0] / 2.0 + slack && offset.dot(v).abs() <= self.size[1] / 2.0 + slack
    }
}

impl Default for CutTool {
    fn default() -> CutTool {
        CutTool::new()
    }
}
