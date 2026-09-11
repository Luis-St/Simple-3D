//! The saved kinds as the body of a dropdown (issue 67).
//!
//! One row per kind: the name, which puts that rule on the pattern, and a cross
//! on the end of it, which asks to delete that one. Both halves name the same
//! kind, which is the point -- deleting used to be a cross *outside* the
//! dropdown that acted on whatever the box happened to be showing, so removing
//! one meant picking it first, and picking it applied it to the pattern. A list
//! of things is where the delete for one of them belongs.
//!
//! Drawn from one place because the shelf is offered in two: the tool's own
//! window and the properties panel's Rule row. They are the same list and had
//! better behave the same way.

use crate::theme;
use simple3d_core::pattern_library::Entry;

/// The width the cross and the gap before it take out of a row.
const CROSS: f32 = 22.0;

/// What the user asked of the list. Returned rather than acted on, because a
/// dropdown's body is drawn inside a closure that is already holding the
/// application.
#[derive(Clone, Debug)]
pub(crate) enum ShelfPick {
    /// Put this rule on the pattern.
    Apply(Entry),
    /// Ask before taking this one off the shelf for good.
    Delete(Entry),
}

/// The rows themselves, for the inside of a [`egui::ComboBox::show_ui`].
///
/// `chosen` is the name the box is currently showing, so the row for it is
/// drawn as selected and clicking it again does nothing.
pub(crate) fn items(ui: &mut egui::Ui, entries: &[Entry], chosen: Option<&str>) -> Option<ShelfPick> {
    let mut picked = None;
    for entry in entries {
        let on = chosen == Some(entry.name.as_str());
        ui.horizontal(|ui| {
            if ui.selectable_label(on, &entry.name).clicked() && !on {
                picked = Some(ShelfPick::Apply(entry.clone()));
            }
            // Hard against the right edge, so a column of crosses lines up down
            // the list however long the names in front of them are.
            ui.add_space((ui.available_width() - CROSS).max(0.0));
            if ui
                .add(egui::Button::new(theme::hint("\u{00d7}")).small())
                .on_hover_text(format!("Delete \u{201C}{}\u{201D} from the shelf", entry.name))
                .clicked()
            {
                picked = Some(ShelfPick::Delete(entry.clone()));
            }
        });
    }
    picked
}
