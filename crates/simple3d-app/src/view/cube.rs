//! The orientation cube: its zones, and what each one means.

use super::*;
use simple3d_geom::Vec3;

/// The cube's six faces as outward normals and views; shared with the View menu so they agree.
pub const CUBE_FACES: [([i32; 3], ViewPreset, &str); 6] = [
    ([1, 0, 0], ViewPreset::Right, "RGT"),
    ([-1, 0, 0], ViewPreset::Left, "LFT"),
    ([0, 1, 0], ViewPreset::Back, "BCK"),
    ([0, -1, 0], ViewPreset::Front, "FRT"),
    ([0, 0, 1], ViewPreset::Top, "TOP"),
    ([0, 0, -1], ViewPreset::Bottom, "BTM"),
];

/// Every clickable zone of the cube (6 faces, 12 edges, 8 corners) as cube-space vectors; the count
/// of non-zero components gives the zone type. Edges and corners give three-quarter views (issue 34).
pub const fn cube_zones() -> [[i32; 3]; 26] {
    let mut out = [[0i32; 3]; 26];
    let mut count = 0;
    let mut x = -1;
    while x <= 1 {
        let mut y = -1;
        while y <= 1 {
            let mut z = -1;
            while z <= 1 {
                if !(x == 0 && y == 0 && z == 0) {
                    out[count] = [x, y, z];
                    count += 1;
                }
                z += 1;
            }
            y += 1;
        }
        x += 1;
    }
    out
}

/// Non-zero components of a zone: 1 face, 2 edge, 3 corner.
pub fn zone_order(zone: [i32; 3]) -> usize {
    zone.iter().filter(|c| **c != 0).count()
}

/// The zone at `offset` from the cube's centre: the nearest facing the eye, or `None` off the cube.
/// Nearest-point rather than a quad hit test, which gives the same regions far more cheaply.
pub fn cube_zone_at(yaw_deg: f64, pitch_deg: f64, offset: egui::Vec2, reach: f32) -> Option<[i32; 3]> {
    let mut best: Option<([i32; 3], f32)> = None;
    for zone in cube_zones() {
        let v = Vec3::new(zone[0] as f64, zone[1] as f64, zone[2] as f64);
        let (at, depth) = cube_project(yaw_deg, pitch_deg, v, reach);
        // Facing away, on the unseen side.
        if depth >= 0.0 {
            continue;
        }
        let distance = (offset - at).length();
        if distance > reach * 0.85 {
            continue;
        }
        if best.is_none_or(|(_, d)| distance < d) {
            best = Some((zone, distance));
        }
    }
    best.map(|(zone, _)| zone)
}

/// The view a zone asks for: a face's preset, or the angles towards the zone. `current_yaw` is
/// kept at the poles, where yaw is invisible.
pub fn cube_zone_angles(zone: [i32; 3], current_yaw: f64) -> (f64, f64) {
    if let Some(preset) = cube_zone_preset(zone) {
        let (yaw, pitch) = preset.angles();
        return match preset {
            // Straight up or down: keep the camera's yaw.
            ViewPreset::Top | ViewPreset::Bottom => (current_yaw, pitch),
            _ => (yaw, pitch),
        };
    }
    let v = Vec3::new(zone[0] as f64, zone[1] as f64, zone[2] as f64);
    let length = v.length().max(1e-9);
    let pitch = (v.z / length).asin().to_degrees().clamp(-89.9, 89.9);
    let yaw = v.y.atan2(v.x).to_degrees();
    (yaw, pitch)
}

/// The preset a zone is, if it is a face.
pub fn cube_zone_preset(zone: [i32; 3]) -> Option<ViewPreset> {
    CUBE_FACES.iter().find(|(normal, _, _)| *normal == zone).map(|(_, preset, _)| *preset)
}

/// A zone's name for the status line: the preset's for a face, else the sides it lies between.
pub fn cube_zone_label(zone: [i32; 3]) -> String {
    if let Some(preset) = cube_zone_preset(zone) {
        return preset.label().to_string();
    }
    let names = [(0, 1, "right"), (0, -1, "left"), (1, 1, "back"), (1, -1, "front"), (2, 1, "top"), (2, -1, "bottom")];
    let parts: Vec<&str> =
        names.iter().filter(|(axis, sign, _)| zone[*axis] == *sign).map(|(_, _, name)| *name).collect();
    let what = if zone_order(zone) == 3 { "corner" } else { "edge" };
    format!("{} {what}", parts.join("-"))
}

/// Project a unit-cube direction for the cube widget: screen offset scaled by `reach`, and depth
/// (negative towards the eye). Its own projection, since the cube's size is fixed.
pub fn cube_project(yaw_deg: f64, pitch_deg: f64, v: Vec3, reach: f32) -> (egui::Vec2, f64) {
    // The viewport's basis from the same two angles, so the cube cannot disagree with the view (the
    // previous one was a quarter turn off).
    let (y, p) = (yaw_deg.to_radians(), pitch_deg.to_radians());
    let right = Vec3::new(-y.sin(), y.cos(), 0.0);
    let up = Vec3::new(-y.cos() * p.sin(), -y.sin() * p.sin(), p.cos());
    let forward = Vec3::new(-p.cos() * y.cos(), -p.cos() * y.sin(), -p.sin());
    // Screen y grows downward, so up is negated; negative depth faces the eye.
    (egui::vec2(v.dot(right) as f32, -v.dot(up) as f32) * reach, v.dot(forward))
}
