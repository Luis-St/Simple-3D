//! One step of a drag: where the pointer has reached, and what that means
//! for the node.

use super::*;
use crate::view::View;
use simple3d_core::scene::Scene;
use simple3d_core::unit::{format_angle, format_length, wrap_degrees, Unit};

impl Drag {
    /// Apply the drag for the current cursor position. Called every frame, so the
    /// property editor tracks the handle live -- one source of truth, both
    /// directions (spec section 6.2).
    pub fn update(
        &mut self,
        scene: &mut Scene,
        view: &View,
        cursor: egui::Pos2,
        mods: Mods,
        move_snap: f64,
        rotate_snap: f64,
        unit: Unit,
    ) {
        // Everything below is measured in the frame the drag began in, never the
        // live one -- see the note on `Drag::gizmo`.
        let gizmo = &self.gizmo.clone();
        match self.handle {
            Handle::MoveAxis(axis) => {
                let Some(along) = view.ray_axis(cursor, gizmo.origin, gizmo.axes[axis]) else { return };
                let delta = mods.snap(along - self.grab, move_snap);
                let world_delta = gizmo.axes[axis] * delta;
                self.write_position(scene, world_delta);
                self.readout = format!("{} {}", axis_name(axis), signed_length(delta, unit));
            }
            Handle::MovePlane(axis) => {
                let Some(point) = view.ray_plane(cursor, gizmo.origin, gizmo.axes[axis]) else { return };
                let raw = point - self.grab_point;
                let (u, v) = other_axes(axis);
                let du = mods.snap(raw.dot(gizmo.axes[u]), move_snap);
                let dv = mods.snap(raw.dot(gizmo.axes[v]), move_snap);
                let world_delta = gizmo.axes[u] * du + gizmo.axes[v] * dv;
                self.write_position(scene, world_delta);
                self.readout = format!(
                    "{} {}  {} {}",
                    axis_name(u),
                    signed_length(du, unit),
                    axis_name(v),
                    signed_length(dv, unit)
                );
            }
            Handle::RotateRing(axis) => {
                let Some(angle) = ring_angle(gizmo, axis, view, cursor) else { return };
                // Unwrap across the seam so a full turn keeps counting up.
                let mut step = angle - self.last_angle;
                if step > 180.0 {
                    step -= 360.0;
                } else if step < -180.0 {
                    step += 360.0;
                }
                self.turns += step;
                self.last_angle = angle;
                let delta = mods.snap(self.turns, rotate_snap);
                let mut rotation = self.start_rotation;
                // Where the body ends up, which is a direction and so lives in
                // one turn (issue 84): the ring counts turns without end, but a
                // body two and a bit turns round stands where a body a bit round
                // stands, and that is what the rotation field reads.
                set_axis(&mut rotation, axis, wrap_degrees(get_axis(self.start_rotation, axis) + delta));
                if let Some(node) = scene.get_mut(self.node) {
                    node.rotation = rotation;
                }
                // Through the same wrap the field uses, and not merely stripped
                // of its whole turns: one rule for what an angle in this
                // application reads as. A field on a rotation cannot show a
                // negative, so a turn backwards is 345 here as well -- the
                // readout and the row it will land in say the same kind of
                // number, which they did not while this one kept its sign.
                self.readout = format!("{} {}deg", axis_name(axis), format_angle(wrap_degrees(delta)));
            }
            Handle::ResizeFace(axis, positive) => {
                let anchor = gizmo.own.point(gizmo.face_centre(axis, positive));
                let Some(along) = view.ray_axis(cursor, anchor, gizmo.axes[axis]) else { return };
                let outward = mods.snap(along - self.grab, move_snap) * if positive { 1.0 } else { -1.0 };
                let applied = self.size_axis(scene, axis, outward, mods.symmetric, positive);
                self.readout = match applied {
                    Some(extent) => format!("{} {}", axis_name(axis), format_length(extent, unit)),
                    None => "not resizable on this axis".to_string(),
                };
            }
            Handle::ResizeCorner(sides) => {
                let anchor = gizmo.own.point(gizmo.corner(sides));
                let Some(point) = view.ray_plane(cursor, anchor, view.forward()) else { return };
                let raw = point - self.grab_point;
                let mut ratio: Option<f64> = None;
                if mods.symmetric {
                    // Preserve proportions: take the axis that moved most, as a
                    // fraction of its own starting extent, and apply that same
                    // fraction to the others.
                    let mut best = 0.0;
                    for axis in 0..3 {
                        if !self.sizeable(axis) {
                            continue;
                        }
                        let extent = self.start_extent(axis);
                        if extent <= 0.0 {
                            continue;
                        }
                        let outward = raw.dot(gizmo.axes[axis]) * if sides[axis] { 1.0 } else { -1.0 };
                        if outward.abs() > best {
                            best = outward.abs();
                            ratio = Some((extent + outward) / extent);
                        }
                    }
                }
                let mut parts: Vec<String> = Vec::new();
                for axis in 0..3 {
                    if !self.sizeable(axis) {
                        continue;
                    }
                    let outward = match ratio {
                        Some(r) => self.start_extent(axis) * (r - 1.0),
                        None => mods.snap(raw.dot(gizmo.axes[axis]), move_snap) * if sides[axis] { 1.0 } else { -1.0 },
                    };
                    if let Some(extent) = self.size_axis(scene, axis, outward, false, sides[axis]) {
                        parts.push(format!("{} {}", axis_name(axis), format_length(extent, unit)));
                    }
                }
                self.readout = parts.join("  ");
            }
        }
    }
}
