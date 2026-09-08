//! Which feature the pointer is nearest to on screen.

use super::*;
use simple3d_geom::Vec3;

/// The nearest point on the segment `a`..`b` to `cursor`, measured on screen,
/// and how far away that landed (issue 78).
///
/// An edge is a line, not the three points on it the feature list carries, and
/// "measure from here along this edge" is an ordinary thing to want. The whole
/// segment is projected and the cursor is dropped onto it in screen space, so
/// what is caught is the place actually being pointed at. `None` when either end
/// falls off screen, where the projection cannot be trusted.
pub fn nearest_on_edge(
    a: Vec3,
    b: Vec3,
    project: impl Fn(Vec3) -> Option<egui::Pos2>,
    cursor: egui::Pos2,
    max_pixels: f32,
) -> Option<(Vec3, f32)> {
    let (sa, sb) = (project(a)?, project(b)?);
    let along = sb - sa;
    let length2 = along.length_sq();
    if length2 < 1e-9 {
        return None;
    }
    let t = ((cursor - sa).dot(along) / length2).clamp(0.0, 1.0);
    let distance = (sa + along * t - cursor).length();
    if distance > max_pixels {
        return None;
    }
    // The screen parameter is used in the model: a perspective divide would put
    // the point slightly off along the edge, but the viewport is orthographic
    // (issue 26), where the two parameters are the same number.
    Some((a + (b - a) * t as f64, distance))
}

/// Every feature within `max_pixels` of the cursor on screen, with how far away
/// each landed, in the order the list holds them.
///
/// The match is by screen distance, not world distance: what a user means by
/// "that corner" is the one under the pointer, and two corners far apart in the
/// model can sit close together in the frame. `project` returns `None` for a
/// point that does not land on screen, which is skipped.
///
/// All of them, rather than only the nearest, because a caller that will not
/// take every feature -- the measure tool, which takes only what the picture
/// shows -- has to work outward from the cursor until one is acceptable. Asking
/// that question of every feature instead costs a ray cast each.
pub fn near_on_screen(
    features: &[Feature],
    project: impl Fn(Vec3) -> Option<egui::Pos2>,
    cursor: egui::Pos2,
    max_pixels: f32,
) -> Vec<(&Feature, f32)> {
    let mut out = Vec::new();
    for feature in features {
        let Some(screen) = project(feature.point) else { continue };
        let distance = (screen - cursor).length();
        if distance <= max_pixels {
            out.push((feature, distance));
        }
    }
    out
}
