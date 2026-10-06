//! The application as the window system sees it.

use super::*;
use std::time::Duration;

impl App {
    /// The colour behind the window's painting: opaque, with nothing for a compositor to blend.
    pub(crate) fn clear_colour(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = crate::theme::token::SURFACE_0;
        [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, 1.0]
    }

    /// One frame of this window; the shell calls it for every window, each in its own viewport
    /// (issue 107).
    pub(crate) fn run_frame(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // The GPU texture is registered with egui once, here, where `eframe::Frame` is available; the
        // test harness drives `App::ui` without one.
        self.prepare_gpu(frame);
        // A new message restarts its clock; watching the value means no assignment can forget to.
        if self.status != self.last_status {
            self.last_status = self.status.clone();
            self.status_at = std::time::Instant::now();
        }
        if let Some((result, renderable)) = self.worker.poll() {
            self.evaluated = result;
            self.evaluation_generation += 1;
            if std::mem::take(&mut self.frame_when_evaluated) {
                // The starting scene can only be framed once evaluated, since framing needs its bounds.
                self.frame_all();
            }
            self.scene_renderable = renderable;
            self.warm_snaps();
            self.settle();
            self.renderable_key = u64::MAX;
            self.image_key = u64::MAX;
        }
        // A GPU-drawn drag is evaluated once, when it ends (`App::live_drag`, `App::live_csg`).
        let submitting = self.dirty && !self.drag_drawn_live();
        if submitting {
            self.worker.want(self.wanted_renderables());
            self.worker.submit(&self.scene);
            self.dirty = false;
        }
        self.refresh_committed(submitting);
        self.poll_snap_warming();
        self.poll_export();
        self.poll_import();
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

        // Confirmation on quit (spec section 7.4), including the window's close button. The close is
        // always cancelled and routed through the shell (issue 107), since a window cannot remove
        // itself and closing the root viewport ends the process. The shell's own quit close
        // (`leaving`) is let through, or nothing could ever close the app.
        if ctx.input(|i| i.viewport().close_requested()) && !self.leaving {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request_close_window();
        }
        if std::mem::take(&mut self.quit_now) {
            self.window_request = Some(crate::shell::WindowRequest::Quit);
        }
        if std::mem::take(&mut self.close_now) {
            self.window_request = Some(crate::shell::WindowRequest::Close);
        }
        // Keep repainting while work is in flight. Drags and camera moves repaint every frame; the rest
        // request a delayed frame, since an undelayed request spins the loop flat out.
        if self.drag.is_some() || self.camera_move.is_some() {
            ctx.request_repaint();
        } else if self.work_in_flight() {
            // Progress and preview at a readable rate, not as fast as possible.
            ctx.request_repaint_after(Duration::from_millis(33));
        } else if self.file_prompt.is_some() {
            // The file dialog answers on a thread with no event, so poll four times a second; this also
            // keeps the footer's seconds count current.
            ctx.request_repaint_after(Duration::from_millis(250));
        } else if self.status != Status::Idle {
            // A status message is static until its fade starts, so ask only for that frame, then for fade frames.
            let age = self.status_at.elapsed();
            if age < STATUS_LIFETIME {
                ctx.request_repaint_after(STATUS_LIFETIME - age);
            } else if status_opacity(&self.status, age) > 0.0 {
                ctx.request_repaint_after(Duration::from_millis(33));
            }
        }
    }
}

impl App {
    /// Whether background work (evaluation, export, split, simplify, reassembly) will change the
    /// screen without input. Results come over channels, which do not wake the loop.
    pub(crate) fn work_in_flight(&self) -> bool {
        // A dirty scene counts: it is submitted at the top of the next frame, which nothing else would
        // request (this once left the simplify tool's result unshown).
        self.dirty
            || self.worker.is_busy()
            || self.export_job.is_some()
            || self.export_preview.running.is_some()
            || self.split_job.is_some()
            || self.simplify_tool.as_ref().is_some_and(|tool| tool.job.is_some())
            || self.reassemble_tool.as_ref().is_some_and(|tool| tool.job.is_some())
    }
}
