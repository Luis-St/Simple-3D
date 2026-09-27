//! How far each kind of line is pulled towards the camera, so lines on a surface show in front of it.

/// Line depth bias as a fraction of its own depth key, so it holds at every zoom; a fixed nudge
/// would exceed the scene's depth range from afar.
pub(crate) const EDGE_BIAS: f32 = 2.0e-4;

pub(crate) const SELECTION_BIAS: f32 = 8.0e-4;

/// Grid and axes are biased away from the eye so coplanar faces hide them (the grid used to cross a
/// plate's side walls). Axes less than the grid, since X and Y lie on grid lines.
pub(crate) const GRID_BIAS: f32 = -8.0e-4;

pub(crate) const AXIS_BIAS: f32 = -5.0e-4;

/// Plane marks must beat their surface and its feature edges. Below about 2e-3 they dash; an order
/// of magnitude above, they show through the far side.
pub(crate) const MARK_BIAS: f32 = 3.0e-3;

/// Preview loops lie on surfaces like plane marks, so the same bias and no more.
pub(crate) const PREVIEW_BIAS: f32 = 3.0e-3;
