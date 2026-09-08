//! The axes as they appear on screen, and what each is called.

use super::*;
use crate::view::View;
use simple3d_core::unit::{format_length, Unit};
use simple3d_geom::Vec3;

/// The cursor's angle around a ring, in degrees, measured from the ring's first
/// in-plane axis.
pub(crate) fn ring_angle(gizmo: &Gizmo, axis: usize, view: &View, cursor: egui::Pos2) -> Option<f64> {
    let point = view.ray_plane(cursor, gizmo.origin, gizmo.axes[axis])?;
    let d = point - gizmo.origin;
    let (u, v) = other_axes(axis);
    let (x, y) = (d.dot(gizmo.axes[u]), d.dot(gizmo.axes[v]));
    if x.hypot(y) < 1e-9 {
        return None;
    }
    Some(y.atan2(x).to_degrees())
}

pub fn axis_name(axis: usize) -> &'static str {
    ["X", "Y", "Z"][axis]
}

/// Axis colours, matching the origin axes so a handle's meaning is obvious.
pub fn axis_colour(axis: usize) -> egui::Color32 {
    match axis {
        0 => egui::Color32::from_rgb(226, 92, 92),
        1 => egui::Color32::from_rgb(112, 200, 112),
        _ => egui::Color32::from_rgb(104, 152, 245),
    }
}

pub(crate) fn signed_length(mm: f64, unit: Unit) -> String {
    let text = format_length(mm.abs(), unit);
    let sign = if mm < 0.0 { "-" } else { "+" };
    format!("{sign}{text}{}", unit.suffix())
}

/// Nudging with the keyboard (spec section 6.2): the arrow keys act along the two
/// axes most closely aligned with the screen, and a further pair handles the
/// third. Returns the handle-frame axis index for screen-right, screen-up and
/// the remaining axis.
pub fn screen_aligned_axes(gizmo: &Gizmo, view: &View) -> [usize; 3] {
    let (right, up) = view.basis();
    let mut remaining: Vec<usize> = vec![0, 1, 2];
    let pick = |remaining: &mut Vec<usize>, direction: Vec3| -> usize {
        let (best_index, _) = remaining
            .iter()
            .enumerate()
            .max_by(|(_, &a), (_, &b)| {
                let sa = gizmo.axes[a].dot(direction).abs();
                let sb = gizmo.axes[b].dot(direction).abs();
                sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("at least one axis remains");
        remaining.remove(best_index)
    };
    let horizontal = pick(&mut remaining, right);
    let vertical = pick(&mut remaining, up);
    [horizontal, vertical, remaining[0]]
}

/// Whether the given handle-frame axis points right/up on screen, so a nudge
/// "left" really goes left.
pub fn axis_screen_sign(gizmo: &Gizmo, view: &View, axis: usize, vertical: bool) -> f64 {
    let (right, up) = view.basis();
    let direction = if vertical { up } else { right };
    if gizmo.axes[axis].dot(direction) < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// The smallest extent a resize nudge will leave behind, so a shape cannot be
/// nudged to zero or inside out.
pub(crate) const MIN_EXTENT: f64 = 1e-3;
