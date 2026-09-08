//! The handles of the patterns that climb or wind.

use super::*;
use crate::primitive::{Params, ParamsExt};
use simple3d_geom::Vec3;

pub(crate) fn helix_grips(params: &Params) -> Vec<Grip> {
    let ax = params.int("helix_axis").min(2) as usize;
    let count = params.int("helix_count").max(1);
    let radius = params.num("helix_radius");
    let rise = params.num("helix_rise");
    let radial = unit(radial_axis(ax));
    let up = unit(ax);
    let mut out = vec![Grip::slide(
        "Radius",
        radial * radius,
        radial,
        Drive::Length { keys: &["helix_radius"], base: 0.0, per: 1.0, min: POSITIVE },
    )];
    // Both grips ride the axis rather than the helix itself: a grip on the last
    // copy would swing round the turn as the drag changed it, and chase the
    // pointer sideways while it was being pulled straight up.
    if count >= 2 {
        out.push(Grip::slide(
            "Rise",
            up * (rise * (count - 1) as f64),
            up,
            Drive::Length { keys: &["helix_rise"], base: 0.0, per: (count - 1) as f64, min: EITHER_WAY },
        ));
    }
    if rise.abs() > 1e-9 {
        out.push(Grip::slide(
            "Copies",
            up * (rise * count as f64),
            up,
            Drive::Count { key: "helix_count", base: 0.0, per: rise },
        ));
    }
    // The twist, taken hold of on the second copy: it is one turn from the
    // first, so the angle the grip is carried round to *is* the turn per copy,
    // and the rest of the helix follows behind it.
    if count >= 2 && radius.abs() > 1e-9 {
        let angle = params.num("helix_angle");
        out.push(Grip {
            label: "Turn per copy",
            at: turned(ax, radius, angle, rise).point(Vec3::ZERO),
            from: up * rise,
            dir: up,
            radius,
            drive: Drive::Angle { key: "helix_angle", axis: ax, per: 1.0 },
        });
    }
    out
}

/// A custom rule's handles: the run and the radius of every stage in use.
///
/// One stage's numbers are laid out exactly as a linear pattern's are, because
/// that is what a stage stepping along a line is. The turn is left as a number:
/// a stage may step *and* turn at once, and a handle riding a curve that its own
/// neighbour is also moving is one nobody can aim at.
pub(crate) fn custom_grips(params: &Params) -> Vec<Grip> {
    let mut out = Vec::new();
    for (index, k) in STAGES.iter().enumerate().take(stage_count(params)) {
        let stage = stage(params, index);
        if stage.mirror {
            // A mirror is a plane and two copies: no distance, nothing to drag.
            continue;
        }
        let length = stage.step.length();
        let dir = if length > 1e-9 { stage.step * (1.0 / length) } else { unit(0) };
        if length > 1e-9 && stage.count >= 2 {
            out.push(Grip::slide(
                k.grip_spacing,
                stage.step * (stage.count - 1) as f64,
                dir,
                Drive::Length { keys: &k.step, base: 0.0, per: (stage.count - 1) as f64, min: POSITIVE },
            ));
        }
        if length > 1e-9 {
            out.push(Grip::slide(
                k.grip_copies,
                dir * (length * stage.count as f64),
                dir,
                Drive::Count { key: k.count, base: 0.0, per: length },
            ));
        }
        if stage.radius.abs() > 1e-9 {
            let radial = unit(radial_axis(stage.axis));
            out.push(Grip::slide(
                k.grip_radius,
                radial * stage.radius,
                radial,
                Drive::Length { keys: std::slice::from_ref(&k.radius), base: 0.0, per: 1.0, min: POSITIVE },
            ));
        }
    }
    out
}

pub(crate) fn spiral_grips(params: &Params) -> Vec<Grip> {
    let ax = params.int("spiral_axis").min(2) as usize;
    let count = params.int("spiral_count").max(1);
    let start = params.num("spiral_radius");
    let growth = params.num("spiral_growth");
    let rise = params.num("spiral_rise");
    let radial = unit(radial_axis(ax));
    let up = unit(ax);
    // Three grips along one radial line, at the first copy's radius, the last
    // one's, and one copy beyond: a ruler out from the centre that says where
    // the spiral starts, how fast it opens and how far it goes.
    let mut out = vec![Grip::slide(
        "Start radius",
        radial * start,
        radial,
        Drive::Length { keys: &["spiral_radius"], base: 0.0, per: 1.0, min: POSITIVE },
    )];
    if count >= 2 {
        out.push(Grip::slide(
            "Radius per copy",
            radial * (start + growth * (count - 1) as f64),
            radial,
            Drive::Length { keys: &["spiral_growth"], base: start, per: (count - 1) as f64, min: EITHER_WAY },
        ));
    }
    if growth.abs() > 1e-9 {
        out.push(Grip::slide(
            "Copies",
            radial * (start + growth * count as f64),
            radial,
            Drive::Count { key: "spiral_count", base: start, per: growth },
        ));
    }
    if count >= 2 && rise.abs() > 1e-9 {
        out.push(Grip::slide(
            "Rise",
            up * (rise * (count - 1) as f64),
            up,
            Drive::Length { keys: &["spiral_rise"], base: 0.0, per: (count - 1) as f64, min: EITHER_WAY },
        ));
    }
    let second = start + growth;
    if count >= 2 && second.abs() > 1e-9 {
        let angle = params.num("spiral_angle");
        out.push(Grip {
            label: "Turn per copy",
            at: turned(ax, second, angle, rise).point(Vec3::ZERO),
            from: up * rise,
            dir: up,
            radius: second,
            drive: Drive::Angle { key: "spiral_angle", axis: ax, per: 1.0 },
        });
    }
    out
}
