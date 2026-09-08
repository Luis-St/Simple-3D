//! How far each kind of line is pulled towards the camera, so that what is
//! drawn on a surface is drawn in front of it.

/// Depth bias for a line, as a fraction of its own depth key rather than an
/// absolute amount, so one set of numbers holds at every zoom: the key is the
/// distance from the eye, which grows with the camera's own distance, and a
/// fixed nudge that is invisible up close would be larger than the whole
/// scene's depth range when the camera is far away.
pub(crate) const EDGE_BIAS: f32 = 2.0e-4;

pub(crate) const SELECTION_BIAS: f32 = 8.0e-4;

/// The grid and the axes are biased *away* from the eye, so a face that happens
/// to be coplanar with one of them hides it. Without this a plate 4mm thick and
/// centred on the origin has the ground grid drawn straight across its side
/// walls, because the wall and the grid line tie at exactly equal depth and the
/// grid got there first. The axes are biased slightly less than the grid,
/// because the X and Y axes lie exactly along grid lines and would otherwise
/// lose that tie in turn.
pub(crate) const GRID_BIAS: f32 = -8.0e-4;

pub(crate) const AXIS_BIAS: f32 = -5.0e-4;

/// A plane mark sits *on* the surface it is drawn on, so it needs to win the
/// tie against that surface -- and against the feature edges of the same
/// solid, which is why it is biased further than they are. Found by looking:
/// below about 2e-3 the mark breaks into dashes wherever the line's own
/// interpolated depth rounds behind the face it lies on, and an order of
/// magnitude above this it starts showing through the far side of a solid.
pub(crate) const MARK_BIAS: f32 = 3.0e-3;

/// A preview loop sits on the surface it is drawn over exactly as a plane mark
/// does -- the cells at the ends of a run lie in the faces the run starts and
/// stops at -- so it needs the same bias to win that tie, and no more, or it
/// starts showing through the far side of the solid.
pub(crate) const PREVIEW_BIAS: f32 = 3.0e-3;
