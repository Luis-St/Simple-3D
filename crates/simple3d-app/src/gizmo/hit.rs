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

    /// A plane handle's four corners on screen -- the origin, the end of its
    /// first axis, the far corner and the end of its second -- or `None` while
    /// the plane is seen too nearly edge-on to be a handle at all.
    ///
    /// Edge-on, the square collapses onto a line, and an outlined polygon of no
    /// area is one egui draws with mitre spikes thousands of pixels long: a
    /// line straight through the handle and off across the viewport, there at
    /// exactly one camera angle and gone the moment the camera turns. Grabbing
    /// such a handle would be no better, since the drag moves in a plane the
    /// eye cannot see into.
    pub fn plane_quad(&self, axis: usize, view: &View) -> Option<[egui::Pos2; 4]> {
        let (u, v) = other_axes(axis);
        let side = self.arm(view) * PLANE_FRACTION;
        let corners = [
            self.origin,
            self.origin + self.axes[u] * side,
            self.origin + (self.axes[u] + self.axes[v]) * side,
            self.origin + self.axes[v] * side,
        ];
        let mut quad = [egui::Pos2::ZERO; 4];
        for (screen, world) in quad.iter_mut().zip(corners) {
            *screen = view.project(world)?.0;
        }
        let twice_area: f32 = (0..4)
            .map(|i| {
                let (a, b) = (quad[i], quad[(i + 1) % 4]);
                a.x * b.y - b.x * a.y
            })
            .sum();
        let facing = (ARM_PIXELS * PLANE_FRACTION).powi(2) as f32;
        (twice_area.abs() * 0.5 >= facing * PLANE_MIN_FACING).then_some(quad)
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
                Handle::MovePlane(axis) if self.plane_quad(axis, view).is_none() => continue,
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
