//! A sketch point: where curves start, end and are centred.

use serde::{Deserialize, Serialize};

pub type PointId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub id: PointId,
    /// Where it is, in the sketch's own plane, in millimetres.
    pub at: [f64; 2],
    /// Whether the solver may not move it: the points of edges copied in from the model, which stand
    /// for geometry outside the sketch. Omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub fixed: bool,
}

pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}
