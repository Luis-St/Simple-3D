//! The text styles a panel uses.

use super::*;

/// A vertical scroll area for a list panel, with a visible bar. egui paints the handle in the
/// widget fill, one shade off the dock, so overflowing lists looked complete. The colours are raised
/// on the container only; give the returned style back to the contents.
///
/// ```ignore
/// let (area, restore) = theme::list_scroll_area(ui);
/// area.show(ui, |ui| {
///     ui.set_style(restore);
///     // rows
/// });
/// ```
pub fn list_scroll_area(ui: &mut egui::Ui) -> (egui::ScrollArea, std::sync::Arc<egui::Style>) {
    let restore = ui.style().clone();
    let widgets = &mut ui.style_mut().visuals.widgets;
    widgets.inactive.bg_fill = token::SCROLL_HANDLE;
    widgets.hovered.bg_fill = token::SCROLL_HANDLE_HOVER;
    widgets.active.bg_fill = token::TEXT_LO;
    (egui::ScrollArea::vertical().auto_shrink([false, false]), restore)
}

/// A value: a name, a measurement, anything the user typed or the model owns.
pub fn value(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).size(font::VALUE).color(token::TEXT_HI)
}

/// A number in tabular figures, so columns align and digits do not shift while scrubbing.
pub fn numeric(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).size(font::VALUE).monospace().color(token::TEXT_HI)
}

/// A quiet aside: a hint, a unit, an untouched default.
pub fn hint(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).size(font::SMALL).color(token::TEXT_LO)
}

/// A panel header in faux small caps (egui has none): upper case, one size down, letter-spaced.
pub fn header_text(name: &str) -> egui::RichText {
    let spaced: String = name.to_uppercase().chars().flat_map(|c| [c, '\u{2009}']).collect();
    egui::RichText::new(spaced.trim_end().to_string()).size(font::HEADER).color(token::TEXT_LO).strong()
}
