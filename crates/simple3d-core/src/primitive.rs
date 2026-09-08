//! The declarative primitive registry (spec section 3.2, "Extensibility").
//!
//! Adding a primitive means adding one `PrimitiveSpec` to `REGISTRY` and
//! nothing else. The Add menu, the property editor, the project file format,
//! the clipboard and undo all derive themselves from these declarations -- there
//! is deliberately no per-primitive user-interface code anywhere in the app.

mod param;
mod registry;
pub use param::{ParamKind, ParamSpec, ParamValue};
mod params;
pub use params::{Params, ParamsExt};
mod spec;
pub use spec::{categories, lookup, AxisDriver, PrimitiveSpec};
mod drivers;
pub(crate) use drivers::*;
#[cfg(test)]
mod tests;

pub use registry::REGISTRY;

const BOXES: &str = "Boxes and prisms";

const ROUND: &str = "Round solids";

const CONES: &str = "Cones and pyramids";

const POLY: &str = "Regular polyhedra";

const FLAT: &str = "Flat shapes";

const MEASURE: &[&str] = &["Across corners", "Across flats"];

const CORNER_STYLE: &[&str] = &["Rounded", "Chamfered"];

const CHAMFER_EDGES: &[&str] = &["All edges", "Vertical edges", "Top and bottom"];

const WALL_MODE: &[&str] = &["Wall thickness", "Inner diameter"];

const SIZE_MODE: &[&str] = &["Circumscribed diameter", "Edge length"];
