//! A dialog window, whether it is its own window or drawn inside the main
//! one.

use super::*;
use crate::app::{App, Modal};
use crate::theme;

impl App {
    /// One of the application's dialogs, as a window of the window system's own
    /// rather than a rectangle drawn over the viewport (issue 53).
    ///
    /// `show_viewport_immediate` opens a real top-level window and builds its
    /// contents in the same pass as the main one, so a dialog goes on reading
    /// and writing the application state directly; the deferred kind takes a
    /// `'static` callback and could not. A backend with no viewports at all --
    /// and the headless context the tests drive -- hands the body straight back
    /// as `Embedded`, and it is drawn inside the main window as it used to be.
    ///
    /// egui's `ViewportBuilder` exposes neither transient-for nor a modal flag,
    /// so the two things a dialog would otherwise inherit from its parent are
    /// asked for by hand: it opens over the middle of the parent and asks to
    /// stay above it. Wayland grants neither -- a client there does not place
    /// its own windows -- and ignores both without complaint.
    ///
    /// Every dialog is the same shape: the contents fill the window, and the
    /// buttons sit in a row of their own along the foot, right-aligned and all
    /// one size, under a rule (issue 63). The row is a panel rather than the
    /// last thing the body draws, so the buttons are in the same place in every
    /// dialog whatever the contents above them do -- and a body that scrolls can
    /// never push them off the bottom edge.
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
            // A dialog is not a window to put away and come back to: what is
            // fixed in size has nothing to maximise, and minimising one out of
            // sight while the application waits on it is a trap.
            .with_minimize_button(false)
            .with_maximize_button(resizable)
            .with_window_level(egui::WindowLevel::AlwaysOnTop);
        if let Some(min) = min_size {
            builder = builder.with_min_inner_size(min);
        }
        // Placed once, when the dialog opens, and never again. This body runs
        // on every frame of the parent's, and a builder that asks for a
        // position each time is a window that is put back where it started
        // whenever the parent repaints -- and, with the size asked for in the
        // same breath, one the window manager may resize under its own title
        // bar while the keyboard is somewhere else entirely.
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
                    // Above the backdrop that blocks the main window, which is
                    // a foreground layer created just before this one. A window
                    // left at the default `Middle` would be under it and could
                    // not be clicked at all.
                    .order(egui::Order::Foreground)
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO);
                // The size a dialog asks for is the size it gets here too. An
                // `egui::Window` given none sizes itself to its contents, and a
                // body that lays itself out against the room it is given has no
                // contents to be sized to: the pattern tool came out 362 px
                // wide, which is under what its two columns need, so it dropped
                // the picture and drew the numbers alone -- the preview
                // rendering nothing at all, in the one configuration
                // (`embed_dialogs`) that exists to keep the application off a
                // second window system surface.
                if resizable {
                    window = window.default_size(size);
                    if let Some(min) = min_size {
                        window = window.min_size(min);
                    }
                } else {
                    // The short forms are as tall as what is in them, which is
                    // what an auto-sized window already does and what
                    // `fit_height` asks the real window for. Only the width has
                    // to be said.
                    window = window.default_width(size.x);
                }
                window.show(ctx, |ui| {
                    if resizable {
                        // The same shape as the window of its own: the buttons
                        // are a panel along the foot, taken out of the room
                        // before the body is laid out. Stacked under a body
                        // that fills whatever it is given, they are pushed out
                        // of the window -- and the window, being sized to what
                        // is in it, grows by their height every frame until it
                        // is off the bottom of the screen.
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
                        // A dialog whose height is its contents' has no room to
                        // take the buttons out of: they go under the body, and
                        // the window is as tall as the two together.
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
                    // What the contents actually took, measured from the top of
                    // the room inside the margin to where the next thing would
                    // go -- less the gap that would be left before it.
                    ui.cursor().top() - ui.max_rect().top() - ui.spacing().item_spacing.y
                })
                .inner;
            // A dialog whose height is its contents' asks for the height they
            // came out at, so the last line sits the same distance above the
            // rule as the first does below the window's edge -- whatever the
            // font size, the display scale, or how long the paths it prints
            // turn out to be here.
            if fit_height {
                let want = (used + pad * 2.0 + theme::metric::DIALOG_ACTIONS).ceil();
                if (want - ctx.screen_rect().height()).abs() > 1.0 {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(size.x, want)));
                }
            }
            // The window's own close button, which no longer passes through any
            // code of ours: whatever the open dialog is, closing it cancels it.
            if ctx.input(|i| i.viewport().close_requested()) {
                self.dismiss_modal();
            }
        });
    }

    /// Close whatever dialog is open, the way that dialog is cancelled.
    pub(super) fn dismiss_modal(&mut self) {
        match self.modal {
            Modal::SavePrimitive => self.cancel_save_primitive(),
            Modal::ConfirmExtractAll => {
                self.confirm_extract = None;
                self.modal = Modal::None;
            }
            Modal::ConfirmCloseTab => self.cancel_close_tab(),
            Modal::Keymap => {
                self.recording = None;
                self.modal = Modal::None;
            }
            _ => self.modal = Modal::None,
        }
    }
}
