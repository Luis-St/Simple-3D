//! Drawing the ground grid, faded out at its edge.

use super::*;
use crate::raster::Rgba;
use crate::view::View;
use simple3d_geom::Vec3;

pub(crate) fn push_grid(steps: &mut Vec<Step>, view: &View, grid: &Grid, palette: &Palette) {
    let (fine, coarse, strength) = grid_levels(view, grid.spacing);
    let radius = grid_radius(view);
    // Both levels cover the same ground; a shorter fine reach looked like a detail patch around the
    // origin. `grid_levels` fades the fine level across the whole ground.
    if strength > 0.03 {
        push_grid_level(steps, view, fine, radius, strength, false, palette);
    }
    push_grid_level(steps, view, coarse, radius, 1.0, true, palette);
}

/// One grid decade centred on the camera target, snapped to its spacing so lines sit on whole
/// multiples and the axes lie along the grid.
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
    let cx = (view.camera().target.x / spacing).round() * spacing;
    let cy = (view.camera().target.y / spacing).round() * spacing;
    // Every tenth line is major, counted from the world origin so majors stay put while panning.
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

/// How many pieces a grid line is cut into to fade it: invisible steps, still cheap.
pub(crate) const FADE_STEPS: usize = 24;

/// The parameter range in `[0, 1]` of a world segment whose projection lands in the frame, or
/// `None`. Fade steps are spent only on the visible stretch, avoiding banding, and missed lines
/// cost nothing.
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

/// One grid line as short segments fading with distance from the centre, since a hard edge in
/// mid-air reads as part of the model.
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
        // Measured on screen, not in the world, where tilt turns the circular falloff into an ellipse
        // that faded out before the viewport's top and bottom.
        let screen = view.view_to_screen(view.to_view(mid)).0;
        let distance = ((screen.x - view.centre.x) as f64).hypot((screen.y - view.centre.y) as f64);
        // Squared falloff: full strength mid-frame, gone just past the corners.
        let fade = 1.0 - (distance / (half_diagonal * 1.08)).min(1.0).powi(2);
        if fade <= 0.03 {
            continue;
        }
        let faded = [colour[0], colour[1], colour[2], (colour[3] as f64 * fade).round() as u8];
        // Never writes depth: grid and axes must lose every tie with the model, including exact ones.
        steps.push(line_step(view, a, b, faded, bias, 0, false));
    }
}
