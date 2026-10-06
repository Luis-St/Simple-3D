//! How far each kind of line is pulled towards the camera, so lines on a surface show in front of it.

/// Line depth bias as a fraction of its own depth key, so it holds at every zoom; a fixed nudge
/// would exceed the scene's depth range from afar.
pub(crate) const EDGE_BIAS: f32 = 2.0e-4;

/// Above [`MARK_BIAS`], so a plane mark crossing the selection outline no longer paints over it (issue 99).
pub(crate) const SELECTION_BIAS: f32 = 4.0e-3;

/// The outline is also pulled forward by this many pixels of the steepest drawn face beside it: a
/// line pixel lands up to that far onto a face seen nearly edge-on, whose depth then changes too fast
/// for [`SELECTION_BIAS`] alone, and whole stretches of the outline went missing (issue 99).
pub(crate) const SELECTION_SLOPE_PIXELS: f32 = 2.0;

/// The most the slope may add, as a fraction of the depth key, so an edge-on face cannot pull the
/// outline through everything in front of it.
pub(crate) const SELECTION_SLOPE_CAP: f32 = 2.0e-2;

/// Feature edges are pulled forward by this many pixels of the steepest drawn face beside them, as
/// the outline is: with [`EDGE_BIAS`] alone, an edge beside a face seen at a grazing angle sank into
/// it and drew dashed or not at all (issue 115). One pixel covers a line pixel's centre lying up to
/// half a pixel onto that face.
pub(crate) const EDGE_SLOPE_PIXELS: f32 = 1.0;

/// The most the slope may add to an edge, as a fraction of its depth key. Below the outline's cap,
/// since a feature edge pulled too far shows through the faces in front of it.
pub(crate) const EDGE_SLOPE_CAP: f32 = 5.0e-3;

/// How fast a projected triangle's depth key changes per pixel; nearly edge-on faces change fastest,
/// and a triangle seen exactly edge-on reports `f32::MAX`.
pub(crate) fn depth_slope([a, b, c]: [crate::raster::Vertex; 3]) -> f32 {
    let (e1, e2) = (b.pos - a.pos, c.pos - a.pos);
    let det = e1.x * e2.y - e2.x * e1.y;
    if det.abs() < 1e-6 {
        return f32::MAX;
    }
    let (k1, k2) = (b.key - a.key, c.key - a.key);
    egui::vec2(k1 * e2.y - k2 * e1.y, e1.x * k2 - e2.x * k1).length() / det.abs()
}

/// Grid and axes are biased away from the eye so coplanar faces hide them (the grid used to cross a
/// plate's side walls). Axes less than the grid, since X and Y lie on grid lines.
pub(crate) const GRID_BIAS: f32 = -8.0e-4;

pub(crate) const AXIS_BIAS: f32 = -5.0e-4;

/// Plane marks must beat their surface and its feature edges. Below about 2e-3 they dash; an order
/// of magnitude above, they show through the far side.
pub(crate) const MARK_BIAS: f32 = 3.0e-3;

/// Preview loops lie on surfaces like plane marks, so the same bias and no more.
pub(crate) const PREVIEW_BIAS: f32 = 3.0e-3;
