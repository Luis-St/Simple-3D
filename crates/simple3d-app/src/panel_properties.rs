//! The property editor (spec section 7.3).
//!
//! Fields are generated from the selected primitive's registry declaration, with no per-primitive
//! code. Values commit on Enter or leaving the field; unparseable text restores the previous value.
//! With nothing selected the dock shows the document's own settings rather than dead fields.

mod section;
pub(crate) use section::*;
mod row;
pub(crate) use row::*;
mod field;
pub use field::grip_id;
pub(crate) use field::*;
mod point;
pub(crate) use point::*;
mod document;
pub(crate) use document::*;
mod selection;
pub use selection::show_inside;
mod common;
pub(crate) use common::*;
mod pattern;
pub(crate) use pattern::*;
mod body;
pub(crate) use body::*;
mod face_edits;
pub(crate) use face_edits::*;
mod component;
pub(crate) use component::*;
mod pieces;
pub(crate) use pieces::*;
mod group;
pub(crate) use group::*;
mod primitive;
pub(crate) use primitive::*;
mod param_field;
pub(crate) use param_field::*;
mod placement;
pub(crate) use placement::*;
mod placement_rows;
pub(crate) use placement_rows::*;
pub use placement_rows::{rotate_step_row, step_row};
mod scrub;
pub(crate) use scrub::*;
mod measure;
pub(crate) use measure::*;
mod edit;
pub(crate) use edit::*;
#[cfg(test)]
mod tests;

use simple3d_core::primitive::ParamKind;

/// The label column's fixed width, so fields align across panels.
const LABEL_WIDTH: f32 = 84.0;

/// The narrowest a row can be and keep a label column beside its field; below this it stacks
/// (issue 51).
const STACK_BELOW: f32 = LABEL_WIDTH + 130.0;

/// The narrowest a readable axis field can be; below this three stack.
const MIN_AXIS_FIELD: f32 = 52.0;

/// The margin a section's frame keeps at its right edge, which rows stay inside.
const EDGE_PAD: f32 = 8.0;

/// The largest document-wide distance (grid, step) in millimetres; a drag must stop somewhere.
const MAX_LENGTH: f64 = 1e6;

/// The rotation step's range (issue 98): non-zero, since it divides, and at most half a turn.
const ROTATE_STEP: ParamKind = ParamKind::Angle { min: 0.1, max: 180.0, wrap: false };

/// The segment count range: below three there is no curve, above 512 nothing prints finer.
const SEGMENTS: ParamKind = ParamKind::Count { min: 3, max: 512 };

/// One component of a point in space, a length with no floor.
pub(crate) const POINT: ParamKind = ParamKind::Length { min: f64::NEG_INFINITY };

/// The widest a point field may be: room for a far-off signed view centre like `-6248130.96`
/// with padding.
const POINT_FIELD_MAX: f32 = 104.0;
