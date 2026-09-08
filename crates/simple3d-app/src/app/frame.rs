//! One frame: preparing the renderer and laying the interface out.

use super::*;
use crate::panel_viewport;
use simple3d_core::config::{self, Side};

impl App {
    /// Everything the window is, in the order it stacks: the bars, the tool
    /// rail, the two docks, the viewport, the drag that may be crossing between
    /// them, and whatever modal is open over the lot.
    ///
    /// Separate from `update` so a test can drive a real frame -- the same
    /// panels in the same order -- against a headless context and replay a
    /// pointer over it. A gesture that is only ever performed by hand is a
    /// gesture nothing checks.
    /// Build the GPU renderer if it is wanted and can be had, and give egui the
    /// texture it draws into.
    ///
    /// Failure here is not fatal and is not silent: the reason is kept and shown
    /// beside the engine picker, and the viewport goes on drawing in software.
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
        // A texture of its own, made at once so there is something to register;
        // the first real frame reallocates it to the viewport's size without
        // changing its identity.
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

    pub fn ui(&mut self, ctx: &egui::Context) {
        // Before the panels rather than after: what they change this frame is
        // written on the next one, and a frame is drawn for every change any of
        // them makes.
        self.record_window_shape(ctx);
        self.persist_settings_if_changed(ctx);
        self.menu_bar(ctx);
        self.status_bar(ctx);
        crate::panel_toolrail::show(self, ctx);
        crate::dock::show(self, ctx, Side::Left);
        crate::dock::show(self, ctx, Side::Right);
        // After the docks and the rail, so the row of tabs spans the workspace
        // itself rather than the whole window: the documents belong to the
        // viewport under them, not to the window's chrome.
        crate::tabs::show(self, ctx);
        panel_viewport::show(self, ctx);
        // Over the viewport, and drawn after it so it is the layer above: an
        // in-place popup is part of the picture rather than a window in front of
        // the application (issue 82).
        crate::split_tool::show(self, ctx);
        crate::measure_tool::show(self, ctx);
        crate::section_tool::show(self, ctx);
        crate::dock::resolve_drag(self, ctx);
        // A dialog is modal, and it was only half of one: `handle_shortcuts`
        // hands it the keyboard, but nothing stopped the main window taking the
        // pointer, so a dialog left open behind it was an application whose
        // shortcuts had all silently stopped while the document could still be
        // edited by mouse -- including, with the Export dialog up, editing the
        // very geometry that dialog is reporting on.
        //
        // Drawn before the dialog itself, so the dialog's own layer is created
        // after this one and therefore sits above it: an embedded dialog is a
        // window in this same viewport and has to stay live. A dialog that is a
        // window of its own is a viewport of its own, which this cannot reach.
        if self.modal != Modal::None {
            egui::Modal::new(egui::Id::new("dialog-backdrop"))
                .backdrop_color(egui::Color32::from_black_alpha(64))
                .frame(egui::Frame::NONE)
                .show(ctx, |_ui| {});
        }
        self.modals(ctx);
        // A load let go of somewhere no drop could take it is simply put down.
        // The outliner ends a drag it can see the end of, but a shape picked up
        // from the palette can be released over a window with no outliner in it
        // at all -- and a drag left running would have shown a phantom slab the
        // next time the tree was opened.
        if ctx.input(|i| !i.pointer.any_down()) {
            self.outliner_drag = None;
            self.drop_target = None;
        }
    }
}
