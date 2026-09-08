//! The window furniture: keyboard dispatch, menu bar, status bar and the modal
//! windows (export, keymap editor, about, errors, quit confirmation).

mod dialog;
mod dialog_parts;
mod ghosts;
mod menu;
mod menu_add_view;
mod menu_file_edit;
mod menu_manipulate;
mod shortcuts;
mod status_bar;
mod summary;
pub(crate) use dialog_parts::*;
mod confirm;
mod dialogs;
mod export_bodies;
mod export_dialog;
mod keymap_editor;
mod save_primitive;

use simple3d_core::scene::ExportBody;

/// What one export-body mark is called, in the picker's button and in its menu.
fn body_label(body: Option<ExportBody>) -> String {
    match body {
        None => "A body of its own".to_string(),
        Some(ExportBody::Shared(key)) => format!("Body {key}"),
        Some(ExportBody::Split) => "Split into its parts".to_string(),
    }
}
