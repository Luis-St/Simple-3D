//! The tool that puts a mesh back into objects and groups (issue 108).
//!
//! Converting a shape to a mesh is the one door in the application that only
//! opens one way: the recipe is gone and what is left is triangles. Importing
//! a printer file arrives on the far side of that door to begin with -- a
//! whole assembly as one body, with nothing in it that can be edited except
//! the transform of the lot. This is the way back. It separates the mesh into
//! the bodies it is really in, works out which of them are shapes the
//! application can build, and stands a group where the mesh stood holding a box
//! that is a box again and a cylinder whose diameter can be typed.
//!
//! See [`simple3d_geom::reassemble`] for how a body is recognised and what the
//! recognition will and will not claim.
//!
//! ## The preview is drawn, not stood in the document
//!
//! Unlike the simplify tool, which puts its result on the node while the window
//! is open, this one draws what it found *over* the mesh and leaves the
//! document alone until Reassemble is pressed. The difference is what the two
//! results are. A simplification is one mesh replacing one mesh -- a pointer
//! written and a pointer written back -- and what there is to judge is the
//! surface, which has to be the real surface. A reassembly is a subtree
//! replacing a node: new ids, a new selection, a new shape of tree, and putting
//! that in and taking it out again on every turn of a number would be the
//! outliner rebuilding itself under the pointer. What there is to judge here is
//! not a surface but a *reading* -- which bodies were found, which of them were
//! recognised, what each one was called -- and the honest way to show a reading
//! is to draw it on the thing being read.
//!
//! So the shapes that were recognised are drawn as they would be built, on the
//! model, with the depth buffer, and the bodies that were not are drawn as the
//! boxes they will be kept in. What the viewport does underneath is the
//! document's own [`PreviewViewport`](simple3d_core::scene::PreviewViewport)
//! setting, the same one the pattern, split and simplify tools preview through.
//!
//! It is an [in-place popup](crate::popup), non-modal like the rest, so the
//! model underneath can be orbited and zoomed while the numbers are being
//! turned -- which is the only way to see whether the pin it found really is
//! the pin.

mod open;
mod preview;
pub(crate) use preview::*;
mod run;
mod window;
pub(crate) use window::*;
mod controls;
pub(crate) use controls::*;
mod summary;
pub(crate) use summary::*;

use crate::worker::ReassembleJob;
use simple3d_core::mesh_data::MeshData;
use simple3d_core::primitive::ParamKind;
use simple3d_core::scene::NodeId;
use simple3d_core::xform::Xform;
use simple3d_geom::reassemble::{Assembly, Reassemble};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// What the tool is working on while its window is open.
pub struct ReassembleTool {
    pub target: NodeId,
    /// The mesh as the document holds it. Re-read whenever it changes under
    /// the tool -- an undo, a paste -- because the window is not modal and what
    /// is on the node now is what there is to take apart.
    pub mesh: Arc<MeshData>,
    pub plan: Reassemble,
    /// What the last finished run found, and what it was asked for.
    pub found: Option<Found>,
    /// The run in flight. At most one: a scrub asks for a new answer on every
    /// frame it moves, and what is wanted is the newest of those.
    pub job: Option<ReassembleJob>,
    /// Whether what was found is drawn over the mesh.
    pub outlines: bool,
    /// Where the mesh's own frame stands in the world, so what was found can be
    /// drawn on the model rather than beside it. The bodies are worked out in
    /// the mesh's own frame -- the frame the node's geometry is in and the one
    /// a tolerance in millimetres means something in -- and this is the way
    /// back out to the viewport.
    pub placement: Xform,
    /// Which evaluation `placement` was taken from, so the tool can tell when
    /// what it is drawing has gone stale. The window is not modal, and the mesh
    /// can be moved while it is open.
    pub generation: u64,
}

impl ReassembleTool {
    /// Everything the preview drawn over the model depends on, for the key the
    /// viewport's cached image is rebuilt on. What was *found* is not hashed:
    /// it is thousands of points, and it is exactly what these numbers came to.
    pub(crate) fn hash_preview<H: Hasher>(&self, hasher: &mut H) {
        self.target.hash(hasher);
        self.generation.hash(hasher);
        self.outlines.hash(hasher);
        self.found.is_some().hash(hasher);
        self.plan.recognise.hash(hasher);
        self.plan.group_touching.hash(hasher);
        self.plan.max_objects.hash(hasher);
        self.plan.tolerance.to_bits().hash(hasher);
        for row in self.placement.m {
            for number in row {
                number.to_bits().hash(hasher);
            }
        }
        for number in [self.placement.t.x, self.placement.t.y, self.placement.t.z] {
            number.to_bits().hash(hasher);
        }
    }
}

/// A finished run, and the numbers it answered.
pub struct Found {
    pub plan: Reassemble,
    pub assembly: Arc<Assembly>,
    /// What is drawn over the mesh, in the mesh's own frame.
    ///
    /// Worked out once, when the run lands, rather than on every frame: what
    /// the lines are does not change while the numbers do not, and the mesh
    /// being moved about under the window changes only where they are put.
    pub outline: Vec<Vec<simple3d_geom::Vec3>>,
    /// Whether that is few enough lines to draw at all.
    pub drawn: bool,
}

/// Identifies the popup, and is what remembers where it was dragged to.
const KEY: &str = "reassemble-tool";

/// How wide the window is: a label, its field, and no more. A popup lives over
/// the model, so every pixel of it is a pixel of the thing being taken apart
/// that cannot be seen.
const WIDTH: f32 = 340.0;

/// How wide a number field is: enough for a length with its unit on it.
const FIELD_WIDTH: f32 = 90.0;

/// How far a surface may be from the shape fitted to it. Zero is allowed and
/// means what it says: nothing but a shape this application generated itself.
const TOLERANCE: ParamKind = ParamKind::Length { min: 0.0 };

/// The most objects to place. One is the whole mesh as one object, which is
/// what it already is; the top is past any document this is a modeller for, and
/// is there to stop a typed number rather than to be reached.
const OBJECTS: ParamKind = ParamKind::Count { min: 1, max: 10_000 };

/// The most line loops the preview will draw in a frame.
///
/// Past this it is not drawn at all, and the window says so. Part of a picture
/// of two hundred parts is a picture of a different two hundred parts, and the
/// loops are redrawn on every orbit of a model that is already being drawn.
const PREVIEW_LOOPS: usize = 12_000;
