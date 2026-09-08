//! The text styles a panel uses.

use super::*;

/// A vertical scroll area for a list panel, with a bar that can be seen.
///
/// egui paints the scrollbar handle in the *widget* fill of the `Ui` the area
/// was shown in, which in this palette is one shade off the dock behind it --
/// close enough that a list holding more rows than fit looked complete, with
/// nothing to say the tree carried on below the fold. The handle colours are
/// raised here, on the container only: the style it returns is the one the
/// contents must be given back, so the rows inside keep the ordinary widget
/// colours.
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

/// A number. Tabular figures, so a column of them lines up and no digit shifts
/// width while it is being scrubbed -- the typographic tell that this
/// application is about measurement.
pub fn numeric(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).size(font::VALUE).monospace().color(token::TEXT_HI)
}

/// A quiet aside: a hint, a unit, a default nobody has overridden.
pub fn hint(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).size(font::SMALL).color(token::TEXT_LO)
}

/// A panel header, in the small-caps-with-tracking form the design calls for.
/// egui has no small caps, so the effect is made the way a typesetter without
/// the face would make it: upper case, one size down, letters spaced out.
pub fn header_text(name: &str) -> egui::RichText {
    let spaced: String = name.to_uppercase().chars().flat_map(|c| [c, '\u{2009}']).collect();
    egui::RichText::new(spaced.trim_end().to_string()).size(font::HEADER).color(token::TEXT_LO).strong()
}
