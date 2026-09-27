//! The frame every tool's in-place popup shares.

use super::*;
use crate::popup::{self, PopupEvent, PopupSpec};

impl App {
    /// Draw a tool's popup over the viewport where it was left: `body` scrolls on short viewports so
    /// the `actions` row stays reachable.
    pub(crate) fn tool_popup(
        &mut self,
        ctx: &egui::Context,
        spec: PopupSpec<'_>,
        body: impl FnOnce(&mut App, &mut egui::Ui),
        actions: impl FnOnce(&mut App, &mut egui::Ui),
    ) -> PopupEvent {
        let bounds = self.viewport_rect;
        let key = spec.key;
        // Taken out of the map so the popup can hold it mutably while the contents hold the app.
        let mut placement = self.popups.remove(key).unwrap_or_default();
        let event = popup::show(ctx, bounds, &mut placement, spec, |ui| {
            popup::scrolling_body(ui, bounds, |ui| body(self, ui));
            popup::action_row(ui, |ui| actions(self, ui));
        });
        self.popups.insert(key, placement);
        event
    }
}
