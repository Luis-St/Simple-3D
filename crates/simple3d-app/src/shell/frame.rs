//! One frame of the whole application: every window in order, then their requests.

use super::*;
use crate::app::APP_NAME;

impl eframe::App for Shell {
    /// The colour behind the windows' painting; all windows give the same answer.
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.windows[0].clear_colour(visuals)
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Read every frame so the setting applies to the next dialog. With embedded dialogs there are no
        // viewports, so extra windows are folded back into the first.
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
                // No viewports after all (backend or setting changed), so this window's documents go back to
                // the visible one.
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

/// A document window's builder: the same furniture as the main window, since it is one.
fn builder_for(window: &App) -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_title(window.title())
        // The same app id, so the desktop groups all windows together.
        .with_app_id("net.simple3d.Simple3D")
        .with_icon(crate::icon::shared_icon())
        .with_inner_size(window.settings.window_size)
        .with_min_inner_size([900.0, 560.0])
        .with_decorations(true)
}

impl Shell {
    /// Tell every window about the others, for sending and dropping tabs. Positions come from each
    /// window's last frame, which lags only while that window moves.
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

    /// Carry a setting changed in one window to the others. Each window has its own `AppSettings` and
    /// only the first writes the file, so a change elsewhere is taken as the truth and spread.
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

    /// Put every document into the first window and close the rest, for when no viewports exist.
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
