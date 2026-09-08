//! The application as the window system sees it.

use super::*;
use crate::render::Renderable;
use std::time::Duration;

impl eframe::App for App {
    /// What is behind the window's own painting. The frame belongs to the
    /// window system now, so nothing is rounded away and nothing needs to show
    /// the desktop through it: an opaque surface, and no transparency for a
    /// compositor to have to blend.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = crate::theme::token::SURFACE_0;
        [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // The GPU renderer's texture is registered with egui exactly once, here,
        // where `eframe::Frame` is in reach -- `App::ui` is also driven by the
        // test harness, which has no window and so no context to register with.
        // The texture keeps its identity across a resize, so one registration
        // lasts the life of the application.
        self.prepare_gpu(frame);
        // Asked every frame rather than once at startup, so the setting takes
        // effect on the next dialog rather than on the next run. egui reads it
        // when a viewport is shown, and `dialog` already draws the embedded
        // form -- it is the one the headless tests have always driven.
        ctx.set_embed_viewports(self.settings.embed_dialogs);
        // A new message restarts its clock. Watching the value rather than
        // stamping it at every assignment means no `status = ...` anywhere in
        // the application can forget to.
        if self.status != self.last_status {
            self.last_status = self.status.clone();
            self.status_at = std::time::Instant::now();
        }
        if let Some(result) = self.worker.poll() {
            self.evaluated = result;
            self.evaluation_generation += 1;
            if std::mem::take(&mut self.frame_when_evaluated) {
                // The starting scene could not be framed before it had been
                // evaluated, since framing needs its bounds.
                self.frame_all();
            }
            self.scene_renderable = Renderable::prepare(&self.evaluated.mesh);
            self.renderable_key = u64::MAX;
            self.image_key = u64::MAX;
        }
        if self.dirty {
            self.worker.submit(&self.scene);
            self.dirty = false;
        }
        self.poll_export();
        self.poll_split();
        self.poll_file_prompt();
        self.advance_camera();
        self.refresh_node_renderables();
        if let Some(text) = self.clipboard_text.take() {
            ctx.copy_text(text);
        }

        let title = self.title();
        if title != self.last_title {
            self.last_title.clone_from(&title);
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        self.handle_shortcuts(ctx);

        self.ui(ctx);

        // Confirmation on quit (spec section 7.4): intercept the window's own
        // close button as well as the Quit command.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit_now {
            if self.any_unsaved() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.modal = Modal::ConfirmQuit;
            } else {
                self.quit_now = true;
            }
        }
        if self.quit_now {
            self.persist();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // Keep animating while work is in flight, so progress and the preview
        // update without the user having to move the mouse. A drag and a camera
        // move are followed frame by frame, because the next frame is the
        // answer to the pointer; everything else asks for the frame it will
        // actually have something new to show in, since a repaint requested
        // with no delay is a repaint requested for right now, and the loop then
        // runs as fast as the machine allows.
        if self.drag.is_some() || self.camera_move.is_some() {
            ctx.request_repaint();
        } else if self.worker.is_busy() || self.export_job.is_some() || self.split_job.is_some() {
            // Progress and the preview, at a rate a person can read rather than
            // at whatever the rasterizer can manage.
            ctx.request_repaint_after(Duration::from_millis(33));
        } else if self.file_prompt.is_some() {
            // The dialog answers on a thread, and nothing else will wake this
            // loop to notice: the answer arrives with no event of its own. Four
            // times a second is enough to pick it up without being felt, and it
            // keeps the footer's count of seconds honest.
            ctx.request_repaint_after(Duration::from_millis(250));
        } else if self.status != Status::Idle {
            // A status message is still for its whole lifetime and only then
            // fades. Nothing changes until the fade starts, so ask for the
            // frame that starts it, and only during the fade for frames after
            // that.
            let age = self.status_at.elapsed();
            if age < STATUS_LIFETIME {
                ctx.request_repaint_after(STATUS_LIFETIME - age);
            } else if status_opacity(&self.status, age) > 0.0 {
                ctx.request_repaint_after(Duration::from_millis(33));
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist();
    }
}
