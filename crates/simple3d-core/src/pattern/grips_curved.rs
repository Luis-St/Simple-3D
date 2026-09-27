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
    // Height grips ride the axis, since one on the last copy would swing round as the drag changed it.
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
    // The twist grip sits on the second copy, one turn from the first, so its angle is the turn per copy.
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

/// A custom rule's handles: each moving stage's run, laid out like a linear pattern's, and each
/// turning stage's radius. Turns stay numbers, and mirrors have nothing to drag.
pub(crate) fn custom_grips(params: &Params) -> Vec<Grip> {
    let mut out = Vec::new();
    for (index, k) in STAGES.iter().enumerate().take(stage_count(params)) {
        let stage = stage(params, index);
        match stage.mode {
            StageMode::Mirror => continue,
            StageMode::Turn => {
                if stage.radius.abs() > 1e-9 {
                    let radial = unit(radial_axis(stage.axis));
                    out.push(Grip::slide(
                        k.grip_radius,
                        radial * stage.radius,
                        radial,
                        Drive::Length { keys: std::slice::from_ref(&k.radius), base: 0.0, per: 1.0, min: POSITIVE },
                    ));
                }
                continue;
            }
            StageMode::Move => {}
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
    // Three grips along one radial line: start radius, last copy's radius, and one beyond.
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
