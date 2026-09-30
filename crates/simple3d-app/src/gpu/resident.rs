//! Meshes the GPU renderer keeps on the card between frames.
//!
//! Uploaded once and projected per frame from the camera alone; everything camera-independent
//! (faces, edges, outlines, crossings, caps) is derived on the card. Positions are stored in
//! single precision relative to the mesh's centre, and each part is uploaded only when first needed.

use super::*;
use crate::raster::Rgba;
use crate::render::{
    mark_colours, shade, tag_bases, to_vertex, Live, Palette, Renderable, Request, Style, EDGE_BIAS, EDGE_ON,
    MARK_BIAS, PREVIEW_BIAS, SELECTION_BIAS, SELECTION_CREASE, SELECTION_SLOPE_CAP, SELECTION_SLOPE_PIXELS,
};
use crate::snap::MARK_AXIS;
use crate::view::View;
use eframe::glow::{self, HasContext};
use simple3d_core::config::DisplayMode;
use simple3d_core::xform::Xform;
use simple3d_geom::aabb::box_corner;
use simple3d_geom::section::Plane;
use simple3d_geom::Vec3;

mod caps;
mod draw;
mod plan;
mod project;
mod upload;
pub(super) use plan::{mark_planes, plan};
pub(super) use project::{front_face, rows_at, set_forward, set_projection, set_within};

/// A vertex array and the element or vertex buffer it draws from.
pub(super) struct Batch {
    pub(super) array: glow::VertexArray,
    buffer: glow::Buffer,
    pub(super) count: i32,
}

/// The mesh's vertices: position and body per welded vertex.
struct Vertices {
    positions: glow::Buffer,
    bodies: glow::Buffer,
}

/// The mesh laid out in textures, for stages that look up neighbouring triangles (the outline).
struct Tables {
    positions: glow::Texture,
    triangles: glow::Texture,
    bodies: glow::Texture,
}

pub(crate) struct Resident {
    /// Mesh centre the positions are stored relative to, keeping single-precision values small.
    origin: Vec3,
    /// Mesh extent relative to `origin`, used to size the depth range.
    lo: Vec3,
    hi: Vec3,
    vertices: Option<Vertices>,
    pub(super) faces: Option<Batch>,
    /// One colour tag per triangle, when any triangle is painted.
    pub(super) paint: Option<glow::Texture>,
    pub(super) edges: Option<Batch>,
    tables: Option<Tables>,
    outline: Option<Batch>,
}

/// Which parts of a resident a frame draws with.
#[derive(Clone, Copy, Default)]
pub(super) struct Needs {
    faces: bool,
    edges: bool,
    outline: bool,
}

/// Where a resident is drawn this frame: moved by a drag, with a vertex range hidden.
#[derive(Clone, Copy)]
pub(super) struct Placing {
    xform: Xform,
    hide: [u32; 2],
}

impl Placing {
    pub(super) const NONE: Placing = Placing { xform: Xform::IDENTITY, hide: [0, 0] };

    pub(super) fn moved(xform: Option<Xform>) -> Placing {
        Placing { xform: xform.unwrap_or(Xform::IDENTITY), hide: [0, 0] }
    }

    fn of(live: &Live, id: u64) -> Placing {
        Placing {
            xform: live.placed(id).copied().unwrap_or(Xform::IDENTITY),
            hide: live.hidden(id).map_or([0, 0], |range| [range.start, range.end]),
        }
    }
}

/// What one frame asks of the resident meshes.
#[derive(Default)]
pub(super) struct Plan {
    pub(super) solids: Vec<FaceDraw>,
    pub(super) lines: Vec<LineDraw>,
    pub(super) ghosts: Vec<FaceDraw>,
    pub(super) glows: Vec<FaceDraw>,
    /// A tool's templates (issue 70): shaded like ghosts, and their edges where seen and where hidden.
    pub(super) templates: Vec<FaceDraw>,
    pub(super) template_edges: Vec<LineDraw>,
    pub(super) template_hidden: Vec<LineDraw>,
    /// Selected and glowing bodies' outlines.
    pub(super) outlines: Vec<OutlineDraw>,
    /// Plane marks and the edge round a section's cut.
    pub(super) crossings: Vec<CrossingDraw>,
    /// A section's cap, filled where the cut runs through material.
    pub(super) caps: Vec<CapDraw>,
    /// Tool preview lines drawn over the model, depth-tested but not written.
    pub(super) overlays: Vec<LineDraw>,
    /// Shapes a boolean preview is drawn from (`csg.rs`).
    pub(super) csg: Vec<u64>,
    /// Edge colour of a boolean preview, when the display mode draws lines.
    pub(super) csg_edges: Option<Rgba>,
}

pub(super) struct FaceDraw {
    id: u64,
    placing: Placing,
    mode: i32,
    base: Rgba,
    tag_base: u16,
}

pub(super) struct LineDraw {
    id: u64,
    placing: Placing,
    colour: Rgba,
    bias: f32,
    tag_base: Option<u16>,
}

impl LineDraw {
    /// A tool's preview loops, biased towards the eye like `push_preview`.
    pub(super) fn preview(id: u64, colour: Rgba) -> LineDraw {
        LineDraw { id, placing: Placing::NONE, colour, bias: PREVIEW_BIAS, tag_base: None }
    }

    /// A tool template's edges, biased like `preview` since they may lie on a body's face.
    pub(super) fn template(id: u64, placing: Placing, colour: Rgba) -> LineDraw {
        LineDraw { id, placing, colour, bias: PREVIEW_BIAS, tag_base: None }
    }
}

pub(super) struct OutlineDraw {
    id: u64,
    placing: Placing,
    colour: Rgba,
    tag_base: u16,
    /// Every crease rather than only those facing the eye: wireframe.
    all_creases: bool,
}

pub(super) struct CrossingDraw {
    id: u64,
    placing: Placing,
    /// Each plane, and the colour its crossing is drawn in.
    planes: Vec<(Plane, Rgba)>,
    /// Sections that cut the crossing. A cut's own edge is cut only by the others,
    /// since its own plane test would fray it.
    cuts: Vec<Plane>,
    /// The other walls of the box whose face the line lies on (`BOX_COMMON`'s `u_within`).
    within: Vec<Plane>,
}

pub(super) struct CapDraw {
    id: u64,
    placing: Placing,
    pub(super) plane: Plane,
    /// The other walls of a windowed section's box; empty for an unbounded plane.
    pub(super) bounds: Vec<Plane>,
    /// Other sections, whose cuts are taken out of this cap.
    pub(super) others: Vec<Plane>,
    pub(super) colour: Rgba,
    /// The unpainted cut colour, before shading, for the cap's faces (`draw_caps`).
    pub(super) base: Rgba,
    /// The plane's polygon within the mesh's box, projected.
    pub(super) fill: Vec<GpuVertex>,
}

const SOLID: i32 = 0;
const GHOST: i32 = 1;
const GLOW: i32 = 2;
const CAP: i32 = 3;
