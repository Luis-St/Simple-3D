//! One frame of the whole application: every window, in the order they are
//! shown, and the requests they leave behind.

use super::*;
use crate::app::APP_NAME;

impl eframe::App for Shell {
    /// What is behind the windows' own painting. Asked of the application rather
    /// than of a window, so it is the first window's answer -- they all give the
    /// same one.
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.windows[0].clear_colour(visuals)
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Asked every frame rather than once at startup, so the setting takes
        // effect on the next dialog rather than on the next run. It governs the
        // document windows too: with dialogs embedded there are no viewports to
        // be had at all, so those windows are folded back into the first one
        // rather than left drawing nothing where nobody can reach them.
        ctx.set_embed_viewports(self.settings.embed_dialogs);
        if self.settings.embed_dialogs && self.windows.len() > 1 {
            self.fold_windows_together();
        }
        self.tell_windows_about_each_other();

        let root_id = self.windows[0].window_id;
        self.windows[0].root_window = true;
        self.windows[0].run_frame(ctx, frame);

        for index in 1..self.windows.len() {
            let viewport = self.viewport_of(index);
            let window = &mut self.windows[index];
            window.root_window = false;
            let builder = builder_for(window);
            ctx.show_viewport_immediate(viewport, builder, |ctx, class| {
                // No viewports to be had after all: the backend has none, or a
                // dialog turned embedding on between this frame and the last.
                // Either way this window cannot be seen, so its documents go
                // back to the window that can be.
                if class == egui::ViewportClass::Embedded {
                    window.window_request = Some(WindowRequest::MoveAll(root_id));
                    return;
                }
                window.run_frame(ctx, frame);
            });
        }

        self.share_settings();
        self.resolve_requests(ctx);
        self.resolve_offer(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.windows[0].persist();
    }
}

/// The window a document is shown in: the same furniture as the main one, since
/// it *is* a main one -- there is no lesser kind of document window.
fn builder_for(window: &App) -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_title(window.title())
        // The same application id as the first window, so the desktop groups
        // them together in the window list and the dock rather than showing one
        // unnamed window per document.
        .with_app_id("net.simple3d.Simple3D")
        .with_icon(crate::icon::shared_icon())
        .with_inner_size(window.settings.window_size)
        .with_min_inner_size([900.0, 560.0])
        .with_decorations(true)
}

impl Shell {
    /// Tell every window what the others are, so a tab can be sent to one of
    /// them by name and dropped onto one by hand.
    ///
    /// Where each window *is* comes from the frame it last drew: a window
    /// reports its own rectangle while it draws, which is one frame of lag on a
    /// window that is being moved and none at all on the drop that matters --
    /// the pointer is over the window being dropped on, so that window is not
    /// the one moving.
    fn tell_windows_about_each_other(&mut self) {
        let known: Vec<OtherWindow> = self
            .windows
            .iter()
            .map(|window| OtherWindow {
                id: window.window_id,
                name: window.window_summary(),
                strip: window.strip_on_screen(),
            })
            .collect();
        let unsaved: Vec<bool> = self.windows.iter().map(|window| window.any_unsaved()).collect();
        for (index, window) in self.windows.iter_mut().enumerate() {
            window.other_windows = known.iter().filter(|other| other.id != window.window_id).cloned().collect();
            window.unsaved_elsewhere = unsaved.iter().enumerate().any(|(other, unsaved)| other != index && *unsaved);
        }
    }

    /// Carry a setting changed in one window over to the others.
    ///
    /// Every window holds its own `AppSettings` -- the whole application reads
    /// `app.settings`, and threading one shared copy through all of it would be
    /// a change to every panel -- and only the first window writes the file. A
    /// change made in any other window is therefore taken as the new truth here
    /// and handed to the rest, which is also what gets it written.
    fn share_settings(&mut self) {
        if let Some(changed) = self.windows.iter().find(|window| window.settings != self.settings) {
            self.settings = changed.settings.clone();
        }
        for window in &mut self.windows {
            if window.settings != self.settings {
                window.settings = self.settings.clone();
            }
        }
    }

    /// Put every document back into the first window and close the rest. What
    /// happens when there are no viewports to show a second window in.
    pub(super) fn fold_windows_together(&mut self) {
        while self.windows.len() > 1 {
            let documents = self.windows.pop().expect("more than one window").take_all_tabs();
            for document in documents {
                self.windows[0].adopt(document);
            }
        }
        self.windows[0].status = crate::app::Status::Warning(format!(
            "{APP_NAME} is drawing dialogs inside the window, so every document is in this one"
        ));
    }
}
