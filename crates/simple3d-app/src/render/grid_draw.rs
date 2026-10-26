//! Drawing the ground grid, or a workplane's (issue 125), faded out at its edge.

use super::*;
use crate::raster::Rgba;
use crate::view::View;
use simple3d_geom::Vec3;

/// The ground grid is drawn first and loses every tie, so the model hides it; a workplane's is drawn
/// after the model and beats the face it lies on, so it shows there and is hidden behind other bodies
/// (issue 125).
pub(crate) fn push_grid(steps: &mut Vec<Step>, view: &View, grid: &Grid, palette: &Palette) {
    let plane = grid.plane.unwrap_or_else(|| GridPlane::ground(grid.spacing));
    let bias = if grid.plane.is_some() { WORKPLANE_BIAS } else { GRID_BIAS };
    let (fine, coarse, strength) = grid_levels_on(view, &plane);
    let radius = grid_radius_on(view, &plane);
    // Both levels cover the same ground; a shorter fine reach looked like a detail patch around the
    // origin. `grid_levels` fades the fine level across the whole ground.
    if strength > 0.03 {
        push_grid_level(steps, view, &plane, (fine, bias), radius, strength, (false, (0.0, false)), palette);
    }
    let bold = plane.bold(fine, coarse, strength);
    push_grid_level(steps, view, &plane, (coarse, bias), radius, 1.0, (true, bold), palette);
}

/// One grid decade on `plane` centred on the camera target, snapped to its spacing so lines sit on
/// whole multiples and the axes lie along the grid.
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_grid_level(
    steps: &mut Vec<Step>,
    view: &View,
    plane: &GridPlane,
    (spacing, bias): (f64, f32),
    radius: f64,
    strength: f64,
    (majors, (bold, every_line)): (bool, (f64, bool)),
    palette: &Palette,
) {
    let ((cx, cy), half) = grid_square_on(view, plane, spacing, radius);
    let lines = (half / spacing).round() as i64;
    // Every `major_every`th line is major, counted from the plane's origin so majors stay put while
    // panning. `bold` makes the majors heavier, or every line when `every_line`.
    let every = plane.every() as i64;
    let major = |along: f64| majors && ((along / spacing).round() as i64).rem_euclid(every) == 0;
    let heavy = |along: f64| bold > 0.0 && (every_line || major(along));
    let shade = |along: f64| {
        let colour = match () {
            _ if major(along) => palette.grid_major,
            _ if heavy(along) => mix(palette.grid, palette.grid_major, bold),
            _ => palette.grid,
        };
        [colour[0], colour[1], colour[2], (colour[3] as f64 * strength).round() as u8]
    };
    // A heavy line is drawn twice, a pixel apart, the second as strong as it is heavy.
    let pixel = 1.0 / view.pixels_per_mm().max(1e-9);
    let twin = |colour: Rgba| [colour[0], colour[1], colour[2], (colour[3] as f64 * bold).round() as u8];
    for i in -lines..=lines {
        let offset = i as f64 * spacing;
        let (s, t) = (cx + offset, cy + offset);
        push_faded_line(steps, view, plane.at(s, cy - half), plane.at(s, cy + half), shade(s), bias);
        push_faded_line(steps, view, plane.at(cx - half, t), plane.at(cx + half, t), shade(t), bias);
        if heavy(s) {
            let (from, to) = (plane.at(s + pixel, cy - half), plane.at(s + pixel, cy + half));
            push_faded_line(steps, view, from, to, twin(shade(s)), bias);
        }
        if heavy(t) {
            let (from, to) = (plane.at(cx - half, t + pixel), plane.at(cx + half, t + pixel));
            push_faded_line(steps, view, from, to, twin(shade(t)), bias);
        }
    }
}

/// `a` turned toward `b` by `t`, from 0 to 1.
fn mix(a: Rgba, b: Rgba, t: f64) -> Rgba {
    std::array::from_fn(|i| (a[i] as f64 + (b[i] as f64 - a[i] as f64) * t.clamp(0.0, 1.0)).round() as u8)
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
