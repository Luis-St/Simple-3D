//! Pattern nodes (issue 67): a node that repeats its children under a set of
//! transforms rather than as manual duplicates.
//!
//! A pattern holds children the way a group does, but it evaluates to copies of
//! them laid out by a rule -- a line, a grid, a ring, a mirror, a helix or a
//! spiral. Editing the original edits every copy, and the count is a number
//! rather than a hundred pasted nodes to keep in step.
//!
//! Its parameters ride in the same [`Params`] map a primitive uses, so the
//! property editor renders and validates them with no code of its own, the
//! project file and the clipboard carry them already, and undo covers them. The
//! kind is the first parameter -- a choice -- and every other parameter is shown
//! only for the kind it belongs to, exactly as a primitive hides "wall
//! thickness" behind its own choice.

mod keys;
pub(crate) use keys::*;
mod params;
pub use params::{default_params, MAX_INSTANCES, MAX_STAGES, PARAMS};
mod params_size;
pub use params_size::{migrate_params, param_visible, params_for_size};
mod stage_keys;
pub(crate) use stage_keys::*;
pub use stage_keys::{stage_count, stage_keys, StageKeys, STAGES};
mod stage;
pub use stage::{set_stage, stage, Stage};
mod instance;
pub(crate) use instance::*;
pub use instance::{instance_count, instances, radial_axis, Instance};
mod kinds;
pub(crate) use kinds::*;
mod custom;
pub(crate) use custom::*;
pub use custom::{custom_keys, Drive};
mod grip;
pub(crate) use grip::*;
pub use grip::{apply_grip, grip, grips, Grip};
mod grips_straight;
pub(crate) use grips_straight::*;
mod grips_curved;
pub(crate) use grips_curved::*;
#[cfg(test)]
mod tests;

/// The kinds of pattern, in the order they appear in the "kind" choice; the
/// index into this list is the value the choice parameter holds.
pub const KINDS: &[&str] = &["Linear", "Grid", "Circular", "Mirror", "Helix", "Spiral", "Custom"];

const LINEAR: u32 = 0;

const GRID: u32 = 1;

const CIRCULAR: u32 = 2;

const MIRROR: u32 = 3;

const HELIX: u32 = 4;

const SPIRAL: u32 = 5;

/// A rule the user built themselves, out of stages (issue 67).
pub const CUSTOM: u32 = 6;
