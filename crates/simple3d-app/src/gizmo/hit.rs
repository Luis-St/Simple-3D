//! Which handle the pointer is over.

use super::*;
use crate::view::View;
use simple3d_geom::Vec3;

impl Gizmo {
    /// Points along a rotate ring, for drawing it and for hit-testing it.
    pub fn ring_points(&self, axis: usize, view: &View, count: usize) -> Vec<Vec3> {
        let radius = self.arm(view) * 0.86;
        let (u, v) = other_axes(axis);
        (0..count)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / count as f64;
                self.origin + self.axes[u] * (radius * t.cos()) + self.axes[v] * (radius * t.sin())
            })
            .collect()
    }

    /// The handle under the cursor, if any.
    pub fn hit_test(&self, view: &View, cursor: egui::Pos2, is_group: bool) -> Option<Handle> {
        let mut best: Option<(f32, Handle)> = None;
        for handle in self.handles(is_group) {
            let distance = match handle {
                Handle::RotateRing(axis) => {
                    let points = self.ring_points(axis, view, 72);
                    let mut nearest = f32::MAX;
                    for pair in
                        points.windows(2).chain(std::iter::once([*points.last().unwrap(), points[0]].as_slice()))
                    {
                        let (Some((a, _)), Some((b, _))) = (view.project(pair[0]), view.project(pair[1])) else {
                            continue;
                        };
                        nearest = nearest.min(distance_to_segment(cursor, a, b));
                    }
                    nearest
                }
                other => match view.project(self.handle_point(other, view)) {
                    Some((screen, _)) => (screen - cursor).length(),
                    None => continue,
                },
            };
            if distance <= GRAB_PIXELS && best.is_none_or(|(d, _)| distance < d) {
                best = Some((distance, handle));
            }
        }
        best.map(|(_, handle)| handle)
    }
}

pub const CORNERS: [[bool; 3]; 8] = [
    [false, false, false],
    [true, false, false],
    [false, true, false],
    [true, true, false],
    [false, false, true],
    [true, false, true],
    [false, true, true],
    [true, true, true],
];

pub(crate) fn other_axes(axis: usize) -> (usize, usize) {
    match axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    }
}

pub fn get_axis(v: Vec3, axis: usize) -> f64 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

pub fn set_axis(v: &mut Vec3, axis: usize, value: f64) {
    match axis {
        0 => v.x = value,
        1 => v.y = value,
        _ => v.z = value,
    }
}

pub(crate) fn distance_to_segment(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 1e-9 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}
