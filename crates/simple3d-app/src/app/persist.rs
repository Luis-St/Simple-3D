//! Quitting, and writing the settings and keymap back out.

use super::*;
use simple3d_core::config::{self};
use std::path::Path;
use std::time::{Duration, Instant};

impl App {
    // -- shutdown -----------------------------------------------------------

    /// Quit every window, asking first if anything anywhere would be lost.
    pub fn request_quit(&mut self) {
        self.closing_window = false;
        if self.any_unsaved() || self.unsaved_elsewhere {
            self.modal = Modal::ConfirmQuit;
        } else {
            self.modal = Modal::None;
            self.quit_now = true;
        }
    }

    /// Close this window (issue 107), asking only about its own documents; with one window it is a quit.
    pub fn request_close_window(&mut self) {
        self.closing_window = true;
        if self.any_unsaved() {
            self.modal = Modal::ConfirmQuit;
        } else {
            self.modal = Modal::None;
            self.close_now = true;
        }
    }

    /// The answer to the question above, whichever was asked.
    pub fn confirm_quit(&mut self) {
        if self.closing_window {
            self.close_now = true;
        } else {
            self.quit_now = true;
        }
    }

    /// Whether the question is about closing one of several windows.
    pub(crate) fn closing_one_window(&self) -> bool {
        self.closing_window && !self.other_windows.is_empty()
    }

    /// Where this application reads and writes its settings and keymap.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn persist(&mut self) {
        let _ = config::save_settings_to(&self.config_dir, &self.settings);
        self.persisted_settings = self.settings.clone();
        self.settings_written = Some(Instant::now());
        self.persist_keymap();
    }

    /// Record the window's size and maximized state every frame so the next run matches (issue 95);
    /// the save-on-change below writes them. The size is taken only while unmaximized, since a
    /// maximized window reports the screen, and rounded to whole points to avoid drift writes.
    pub(super) fn record_window_shape(&mut self, ctx: &egui::Context) {
        let (size, maximized, fullscreen, minimized) = ctx.input(|i| {
            let viewport = i.viewport();
            (viewport.inner_rect.map(|rect| rect.size()), viewport.maximized, viewport.fullscreen, viewport.minimized)
        });
        // Only where the window system answers, so silence never un-maximizes.
        if let Some(maximized) = maximized {
            self.settings.window_maximized = maximized;
        }
        let ordinary = !maximized.unwrap_or(false) && !fullscreen.unwrap_or(false) && !minimized.unwrap_or(false);
        if let Some(size) = size.filter(|_| ordinary) {
            let size = [size.x.round(), size.y.round()];
            if size.iter().all(|n| n.is_finite() && *n >= 1.0) {
                self.settings.window_size = size;
            }
        }
    }

    /// Write settings as soon as they change, so a crash or kill does not lose them. Detected by
    /// comparison rather than calls at every change site, and rate-limited for scrubs, with a follow-up
    /// frame requested so the last value gets written.
    pub(super) fn persist_settings_if_changed(&mut self, ctx: &egui::Context) {
        const GAP: Duration = Duration::from_millis(250);
        if self.settings == self.persisted_settings {
            return;
        }
        match self.settings_written.map(|at| at.elapsed()) {
            Some(since) if since < GAP => ctx.request_repaint_after(GAP - since),
            _ => {
                let _ = config::save_settings_to(&self.config_dir, &self.settings);
                self.persisted_settings = self.settings.clone();
                self.settings_written = Some(Instant::now());
            }
        }
    }

    /// Write the keymap now, so a rebinding survives a hard kill (acceptance criterion 28).
    pub fn persist_keymap(&self) {
        let _ = config::save_keymap_to(&self.config_dir, &self.keymap);
    }
}
