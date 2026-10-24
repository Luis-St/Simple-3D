//! How far the ground grid reaches and which levels of it are drawn.

use super::GridPlane;
use crate::view::View;

/// Half the viewport diagonal in millimetres: the frame's own reach. Sizes the pinned axis
/// cross, which must not grow with tilt.
pub fn frame_reach(view: &View) -> f64 {
    let half_diagonal = ((view.size.x as f64).hypot(view.size.y as f64) / 2.0).max(1.0);
    half_diagonal / view.pixels_per_mm().max(1e-9)
}

/// How far past the face-on reach a tilted ground may extend; near edge-on it would be unbounded.
pub(crate) const MAX_TILT_REACH: f64 = 6.0;

/// The world radius the ground grid and axes cover.
pub fn grid_radius(view: &View) -> f64 {
    grid_radius_on(view, &GridPlane::ground(1.0))
}

/// The radius a grid on `plane` covers round the point under the camera target.
///
/// Derived from the frame, so it changes smoothly with zoom rather than jumping with the spacing.
/// The frame's corners are projected onto the plane and the furthest is used, since a tilted
/// plane reaches much further along the view than across it.
pub fn grid_radius_on(view: &View, plane: &GridPlane) -> f64 {
    let face_on = frame_reach(view);
    let (s, t) = plane.local(view.camera().target);
    let centre = plane.at(s, t);
    let half = view.size / 2.0;
    let mut reach: f64 = 0.0;
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let corner = egui::pos2(view.centre.x + half.x * sx, view.centre.y + half.y * sy);
        // Edge-on: the plane is a line on screen, with no extent to cover.
        let Some(hit) = view.ray_plane(corner, plane.origin, plane.normal()) else {
            return face_on * 1.35;
        };
        reach = reach.max((hit - centre).length());
    }
    // A little past the corner, and never less than face-on, so a top view is unchanged.
    (reach * 1.08).clamp(face_on * 1.35, face_on * MAX_TILT_REACH)
}

/// The narrowest a grid cell is drawn, in pixels, and the width of full strength; in between it
/// fades, so decades arrive without a step.
pub(crate) const CELL_FADE_OUT: f64 = 6.0;

pub(crate) const CELL_FULL: f64 = 26.0;

/// How much of a ground cell survives the view's tilt, as a factor on its screen size.
///
/// Foreshortening shrinks cells seen from low down, so this steps the grid up as the view
/// flattens, like zooming out. One number per frame (parallel projection); the square root of
/// the foreshortening (cell area) keeps orbiting from jumping decades.
pub(crate) fn ground_squash(view: &View, plane: &GridPlane) -> f64 {
    view.forward().dot(plane.normal()).abs().max(1e-3).sqrt()
}

/// The two grid decades at this zoom: the coarse one at full strength, the fine one below it,
/// and the fine one's strength. The fine level fades between `CELL_FULL` and `CELL_FADE_OUT`, so
/// stepping up changes nothing on screen.
pub fn grid_levels(view: &View, spacing: f64) -> (f64, f64, f64) {
    grid_levels_on(view, &GridPlane::ground(spacing))
}

/// [`grid_levels`] for a grid on `plane`, at its own spacing.
pub fn grid_levels_on(view: &View, plane: &GridPlane) -> (f64, f64, f64) {
    let spacing = plane.spacing.max(1e-6);
    let pixels_per_mm = view.pixels_per_mm() * ground_squash(view, plane);
    let mut coarse = spacing;
    // Bounded, so an absurd zoom cannot loop forever.
    for _ in 0..40 {
        if coarse * pixels_per_mm >= CELL_FULL {
            break;
        }
        coarse *= 10.0;
    }
    if coarse <= spacing * 1.000_001 {
        // The document's spacing is already wide enough: no finer decade to fade in.
        return (spacing, spacing, 0.0);
    }
    let fine = coarse / 10.0;
    let cell = fine * pixels_per_mm;
    let t = ((cell - CELL_FADE_OUT) / (CELL_FULL - CELL_FADE_OUT)).clamp(0.0, 1.0);
    // Smoothstep, so the fine grid arrives and leaves without an edge.
    (fine, coarse, t * t * (3.0 - 2.0 * t))
}

/// Where one grid decade on `plane` is drawn, in its own coordinates: centred on the camera target
/// snapped to its spacing, so lines sit on whole multiples counted from the plane's origin and the
/// axes lie along the ground's grid, and how far either side, `MAX_LINES` at most.
pub(crate) fn grid_square_on(view: &View, plane: &GridPlane, spacing: f64, radius: f64) -> ((f64, f64), f64) {
    let lines = ((radius / spacing).ceil() as i64).clamp(1, MAX_LINES);
    let (s, t) = plane.local(view.camera().target);
    let cx = (s / spacing).round() * spacing;
    let cy = (t / spacing).round() * spacing;
    ((cx, cy), spacing * lines as f64)
}

/// The legible grid spacing at this zoom: `grid_levels`' coarse decade.
pub fn effective_grid_spacing(view: &View, spacing: f64) -> f64 {
    grid_levels(view, spacing).1
}

/// The most lines per grid level either side of its centre, bounding pathological cases only;
/// off-frame lines are dropped by `visible_span` anyway.
pub(crate) const MAX_LINES: i64 = 400;
