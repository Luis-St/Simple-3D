//! Drawing the ground grid, faded out at its edge.

use super::*;
use crate::raster::Rgba;
use crate::view::View;
use simple3d_geom::Vec3;

pub(crate) fn push_grid(steps: &mut Vec<Step>, view: &View, grid: &Grid, palette: &Palette) {
    let (fine, coarse, strength) = grid_levels(view, grid.spacing);
    let radius = grid_radius(view);
    // Both levels cover the same ground. Drawing the fine one over a shorter
    // reach made the grid detailed around the origin and coarse everywhere
    // else, so the ground read as a patch of detail sitting on a plainer one
    // rather than as a single grid -- and which one you were looking at
    // depended on where the origin happened to be in the frame. Detail is a
    // question about the zoom, and `grid_levels` already answers it: the fine
    // level fades in and out across the whole ground at once.
    if strength > 0.03 {
        push_grid_level(steps, view, fine, radius, strength, false, palette);
    }
    push_grid_level(steps, view, coarse, radius, 1.0, true, palette);
}

/// One decade of the ground grid, centred on the camera target so panning never
/// runs off the end of it. The centre is snapped to the level's own spacing, so
/// every line sits at a whole multiple of it -- which is what keeps the line
/// through zero *on* zero, and the X and Y axes lying along the grid rather
/// than across it.
pub(crate) fn push_grid_level(
    steps: &mut Vec<Step>,
    view: &View,
    spacing: f64,
    radius: f64,
    strength: f64,
    majors: bool,
    palette: &Palette,
) {
    let lines = ((radius / spacing).ceil() as i64).clamp(1, MAX_LINES);
    let half = spacing * lines as f64;
    let cx = (view.camera.target.x / spacing).round() * spacing;
    let cy = (view.camera.target.y / spacing).round() * spacing;
    // A major line every ten, counted in whole multiples of the spacing from
    // the world origin rather than from the centre, so which lines are major
    // stays put while the camera pans over them.
    let shade = |world: f64| {
        let index = (world / spacing).round() as i64;
        let colour = if majors && index.rem_euclid(10) == 0 { palette.grid_major } else { palette.grid };
        [colour[0], colour[1], colour[2], (colour[3] as f64 * strength).round() as u8]
    };
    for i in -lines..=lines {
        let offset = i as f64 * spacing;
        push_faded_line(
            steps,
            view,
            Vec3::new(cx + offset, cy - half, 0.0),
            Vec3::new(cx + offset, cy + half, 0.0),
            shade(cx + offset),
            GRID_BIAS,
        );
        push_faded_line(
            steps,
            view,
            Vec3::new(cx - half, cy + offset, 0.0),
            Vec3::new(cx + half, cy + offset, 0.0),
            shade(cy + offset),
            GRID_BIAS,
        );
    }
}

/// How many pieces a grid line is cut into to fade it. Enough that the steps
/// between one piece's alpha and the next are invisible, few enough that the
/// whole grid is still one pass of cheap segment drawing.
pub(crate) const FADE_STEPS: usize = 24;

/// Draw one grid line as a run of short segments whose alpha falls off with
/// distance from the grid's centre. A grid that simply stops leaves a hard
/// square edge in mid-air, and the eye reads that edge as part of the model.
/// The stretch of a world segment, as a parameter range inside `[0, 1]`, whose
/// projection lands in the frame -- `None` when none of it does.
///
/// The grid's lines run far outside the viewport, and at a shallow angle the
/// ground reaches several times the width of the frame. Subdividing the whole
/// segment would spend the fade's steps on the part nobody sees and leave two
/// or three of them for the part they do, which shows as banding across the
/// frame; finding the visible stretch first spends them all where they are
/// seen, and drops a line that misses the frame entirely before it costs
/// anything.
pub(crate) fn visible_span(view: &View, from: Vec3, to: Vec3) -> Option<(f64, f64)> {
    let a = view.view_to_screen(view.to_view(from)).0;
    let b = view.view_to_screen(view.to_view(to)).0;
    let (dx, dy) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let (width, height) = (view.size.x as f64, view.size.y as f64);
    let left = view.centre.x as f64 - width / 2.0;
    let top = view.centre.y as f64 - height / 2.0;
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    // Liang-Barsky against the frame, as the rasterizer does with pixels.
    for (edge, room) in [
        (-dx, a.x as f64 - left),
        (dx, left + width - a.x as f64),
        (-dy, a.y as f64 - top),
        (dy, top + height - a.y as f64),
    ] {
        if edge == 0.0 {
            if room < 0.0 {
                return None; // parallel to this edge and outside it
            }
        } else {
            let at = room / edge;
            if edge < 0.0 {
                if at > t1 {
                    return None;
                }
                t0 = t0.max(at);
            } else {
                if at < t0 {
                    return None;
                }
                t1 = t1.min(at);
            }
        }
    }
    (t1 > t0).then_some((t0, t1))
}

pub(crate) fn push_faded_line(steps: &mut Vec<Step>, view: &View, from: Vec3, to: Vec3, colour: Rgba, bias: f32) {
    let Some((visible_from, visible_to)) = visible_span(view, from, to) else { return };
    let half_diagonal = ((view.size.x as f64).hypot(view.size.y as f64) / 2.0).max(1.0);
    let span = visible_to - visible_from;
    for step in 0..FADE_STEPS {
        let t0 = visible_from + span * (step as f64 / FADE_STEPS as f64);
        let t1 = visible_from + span * ((step + 1) as f64 / FADE_STEPS as f64);
        let a = from + (to - from) * t0;
        let b = from + (to - from) * t1;
        let mid = (a + b) * 0.5;
        // Measured on the screen, not in the world. In the world it is a circle
        // about the origin, which the tilt of the ground turns into an ellipse
        // on the screen -- so the grid faded out before the top and bottom of
        // the viewport at every angle but straight down, however far it
        // reached. On the screen the falloff is the same in every direction and
        // the ground covers the frame at any tilt.
        let screen = view.view_to_screen(view.to_view(mid)).0;
        let distance = ((screen.x - view.centre.x) as f64).hypot((screen.y - view.centre.y) as f64);
        // Squared falloff: full strength in the middle of the frame, and gone
        // just past the corners rather than at them.
        let fade = 1.0 - (distance / (half_diagonal * 1.08)).min(1.0).powi(2);
        if fade <= 0.03 {
            continue;
        }
        let faded = [colour[0], colour[1], colour[2], (colour[3] as f64 * fade).round() as u8];
        // Never writes depth: the grid and the axes are drawn before the model
        // and must lose every tie with it, including the exact ties a ground
        // plane makes with a plate whose side walls it cuts.
        steps.push(line_step(view, a, b, faded, bias, 0, false));
    }
}
