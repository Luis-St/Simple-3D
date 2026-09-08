//! The orientation cube: its zones, and what each one means.

use super::*;
use simple3d_geom::Vec3;

/// The six faces of the orientation cube, as an outward normal and the view
/// each one gives. Driven from here so the cube and the View menu cannot come
/// to disagree about which way "front" is.
pub const CUBE_FACES: [([i32; 3], ViewPreset, &str); 6] = [
    ([1, 0, 0], ViewPreset::Right, "RGT"),
    ([-1, 0, 0], ViewPreset::Left, "LFT"),
    ([0, 1, 0], ViewPreset::Back, "BCK"),
    ([0, -1, 0], ViewPreset::Front, "FRT"),
    ([0, 0, 1], ViewPreset::Top, "TOP"),
    ([0, 0, -1], ViewPreset::Bottom, "BTM"),
];

/// Every place the orientation cube can be pointed at: its six face centres,
/// its twelve edge midpoints and its eight corners, as the cube-space vectors
/// that reach them. A component of 0 means "in the middle of that axis", so the
/// number of non-zero components says which of the three a zone is.
///
/// Faces alone were not enough: from the top there is no way to ask for the
/// front without going through the View menu, and a corner is how every other
/// 3D application offers the three-quarter views (issue 34).
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

/// How many of a zone's components are non-zero: 1 for a face, 2 for an edge,
/// 3 for a corner.
pub fn zone_order(zone: [i32; 3]) -> usize {
    zone.iter().filter(|c| **c != 0).count()
}

/// Which zone of the orientation cube a point `offset` from its centre falls
/// on: the nearest of the ones turned towards the eye. `None` when the point is
/// not on the cube at all.
///
/// Nearest-point rather than a hit test against the drawn quads, because the
/// cube is drawn from six flat faces and the zones are the nine regions of
/// each: the point on the cube closest to the pointer is the region the pointer
/// is in, and it says the same thing with a tenth of the geometry.
pub fn cube_zone_at(yaw_deg: f64, pitch_deg: f64, offset: egui::Vec2, reach: f32) -> Option<[i32; 3]> {
    let mut best: Option<([i32; 3], f32)> = None;
    for zone in cube_zones() {
        let v = Vec3::new(zone[0] as f64, zone[1] as f64, zone[2] as f64);
        let (at, depth) = cube_project(yaw_deg, pitch_deg, v, reach);
        // Facing away, so it is on the side of the cube that cannot be seen.
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

/// The view a zone asks for: the preset for one of the six faces, and the yaw
/// and pitch that put the eye on the zone's own direction otherwise.
///
/// `current_yaw` is used for the two poles, where every yaw looks the same and
/// turning to an arbitrary one would spin the model for no reason.
pub fn cube_zone_angles(zone: [i32; 3], current_yaw: f64) -> (f64, f64) {
    if let Some(preset) = cube_zone_preset(zone) {
        let (yaw, pitch) = preset.angles();
        return match preset {
            // Straight up or straight down: the yaw is not visible, so keep
            // the one the camera already has.
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

/// The named view a zone is, when it is one of the six faces.
pub fn cube_zone_preset(zone: [i32; 3]) -> Option<ViewPreset> {
    CUBE_FACES.iter().find(|(normal, _, _)| *normal == zone).map(|(_, preset, _)| *preset)
}

/// What a zone is called, for the status line: the preset's name for a face,
/// and the sides it lies between otherwise.
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

/// Project a unit-cube direction onto the orientation cube's face, given the
/// camera's yaw and pitch. Returns the offset from the cube's centre in points,
/// scaled by `reach`, and a depth that is negative towards the eye.
///
/// This is the cube's own little projection rather than the viewport's, because
/// the cube is always drawn the same size whatever the camera distance and
/// whatever the projection mode.
pub fn cube_project(yaw_deg: f64, pitch_deg: f64, v: Vec3, reach: f32) -> (egui::Vec2, f64) {
    // The viewport's own basis, written out: screen right, screen up and the
    // direction the camera looks. Deriving it from the same two angles is what
    // makes the cube unable to disagree with the view behind it -- and the
    // previous cube, which rotated its own way, disagreed with it by a quarter
    // turn.
    let (y, p) = (yaw_deg.to_radians(), pitch_deg.to_radians());
    let right = Vec3::new(-y.sin(), y.cos(), 0.0);
    let up = Vec3::new(-y.cos() * p.sin(), -y.sin() * p.sin(), p.cos());
    let forward = Vec3::new(-p.cos() * y.cos(), -p.cos() * y.sin(), -p.sin());
    // Screen y grows downward, so up is negated. Depth is negative towards the
    // eye, which is what makes a face visible.
    (egui::vec2(v.dot(right) as f32, -v.dot(up) as f32) * reach, v.dot(forward))
}
