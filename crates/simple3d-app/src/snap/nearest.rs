//! Which feature the pointer is nearest to on screen.

use super::*;
use simple3d_geom::Vec3;

/// The nearest point on segment `a`..`b` to `cursor` on screen, and its distance (issue 78), so an
/// edge is caught where it is pointed at. `None` if either end is off screen.
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
    // The screen parameter equals the world one under the orthographic viewport (issue 26).
    Some((a + (b - a) * t as f64, distance))
}

/// Every feature within `max_pixels` of the cursor on screen, with distances, in list order. By
/// screen distance, since "that corner" is the one under the pointer. All of them, so callers
/// rejecting some (hidden ones) can work outward cheaply.
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
