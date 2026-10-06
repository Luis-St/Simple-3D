//! The material the draft takes away (red) and adds (green), drawn in the picture where the bodies in
//! front can hide it, like push/pull's swept solid.

use super::*;
use crate::raster::Rgba;
use crate::render::{Palette, Renderable};
use simple3d_core::xform::Xform;
use simple3d_geom::rounding::{corner_sliver, edge_ends, edge_sliver};

/// The two renderables, either absent when nothing goes or comes.
#[derive(Default)]
pub(crate) struct Ghost {
    pub(crate) cut: Option<Arc<Renderable>>,
    pub(crate) add: Option<Arc<Renderable>>,
}

impl Ghost {
    /// The exact slivers of the targets with room for the treatment, which stop at the faces where
    /// the cutters overshoot, mitred where two meet at an inside corner as their cutters are.
    pub(crate) fn build(tool: &RoundTool, edges: &[(NodeId, FeatureEdge)], corners: &[(NodeId, Corner)]) -> Ghost {
        let treatment = tool.treatment();
        let (mut cut, mut add) = (Mesh::new(), Mesh::new());
        // An edge merged across objects comes once for each of them.
        let mut fitting: Vec<FeatureEdge> = Vec::new();
        for (_, edge) in edges.iter().filter(|(_, edge)| tool.fits(edge)) {
            if !fitting.iter().any(|e| super::pick::same_edge(e, edge)) {
                fitting.push(*edge);
            }
        }
        for (index, edge) in fitting.iter().enumerate() {
            let ends = edge_ends(&tool.base, &fitting, index, treatment, &tool.joints);
            if let Some(sliver) = edge_sliver(edge, treatment, ends) {
                if edge.convex { &mut cut } else { &mut add }.append(&sliver);
            }
        }
        for (_, corner) in corners.iter().filter(|(_, c)| simple3d_geom::rounding::corner_fits(c, treatment)) {
            if let Some(sliver) = corner_sliver(corner, treatment) {
                cut.append(&sliver);
            }
        }
        let renderable =
            |mesh: Mesh| (!mesh.indices.is_empty()).then(|| Arc::new(Renderable::surface_with_edges(&mesh)));
        Ghost { cut: renderable(cut), add: renderable(add) }
    }
}

/// The ghost as templates for the renderer, red where it cuts and green where it adds. Takes the tool
/// alone, as the picture is drawn while the app's other fields are borrowed.
pub(crate) fn templates<'a>(
    tool: Option<&'a RoundTool>,
    palette: &Palette,
) -> impl Iterator<Item = (&'a Renderable, Xform, Option<Rgba>)> {
    let ghost = tool.map(|tool| &tool.ghost);
    let cut = ghost.and_then(|g| g.cut.as_deref()).map(|r| (r, Xform::IDENTITY, Some(palette.reduction)));
    let add = ghost.and_then(|g| g.add.as_deref()).map(|r| (r, Xform::IDENTITY, Some(palette.extrusion)));
    cut.into_iter().chain(add)
}

impl RoundTool {
    /// What the picture shows of the tool, for the image key.
    pub(crate) fn picture_key(&self) -> Option<u64> {
        self.made_for
    }
}
