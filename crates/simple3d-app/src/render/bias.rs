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

/// Grid and axes are biased away from the eye so coplanar faces hide them (the grid used to cross a
/// plate's side walls). Axes less than the grid, since X and Y lie on grid lines.
pub(crate) const GRID_BIAS: f32 = -8.0e-4;

pub(crate) const AXIS_BIAS: f32 = -5.0e-4;

/// Plane marks must beat their surface and its feature edges. Below about 2e-3 they dash; an order
/// of magnitude above, they show through the far side.
pub(crate) const MARK_BIAS: f32 = 3.0e-3;

/// Preview loops lie on surfaces like plane marks, so the same bias and no more.
pub(crate) const PREVIEW_BIAS: f32 = 3.0e-3;
