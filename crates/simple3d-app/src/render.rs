//! Turning the evaluated scene into the viewport image (spec section 6.1):
//! shaded / shaded-with-edges / wireframe display, the ground grid and origin
//! axes, the selection highlight and translucent ghosts for hidden nodes.

mod renderable;
pub use renderable::Renderable;
mod palette;
pub use palette::Palette;
pub(crate) use palette::*;
mod request;
pub(crate) use request::*;
pub use request::{Grid, Item, Request, Style};
mod frame;
pub use frame::{prepare_frame, render_prepared, Prepared};
mod step;
pub(crate) use step::*;
mod shading;
pub(crate) use shading::*;
mod push_solid;
pub(crate) use push_solid::*;
mod bias;
pub(crate) use bias::*;
mod push_marks;
pub(crate) use push_marks::*;
mod push_selection;
pub(crate) use push_selection::*;
mod grid_extent;
pub(crate) use grid_extent::*;
pub use grid_extent::{effective_grid_spacing, frame_reach, grid_levels, grid_radius};
mod grid_draw;
pub(crate) use grid_draw::*;
mod axis_material;
pub(crate) use axis_material::*;
mod axis_draw;
pub(crate) use axis_draw::*;
mod axis_spans;
pub(crate) use axis_spans::*;
mod axis_prepare;
pub(crate) use axis_prepare::*;
mod plane_marks;
pub(crate) use plane_marks::*;
#[cfg(test)]
mod tests;
