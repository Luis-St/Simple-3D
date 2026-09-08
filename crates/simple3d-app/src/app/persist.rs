//! Quitting, and writing the settings and keymap back out.

use super::*;
use simple3d_core::config::{self};
use std::path::Path;
use std::time::{Duration, Instant};

impl App {
    // -- shutdown -----------------------------------------------------------

    pub fn request_quit(&mut self) {
        if self.any_unsaved() {
            self.modal = Modal::ConfirmQuit;
        } else {
            self.modal = Modal::None;
            self.quit_now = true;
        }
    }

    pub fn confirm_quit(&mut self) {
        self.quit_now = true;
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

    /// Remember how big the window is and whether it is maximized, so the next
    /// run opens the way this one was left (issue 95).
    ///
    /// Nothing ever wrote these two. `main` reads `window_size` and
    /// `window_maximized` out of the settings to build the window, and no code
    /// path put a new value back -- so however the window was left, every run
    /// opened at the 1400 x 880 default. They are read off the window itself
    /// here, on every frame, and the ordinary save-on-change below writes them
    /// out; sampling them in `on_exit` instead would lose them to exactly the
    /// endings that setting has already been fixed for.
    ///
    /// The size is only taken while the window is in its ordinary state. What a
    /// maximized or fullscreen window reports is the screen, and restoring the
    /// screen as the *unmaximized* size is how a window comes back filling the
    /// display with no way back to the size it used to have. Rounded to whole
    /// points because a resize otherwise writes the file for a fraction of a
    /// pixel of drift.
    pub(super) fn record_window_shape(&mut self, ctx: &egui::Context) {
        let (size, maximized, fullscreen, minimized) = ctx.input(|i| {
            let viewport = i.viewport();
            (viewport.inner_rect.map(|rect| rect.size()), viewport.maximized, viewport.fullscreen, viewport.minimized)
        });
        // Only where the window system answers at all: a platform that reports
        // nothing must not reset a maximized window to "not maximized".
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

    /// Write the settings out as soon as they change, rather than only when the
    /// application is closed cleanly.
    ///
    /// A setting changed in the property panel -- where a shape lands, the snap
    /// mode, a snap step -- used to live in memory until `on_exit` ran, so
    /// anything that ended the process another way took it with it: a crash, a
    /// kill, a power cut. The keymap has been written on the spot since
    /// acceptance criterion 28 asked for a rebinding to survive a hard kill, and
    /// there is no reason the rest of the settings deserve less.
    ///
    /// Noticed by comparing rather than by calling `persist` from each of the
    /// dozen places that change something, because that is a list nobody keeps
    /// complete. Rate-limited because a scrubbed number changes every frame,
    /// and the frame that the limit turned away asks for one more frame so the
    /// value is not left unwritten until something else happens to repaint.
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

    /// Write the keymap out now, so a rebinding survives even a hard kill --
    /// this is the half of acceptance criterion 28 that happens before the
    /// restart. Called from every place the keymap editor changes something.
    pub fn persist_keymap(&self) {
        let _ = config::save_keymap_to(&self.config_dir, &self.keymap);
    }
}
