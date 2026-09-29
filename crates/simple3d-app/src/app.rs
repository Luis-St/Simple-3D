//! The application: layout, command dispatch, files, and keeping outliner, properties and viewport in
//! step (spec section 7).

mod state;
pub use state::{App, CubeSpin};
mod drag;
pub use drag::{Carried, DropTarget};
mod status;
pub use status::{status_opacity, Status, STATUS_LIFETIME};
mod modal;
pub use modal::Modal;
mod measure;
pub use measure::{Measure, MeasurePoint, Measurement};
mod pattern_grip;
pub use pattern_grip::PatternGrip;
mod snaps;
pub(crate) use snaps::*;
mod document;
mod files;
mod new;
mod selection;
mod snap_bodies;
pub(crate) use files::*;
mod add;
mod camera;
mod clipboard;
mod csg;
pub(crate) use csg::*;
mod commands;
mod csg_plan;
mod export;
mod features;
mod features_line;
mod insertion;
mod library;
mod manipulate;
mod order;
mod paint;
mod pattern;
mod pieces;
mod pieces_extract;
mod snap_apply;
pub(crate) use export::*;
mod hints;
mod import;
mod persist;
mod tool_popup;
pub use hints::insertion_hint;
pub(crate) use hints::*;
mod frame;
mod run_loop;
#[cfg(test)]
pub(crate) mod tests;

use std::time::Duration;

pub const APP_NAME: &str = "Simple 3D";

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const PROJECT_EXTENSION: &str = "simple3d";

/// An unfinished export by now has gone wrong; a clear message beats a hang (spec section 9).
pub const EXPORT_LIMIT: Duration = Duration::from_secs(120);

/// The same guard on import (issue 105); reading is quicker than exporting, so a read this long is a
/// bad file, not a large model.
pub const IMPORT_LIMIT: Duration = Duration::from_secs(120);
