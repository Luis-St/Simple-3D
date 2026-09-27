//! The saved kinds as a dropdown's body (issue 67).
//!
//! Each row names a kind (applying it) with a cross that asks to delete that one; deleting used to
//! act on whatever the box showed, which meant applying it first. Shared by the tool's window and
//! the properties panel's Rule row so both behave the same.

use crate::theme;
use simple3d_core::pattern_library::Entry;

/// The width the cross and its gap take from a row.
const CROSS: f32 = 22.0;

/// What was asked of the list, returned since the dropdown body runs inside a closure holding the app.
#[derive(Clone, Debug)]
pub(crate) enum ShelfPick {
    /// Put this rule on the pattern.
    Apply(Entry),
    /// Ask before removing this one from the shelf.
    Delete(Entry),
}

/// The rows, for inside [`egui::ComboBox::show_ui`]; `chosen` is drawn selected and ignores clicks.
pub(crate) fn items(ui: &mut egui::Ui, entries: &[Entry], chosen: Option<&str>) -> Option<ShelfPick> {
    let mut picked = None;
    for entry in entries {
        let on = chosen == Some(entry.name.as_str());
        ui.horizontal(|ui| {
            if ui.selectable_label(on, &entry.name).clicked() && !on {
                picked = Some(ShelfPick::Apply(entry.clone()));
            }
            // Against the right edge, so the crosses line up.
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
