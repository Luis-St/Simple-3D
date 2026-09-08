//! The handles of the patterns laid out on straight lines and rings.

use super::*;
use crate::primitive::{Params, ParamsExt};
use simple3d_geom::Vec3;

pub(crate) fn linear_grips(params: &Params) -> Vec<Grip> {
    let count = params.int("count").max(1);
    let step = Vec3::new(params.num("step_x"), params.num("step_y"), params.num("step_z"));
    let length = step.length();
    let dir = if length > 1e-9 { step * (1.0 / length) } else { unit(0) };
    let mut out = Vec::new();
    if count >= 2 {
        out.push(Grip::slide(
            "Spacing",
            step * (count - 1) as f64,
            dir,
            Drive::Length { keys: LINEAR_RUN, base: 0.0, per: (count - 1) as f64, min: POSITIVE },
        ));
    }
    if length > 1e-9 {
        // One step past the last copy: drag it out and the run grows a copy at
        // a time at the spacing already set.
        out.push(Grip::slide(
            "Copies",
            dir * (length * count as f64),
            dir,
            Drive::Count { key: "count", base: 0.0, per: length },
        ));
    }
    out
}

pub(crate) fn grid_grips(params: &Params) -> Vec<Grip> {
    const COUNT_KEYS: [&str; 3] = ["grid_x", "grid_y", "grid_z"];
    const STEP_KEYS: [&[&str]; 3] = [&["grid_step_x"], &["grid_step_y"], &["grid_step_z"]];
    const SPACING: [&str; 3] = ["Column spacing", "Row spacing", "Layer spacing"];
    const HOW_MANY: [&str; 3] = ["Columns", "Rows", "Layers"];
    let mut out = Vec::new();
    for axis in 0..3 {
        let count = params.int(COUNT_KEYS[axis]).max(1);
        let step = params.num(STEP_KEYS[axis][0]);
        let dir = unit(axis);
        if count >= 2 {
            out.push(Grip::slide(
                SPACING[axis],
                dir * (step * (count - 1) as f64),
                dir,
                Drive::Length { keys: STEP_KEYS[axis], base: 0.0, per: (count - 1) as f64, min: EITHER_WAY },
            ));
        }
        if step.abs() > 1e-9 {
            out.push(Grip::slide(
                HOW_MANY[axis],
                dir * (step * count as f64),
                dir,
                Drive::Count { key: COUNT_KEYS[axis], base: 0.0, per: step },
            ));
        }
    }
    out
}

pub(crate) fn circular_grips(params: &Params) -> Vec<Grip> {
    let ax = params.int("circ_axis").min(2) as usize;
    let radius = params.num("circ_radius");
    let span = params.num("circ_span");
    let radial = unit(radial_axis(ax));
    let mut out = vec![Grip::slide(
        "Radius",
        radial * radius,
        radial,
        Drive::Length { keys: &["circ_radius"], base: 0.0, per: 1.0, min: POSITIVE },
    )];
    if radius.abs() > 1e-9 {
        // The span grip rides on a wider circle than the copies do. On the ring
        // itself a full turn would end where it began, on top of the radius
        // grip, and neither could be picked out from the other.
        let ring = radius * 1.3;
        out.push(Grip {
            label: "Span",
            at: turned(ax, ring, span, 0.0).point(Vec3::ZERO),
            from: Vec3::ZERO,
            dir: unit(ax),
            radius: ring,
            drive: Drive::Angle { key: "circ_span", axis: ax, per: 1.0 },
        });
    }
    // A ring has no outward run to drag copies along -- its copies fill the
    // span evenly however many there are -- so the span grip is what lays it
    // out and the count stays a number.
    out
}
