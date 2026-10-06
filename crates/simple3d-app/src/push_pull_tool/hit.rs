//! The face under the pointer.

use crate::app::App;
use crate::view::View;
use simple3d_core::scene::{CapturedFace, NodeId};
use simple3d_geom::push_pull::{face_basis, flat_face, FlatFace};
use simple3d_geom::Vec3;
use std::sync::Arc;

/// A face of the evaluated model and the object it belongs to.
#[derive(Clone)]
pub(crate) struct FaceHit {
    pub(crate) owner: NodeId,
    pub(crate) face: Arc<FlatFace>,
    /// The face's outline in a frame on it, as the extrusion will store it.
    pub(crate) captured: Arc<CapturedFace>,
}

/// Why a face cannot be pushed, for the status line. Curved surfaces are refused rather than
/// pushed facet by facet: one facet of a cylinder swept out is never what was meant.
const CURVED: &str = "Push / pull works on flat faces; this is a facet of a curved surface";

impl App {
    /// The face of the model under `cursor`, and where the ray met it. `Err` with a reason when the
    /// pointer is over the model but on nothing that can be pushed.
    pub(crate) fn face_under(&self, view: &View, cursor: egui::Pos2) -> Option<(Result<FaceHit, &'static str>, Vec3)> {
        let mesh = &self.evaluated.mesh;
        let (origin, direction) = view.ray(cursor);
        let (t, triangle) = crate::pick::ray_mesh_triangle(mesh, origin, direction)?;
        let at = origin + direction * t;
        Some((self.face_of(triangle), at))
    }

    /// The face `triangle` of the evaluated model is part of, cached for a still pointer.
    pub(crate) fn face_of(&self, triangle: usize) -> Result<FaceHit, &'static str> {
        let mesh = &self.evaluated.mesh;
        let key = (Arc::as_ptr(mesh) as usize, triangle);
        if let Some((addr, at, hit)) = self.push_pull.hover.borrow().as_ref() {
            if (*addr, *at) == key {
                return hit.clone();
            }
        }
        let found = face_hit(self, triangle);
        *self.push_pull.hover.borrow_mut() = Some((key.0, key.1, found.clone()));
        found
    }
}

fn face_hit(app: &App, triangle: usize) -> Result<FaceHit, &'static str> {
    let mesh = &app.evaluated.mesh;
    let owner = NodeId::from(mesh.source(triangle));
    if owner == 0 || !app.scene.contains(owner) {
        return Err("That face belongs to no object of this document");
    }
    let face = flat_face(mesh, triangle).ok_or("That face's outline touches itself and cannot be traced")?;
    if face.curved {
        return Err(CURVED);
    }
    let (u, v) = face_basis(face.normal);
    let outline = face.outline(face.centre, u, v).ok_or("That face's outline cannot be traced")?;
    let captured = CapturedFace { outline, origin: face.centre, u, v, normal: face.normal };
    Ok(FaceHit { owner, face: Arc::new(face), captured: Arc::new(captured) })
}
