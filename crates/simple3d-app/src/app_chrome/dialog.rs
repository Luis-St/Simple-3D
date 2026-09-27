//! A dialog window, either its own window or drawn inside the main one.

use super::*;
use crate::app::{App, Modal};
use crate::theme;

impl App {
    /// One of the application's dialogs as a real window (issue 53).
    ///
    /// `show_viewport_immediate` builds it in the same pass, so it can use app state directly (the
    /// deferred kind needs a `'static` callback). Backends without viewports, and headless tests, get
    /// `Embedded` and draw it in the main window. Centring and staying above the parent are requested
    /// by hand, since egui offers no transient-for or modal flag; Wayland ignores both.
    ///
    /// Buttons sit right-aligned in a panel along the foot under a rule (issue 63), so they never move
    /// or scroll away.
    pub(super) fn dialog(
        &mut self,
        ctx: &egui::Context,
        spec: DialogSpec<'_>,
        mut body: impl FnMut(&mut Self, &mut egui::Ui),
        mut actions: impl FnMut(&mut Self, &mut egui::Ui),
    ) {
        let DialogSpec { key, title, size, resizable, fit_height, min_size } = spec;
        let id = egui::ViewportId::from_hash_of(key);
        let mut builder = egui::ViewportBuilder::default()
            .with_title(title)
            .with_icon(crate::icon::shared_icon())
            .with_inner_size(size)
            .with_resizable(resizable)
            // Dialogs cannot be minimised out of sight while the app waits on them.
            .with_minimize_button(false)
            .with_maximize_button(resizable)
            .with_window_level(egui::WindowLevel::AlwaysOnTop);
        if let Some(min) = min_size {
            builder = builder.with_min_inner_size(min);
        }
        // Placed once on open; re-sending position and size every frame snapped it back and could make
        // the window manager resize it.
        if self.dialog_placed != Some(id) {
            if let Some(parent) = ctx.input(|i| i.viewport().outer_rect) {
                builder = builder.with_position(parent.center() - size * 0.5);
            }
            self.dialog_placed = Some(id);
        }
        ctx.show_viewport_immediate(id, builder, |ctx, class| {
            if class == egui::ViewportClass::Embedded {
                let mut open = true;
                let mut window = egui::Window::new(title)
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(resizable)
                    // Above the backdrop layer blocking the main window; the default `Middle` would be unclickable.
                    .order(egui::Order::Foreground)
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO);
                // Given its requested size: an auto-sized window has no content size to fit a body that lays
                // out against the room it gets (the pattern tool came out too narrow and dropped its preview).
                if resizable {
                    window = window.default_size(size);
                    if let Some(min) = min_size {
                        window = window.min_size(min);
                    }
                } else {
                    // Short forms auto-size their height; only the width needs saying.
                    window = window.default_width(size.x);
                }
                window.show(ctx, |ui| {
                    if resizable {
                        // The buttons are a foot panel taken out before the body, as in a real window; stacked under a
                        // filling body they grew the window every frame.
                        egui::TopBottomPanel::bottom("dialog-actions-embedded")
                            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                                left: 0,
                                right: 0,
                                top: 8,
                                bottom: 0,
                            }))
                            .exact_height(theme::metric::DIALOG_ACTIONS)
                            .show_separator_line(true)
                            .show_inside(ui, |ui| action_row(ui, |ui| actions(self, ui)));
                        egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(ui, |ui| body(self, ui));
                    } else {
                        // Content-height dialogs put the buttons under the body.
                        body(self, ui);
                        ui.separator();
                        action_row(ui, |ui| actions(self, ui));
                    }
                });
                if !open {
                    self.dismiss_modal();
                }
                return;
            }
            let pad = theme::metric::DIALOG_PAD;
            let footer = egui::Frame::NONE.fill(theme::token::SURFACE_1).inner_margin(egui::Margin {
                left: pad as i8,
                right: pad as i8,
                top: 8,
                bottom: 8,
            });
            egui::TopBottomPanel::bottom("dialog-actions")
                .frame(footer)
                .exact_height(theme::metric::DIALOG_ACTIONS)
                .show_separator_line(true)
                .show(ctx, |ui| action_row(ui, |ui| actions(self, ui)));
            let frame = egui::Frame::NONE.fill(theme::token::SURFACE_1).inner_margin(egui::Margin::same(pad as i8));
            let used = egui::CentralPanel::default()
                .frame(frame)
                .show(ctx, |ui| {
                    body(self, ui);
                    // The height the contents took, less the trailing item gap.
                    ui.cursor().top() - ui.max_rect().top() - ui.spacing().item_spacing.y
                })
                .inner;
            // Content-height dialogs ask for their measured height, so margins stay even.
            if fit_height {
                let want = (used + pad * 2.0 + theme::metric::DIALOG_ACTIONS).ceil();
                if (want - ctx.screen_rect().height()).abs() > 1.0 {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(size.x, want)));
                }
            }
            // The window's own close button cancels whatever dialog is open.
            if ctx.input(|i| i.viewport().close_requested()) {
                self.dismiss_modal();
            }
        });
    }

    /// Close whatever dialog is open, the way that dialog is cancelled.
    pub(crate) fn dismiss_modal(&mut self) {
        match self.modal {
            Modal::SavePrimitive => self.cancel_save_primitive(),
            Modal::ConfirmExtractAll => {
                self.confirm_extract = None;
                self.modal = Modal::None;
            }
            Modal::ConfirmDeleteKind => {
                self.confirm_delete_kind = None;
                self.modal = Modal::None;
            }
            Modal::ConfirmCloseTab => self.cancel_close_tab(),
            Modal::ConfirmComponent => self.cancel_component_ask(),
            Modal::Keymap => {
                self.recording = None;
                self.modal = Modal::None;
            }
            _ => self.modal = Modal::None,
        }
    }
}
