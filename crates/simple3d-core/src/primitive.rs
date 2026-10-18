//! The declarative primitive registry (spec section 3.2). A primitive is one `PrimitiveSpec` in
//! `REGISTRY`; menus, editor, file format, clipboard and undo derive from it, with no per-primitive UI code.

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

const PRINTING: &str = "3D printing";

const MEASURE: &[&str] = &["Across corners", "Across flats"];

const CORNER_STYLE: &[&str] = &["Rounded", "Chamfered"];

const CHAMFER_EDGES: &[&str] = &["All edges", "Vertical edges", "Top and bottom"];

const WALL_MODE: &[&str] = &["Wall thickness", "Inner diameter"];

const SIZE_MODE: &[&str] = &["Circumscribed diameter", "Edge length"];

const SCREW_HEAD: &[&str] = &["Countersunk", "Counterbored", "Plain"];

const ROUND_EDGES: &[&str] = &["Vertical edges", "All edges", "Vertical and top edges"];

const CHAMFER_MODE: &[&str] = &["Same on every axis", "Per axis"];

const AXIS_XY: &[&str] = &["X", "Y"];

const TOP_PLACED: &[&str] = &["Centred", "To the minus side", "To the plus side", "Offset"];

const SPHERE_PART: &[&str] = &["Whole", "Upper half"];

const CAP_SIZE: &[&str] = &["Sphere diameter", "Base diameter"];

const TUBE_TOP: &[&str] = &["Same as bottom", "Own diameters"];

const CAPSULE_LENGTH: &[&str] = &["Total, with caps", "Straight part only"];

const TORUS_SIZE: &[&str] = &["Ring and tube", "Outer and inner"];

const PLATE_CORNERS: &[&str] = &["Same radius", "Each corner"];

const HEX_MEASURE: &[&str] = &["Across flats", "Across corners"];

const HEAD_SIDE: &[&str] = &["Top", "Bottom"];

const THREAD_FORM: &[&str] = &["ISO metric, 60 degrees", "Trapezoidal, 30 degrees", "Square", "Custom"];

const THREAD_USE: &[&str] = &["Bolt", "Nut cutter"];

const SPRING_COIL: &[&str] = &["Pitch", "Overall height"];

const SPRING_ENDS: &[&str] = &["Open", "Closed", "Closed and ground"];

const GEAR_TEETH: &[&str] = &["Straight", "Helical"];

const GEAR_PROFILE: &[&str] = &["Involute", "Cycloidal", "Circular arc"];

const NONE_OR_HUB: &[&str] = &["None", "Hub"];

const NONE_OR_KEYWAY: &[&str] = &["None", "Keyway"];
