//! The application: window layout, command dispatch, file handling and the
//! glue that keeps the outliner, property editor and viewport in step
//! (spec section 7).

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
pub use snaps::BodySnaps;
pub(crate) use snaps::*;
mod document;
mod files;
mod new;
mod selection;
pub(crate) use files::*;
mod add;
mod camera;
mod clipboard;
mod commands;
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
pub use export::ExportSummary;
pub(crate) use export::*;
mod hints;
mod persist;
pub use hints::insertion_hint;
pub(crate) use hints::*;
mod frame;
mod run_loop;
#[cfg(test)]
mod tests;

use std::time::Duration;

pub const APP_NAME: &str = "Simple 3D";

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const PROJECT_EXTENSION: &str = "simple3d";

/// An export that has not finished by now has gone wrong; better a clear message
/// than an indefinite hang (spec section 9).
pub const EXPORT_LIMIT: Duration = Duration::from_secs(120);
