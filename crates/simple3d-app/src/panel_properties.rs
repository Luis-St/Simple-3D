//! The property editor (spec section 7.3).
//!
//! Every field here is generated from the selected primitive's declaration in
//! the registry -- there is no per-primitive code. Values commit on Enter and on
//! leaving the field; unparseable text restores the previous value silently.
//!
//! The panel set follows the selection rather than greying out: with nothing
//! selected the dock shows the document's own settings, which is information
//! the user can actually act on, instead of a column of dead fields.

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

/// The label column of a property row. Fixed width, so every panel's fields
/// start at the same x and a column of numbers reads as a column.
const LABEL_WIDTH: f32 = 84.0;

/// The narrowest a row can be and still be worth splitting into a label column
/// and a field column: the column itself, plus enough beside it for a number
/// and its unit. Below this the row stacks instead (issue 51).
const STACK_BELOW: f32 = LABEL_WIDTH + 130.0;

/// The narrowest an axis field can be and still show a measurement. Three of
/// them side by side below this is three fields nobody can read, so they stack.
const MIN_AXIS_FIELD: f32 = 52.0;

/// The margin a section's frame keeps at its right-hand edge, which is the
/// edge every row in it has to stay inside.
const EDGE_PAD: f32 = 8.0;

/// The largest a document-wide distance -- the grid, the step -- may be set to,
/// in millimetres. A kilometre is already far past anything this prints, and a
/// field that can be dragged has to stop somewhere.
const MAX_LENGTH: f64 = 1e6;

/// The range a rotation step may take (issue 98). It divides an angle, so it
/// cannot be zero; and half a turn is the largest step that is still a step,
/// since a whole one puts everything back where it started.
const ROTATE_STEP: ParamKind = ParamKind::Angle { min: 0.1, max: 180.0 };

/// The range a segment count may take, whether it is the document's default or
/// one object's override: below three there is no curve to speak of, and above
/// five hundred the triangles are smaller than anything that prints.
const SEGMENTS: ParamKind = ParamKind::Count { min: 3, max: 512 };

/// One component of a point in space -- the 3D cursor, an end of the measure
/// span. A length with no floor, because half of space is behind the origin.
pub(crate) const POINT: ParamKind = ParamKind::Length { min: f64::NEG_INFINITY };

/// The widest one of the three fields on a point row may be.
///
/// What the numbers need: a place in space is signed and rarely round, where a
/// dimension is usually a number somebody typed. The widest of them is a view
/// centre at the far end of the camera's range, `-6248130.96`, and this leaves
/// that one padding rather than running it edge to edge. Held at the old 56
/// points these were the only fields in the panel whose text touched both sides
/// of the box, with a column of empty row beside them.
const POINT_FIELD_MAX: f32 = 104.0;
