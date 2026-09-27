//! One frame: preparing the renderer and laying the interface out.

use super::*;
use crate::panel_viewport;
use simple3d_core::config::{self, Side};

impl App {
    /// Build the GPU renderer if wanted and possible, and register its texture with egui. Failure is
    /// kept and shown beside the engine picker, and the viewport draws in software.
    pub(super) fn prepare_gpu(&mut self, frame: &mut eframe::Frame) {
        if self.settings.render_engine != config::RenderEngine::Gpu || self.gpu.is_some() {
            return;
        }
        if self.gpu_error.is_some() {
            return; // asked once, refused once; do not retry every frame
        }
        let Some(gl) = self.gl.clone() else {
            self.gpu_error = Some("no OpenGL context (the window has none)".to_string());
            return;
        };
        let mut gpu = match crate::gpu::Gpu::new(gl) {
            Ok(gpu) => gpu,
            Err(why) => {
                self.gpu_error = Some(why);
                return;
            }
        };
        // Made now so there is something to register; the first frame resizes it keeping its identity.
        match gpu.prepare_texture() {
            Ok(texture) => {
                let id = frame.register_native_glow_texture(texture);
                gpu.set_texture_id(id);
                self.gpu = Some(gpu);
                self.image_key = u64::MAX;
            }
            Err(why) => self.gpu_error = Some(why),
        }
    }

    /// Everything the window is, in stacking order. Separate from `update` so tests can drive a
    /// real frame against a headless context.
    pub fn ui(&mut self, ctx: &egui::Context) {
        // Before the panels, so changes are written the next frame. Only the root viewport's window
        // persists geometry and settings (issue 107); the shell spreads changes to every window.
        if self.root_window {
            self.record_window_shape(ctx);
            self.persist_settings_if_changed(ctx);
        }
        self.menu_bar(ctx);
        self.status_bar(ctx);
        crate::panel_toolrail::show(self, ctx);
        crate::dock::show(self, ctx, Side::Left);
        crate::dock::show(self, ctx, Side::Right);
        // After the docks and rail, so the tab row spans only the workspace it belongs to.
        crate::tabs::show(self, ctx);
        // The project's components, under its tab (issue 113).
        crate::components::strip::show(self, ctx);
        panel_viewport::show(self, ctx);
        // Drawn after the viewport, as the layer above it (issue 82).
        crate::split_tool::show(self, ctx);
        crate::simplify_tool::show(self, ctx);
        crate::reassemble_tool::show(self, ctx);
        crate::measure_tool::show(self, ctx);
        crate::section_tool::show(self, ctx);
        crate::pattern_tool::show(self, ctx);
        crate::noise_popup::show(self, ctx);
        crate::dock::resolve_drag(self, ctx);
        // Tab drags leaving the window (issue 107), after the panels so the tab row's rect is known.
        crate::tabs::resolve_drag(self, ctx);
        // Block the main window's pointer while a dialog is open, so the document cannot be edited
        // behind it. Drawn before the dialog so an embedded dialog stays above and live.
        if self.modal != Modal::None {
            egui::Modal::new(egui::Id::new("dialog-backdrop"))
                .backdrop_color(egui::Color32::from_black_alpha(64))
                .frame(egui::Frame::NONE)
                .show(ctx, |_ui| {});
        }
        self.modals(ctx);
        // A drag released where nothing can take it is dropped, so no phantom drag lingers.
        if ctx.input(|i| !i.pointer.any_down()) {
            self.outliner_drag = None;
            self.drop_target = None;
        }
    }
}
