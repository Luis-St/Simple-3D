//! An internal (ring) gear (issue 123): teeth on the inside of a rim, for planetary gearboxes. Its tooth
//! gaps are the teeth of an outer gear of the same count, reaching a little further for clearance.

use super::body::{insert_directions, turned};
use super::{tooth, GearSpec};
use crate::mesh::Mesh;
use crate::revolve::grid_mesh;
use std::f64::consts::{FRAC_PI_2, PI};

impl GearSpec {
    /// How far out an internal gear's gaps reach: the outer gear's tips, a quarter module further so a
    /// meshing gear's tips clear them.
    pub fn internal_root_radius(&self) -> f64 {
        self.tip_radius() + 0.25 * self.module
    }

    /// The narrowest rim an internal gear is built with: a module outside its gaps.
    pub fn internal_rim_radius(&self) -> f64 {
        self.internal_root_radius() + self.module
    }
}

/// The internal gear `spec` describes, its rim `outer_diameter` across but at least a module outside the
/// gaps, centred on its thickness with a gap along +X. The bore, hub and keyway play no part.
pub fn internal_gear_mesh(spec: &GearSpec, outer_diameter: f64) -> Mesh {
    let gaps = spec.internal_root_radius();
    // The teeth end a module inside the pitch circle, where a meshing gear's roots leave them room.
    let tips = (spec.pitch_radius() - spec.module).clamp(gaps * 0.05, gaps * 0.999);
    // The outer gear's flanks, run on radially for the clearance: a pointed tooth carried on would be a
    // sliver.
    let mut flank = spec.flank(tips, spec.tip_radius());
    let (_, last) = flank[flank.len() - 1];
    flank.push((gaps, last));
    let mut toothed = tooth::outline(&flank, spec.teeth.max(3), tips, gaps);
    // Points on the axes, so the rim paired with them reaches its diameter there.
    insert_directions(&mut toothed, &[0.0, FRAC_PI_2, PI, 3.0 * FRAC_PI_2]);
    let radius = (outer_diameter / 2.0).max(spec.internal_rim_radius());
    let rim: Vec<(f64, f64)> = toothed
        .iter()
        .map(|&(x, y)| {
            let a = y.atan2(x);
            (radius * a.cos(), radius * a.sin())
        })
        .collect();
    let body = spec.body();
    let bottom = -spec.thickness / 2.0;
    let top_turn = body.turn_at(spec.thickness);
    let mut rings = vec![turned(&rim, 0.0, bottom), turned(&rim, top_turn, bottom + spec.thickness)];
    rings.extend(body.levels(&toothed, bottom).into_iter().rev());
    grid_mesh(&rings, true)
}
