//! How far the ground grid reaches and which levels of it are drawn.

use crate::view::View;
use simple3d_geom::Vec3;

/// Half the viewport's diagonal, in millimetres at the current zoom: how far
/// the frame itself reaches, with no ground plane involved. What the pinned
/// axis cross is measured against -- it is a mark on the frame, so it is sized
/// by the frame, and it must not grow when a tilt makes the ground reach
/// further or the two axis styles stop being two different pictures.
pub fn frame_reach(view: &View) -> f64 {
    let half_diagonal = ((view.size.x as f64).hypot(view.size.y as f64) / 2.0).max(1.0);
    half_diagonal / view.pixels_per_mm().max(1e-9)
}

/// How much further than the face-on reach a tilted ground may be asked to
/// cover. A view a few degrees off edge-on would otherwise want a grid of
/// unbounded extent, and by then its lines are a wash rather than a measure.
pub(crate) const MAX_TILT_REACH: f64 = 6.0;

/// The world radius the ground grid and the axes cover.
///
/// It is derived from the frame rather than from the grid's spacing: the
/// furthest the viewport reaches across the ground plane at the current zoom.
/// That makes it continuous in the zoom -- the extent of the ground grows
/// smoothly as the camera pulls back, instead of jumping tenfold whenever the
/// spacing steps up a decade, which is what made zooming out lurch.
///
/// The ground is only face-on from straight above. Seen at an angle it is
/// foreshortened, so the frame reaches much further across it along the view
/// than across it sideways -- half the viewport's diagonal is the right answer
/// for a top view and far too small for any other. Taking it as the answer for
/// all of them left the grid stopping short of the top and bottom of the
/// viewport, in a flattened diamond, while the axes carried on past it. So the
/// four corners of the frame are put back onto the ground and the furthest one
/// is what the grid has to reach.
pub fn grid_radius(view: &View) -> f64 {
    let face_on = frame_reach(view);
    let centre = Vec3::new(view.camera.target.x, view.camera.target.y, 0.0);
    let half = view.size / 2.0;
    let mut reach: f64 = 0.0;
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let corner = egui::pos2(view.centre.x + half.x * sx, view.centre.y + half.y * sy);
        // Edge-on: the ground is a line on the screen, with no extent to cover.
        let Some(hit) = view.ray_plane(corner, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)) else {
            return face_on * 1.35;
        };
        reach = reach.max((hit - centre).length());
    }
    // A little past the corner it is reaching, and never less than the face-on
    // answer, so a top view keeps exactly the extent it had.
    (reach * 1.08).clamp(face_on * 1.35, face_on * MAX_TILT_REACH)
}

/// The narrowest a grid cell may be drawn, in pixels, and the width at which it
/// is drawn at full strength. Between the two it is faded, which is what makes a
/// decade of the grid arrive and leave without a step.
pub(crate) const CELL_FADE_OUT: f64 = 6.0;

pub(crate) const CELL_FULL: f64 = 26.0;

/// How much of a ground cell survives the tilt of the view, as a factor on its
/// size on screen.
///
/// A cell is only square on the screen from straight above. At an angle the
/// ground is foreshortened along the view -- at eight degrees a cell keeps a
/// seventh of its depth -- so a spacing that is perfectly legible from above is
/// a wash of lines seen from low down, and the finer of the two levels is a
/// wash covering the whole frame. Asking how big a cell really is on the screen
/// makes the level step up as the view flattens, exactly as it does when the
/// camera pulls back, so the grid stays a measure rather than a texture.
///
/// The projection is parallel, so this is one number for the whole frame rather
/// than something that varies across it: there is no horizon for cells to pile
/// up against. It is the square root of the foreshortening because what is
/// being judged is the cell's *area* on the screen -- the geometric mean of its
/// two sides, one of them untouched -- which keeps the step gentle enough that
/// orbiting does not walk the grid up and down a decade at a time.
pub(crate) fn ground_squash(view: &View) -> f64 {
    view.forward().z.abs().max(1e-3).sqrt()
}

/// The two decades of grid to draw at this zoom: the coarse one, always at full
/// strength, the fine one below it, and how strongly that fine one shows.
///
/// The coarse spacing is the first decade of the document's own spacing whose
/// cell is at least `CELL_FULL` across, so it is never a solid block of lines.
/// The fine spacing is the decade under it, faded out as its cells shrink from
/// `CELL_FULL` to `CELL_FADE_OUT`. At the moment the coarse level steps up, the
/// level it replaces is exactly `CELL_FULL` across and fully drawn, so the
/// picture does not change: the tenfold jump the old single-level grid made is
/// spread across the whole decade of zoom instead.
pub fn grid_levels(view: &View, spacing: f64) -> (f64, f64, f64) {
    let spacing = spacing.max(1e-6);
    let pixels_per_mm = view.pixels_per_mm() * ground_squash(view);
    let mut coarse = spacing;
    // Bounded: an absurd zoom cannot ask for an unbounded number of decades.
    for _ in 0..40 {
        if coarse * pixels_per_mm >= CELL_FULL {
            break;
        }
        coarse *= 10.0;
    }
    if coarse <= spacing * 1.000_001 {
        // The document's own spacing is already wide enough, so there is no
        // finer decade to fade in under it.
        return (spacing, spacing, 0.0);
    }
    let fine = coarse / 10.0;
    let cell = fine * pixels_per_mm;
    let t = ((cell - CELL_FADE_OUT) / (CELL_FULL - CELL_FADE_OUT)).clamp(0.0, 1.0);
    // Smoothstep, so the fine grid arrives and leaves without an edge.
    (fine, coarse, t * t * (3.0 - 2.0 * t))
}

/// Grid spacing that is legible at this zoom: the coarse decade of
/// `grid_levels`. The axes are laid out on it, and the tool rail reads it to
/// say what one grid square means.
pub fn effective_grid_spacing(view: &View, spacing: f64) -> f64 {
    grid_levels(view, spacing).1
}

/// How many lines one level of the grid may draw either side of its centre.
/// The fine level at its densest would otherwise be several thousand, which is
/// work for lines that are all but invisible by then.
///
/// Enough that the cap is not what decides the extent at any ordinary window
/// size and tilt -- it is a bound on the pathological case, not a second
/// answer to "how far does the ground reach". A line that misses the frame is
/// dropped by `visible_span` before it is rasterized, so the ones this bounds
/// are cheap to begin with.
pub(crate) const MAX_LINES: i64 = 400;
