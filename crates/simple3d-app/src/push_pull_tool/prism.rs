//! The solid a drag would make, drawn in the picture where the bodies in front can hide it.

use super::*;
use crate::raster::Rgba;
use crate::render::{Palette, Renderable};
use simple3d_core::xform::Xform;
use simple3d_geom::push_pull::extrude_outline;
use std::sync::Arc;

impl PushDrag {
    /// Remake the swept solid for the current distance, in world space; none at zero.
    pub(crate) fn sweep(&mut self) {
        if self.prism.as_ref().is_some_and(|(at, _)| *at == self.distance) {
            return;
        }
        let face = &self.hit.captured;
        let mut mesh = extrude_outline(&face.outline, self.distance.abs());
        if mesh.indices.is_empty() {
            self.prism = None;
            return;
        }
        // A pull sweeps into the model; mirroring the sweep turns the faces inside out, so they are
        // turned back.
        let sign = self.distance.signum();
        for p in mesh.positions.iter_mut() {
            *p = face.origin + face.u * p.x + face.v * p.y + face.normal * (p.z * sign);
        }
        if sign < 0.0 {
            mesh.flip_winding();
        }
        self.prism = Some((self.distance, Arc::new(Renderable::surface_with_edges(&mesh))));
    }
}

/// The dragged solid as a template for the renderer: green where it adds, red where it cuts.
/// Takes the tool alone, as the picture is drawn while the app's other fields are borrowed.
pub(crate) fn template<'a>(tool: &'a PushPull, palette: &Palette) -> Option<(&'a Renderable, Xform, Option<Rgba>)> {
    let drag = tool.drag.as_ref()?;
    let (distance, prism) = drag.prism.as_ref()?;
    let colour = if *distance > 0.0 { palette.extrusion } else { palette.reduction };
    Some((&**prism, Xform::IDENTITY, Some(colour)))
}
