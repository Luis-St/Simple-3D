//! The application as the window system sees it.

use super::*;
use crate::render::Renderable;
use std::time::Duration;

impl App {
    /// What is behind the window's own painting. The frame belongs to the
    /// window system now, so nothing is rounded away and nothing needs to show
    /// the desktop through it: an opaque surface, and no transparency for a
    /// compositor to have to blend.
    pub(crate) fn clear_colour(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = crate::theme::token::SURFACE_0;
        [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, 1.0]
    }

    /// One frame of this window. The shell calls it for every window it has
    /// open, each in its own viewport (issue 107); it used to be `eframe::App`'s
    /// `update`, back when a window and the application were the same thing.
    pub(crate) fn run_frame(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // The GPU renderer's texture is registered with egui exactly once, here,
        // where `eframe::Frame` is in reach -- `App::ui` is also driven by the
        // test harness, which has no window and so no context to register with.
        // The texture keeps its identity across a resize, so one registration
        // lasts the life of the application.
        self.prepare_gpu(frame);
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

        // Confirmation on quit (spec section 7.4): intercept the window's own
        // close button as well as the Quit command.
        //
        // The button closes *this* window, which with one window open is the
        // same thing as quitting and with several is not (issue 107). Either
        // way the close is always cancelled here and asked for through the
        // shell instead: a window cannot take itself out of the application,
        // and the root viewport cannot be closed at all without ending the
        // process under the other windows.
        //
        // The close the *application* asked for is let through: `Shell::quit`
        // ends the run by closing the root viewport, and a handler that
        // cancelled that close as well would cancel every way out of the
        // application -- which is exactly what it did, and why the close button
        // stopped working.
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
        // Keep animating while work is in flight, so progress and the preview
        // update without the user having to move the mouse. A drag and a camera
        // move are followed frame by frame, because the next frame is the
        // answer to the pointer; everything else asks for the frame it will
        // actually have something new to show in, since a repaint requested
        // with no delay is a repaint requested for right now, and the loop then
        // runs as fast as the machine allows.
        if self.drag.is_some() || self.camera_move.is_some() {
            ctx.request_repaint();
        } else if self.work_in_flight() {
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
}

impl App {
    /// Whether something is running that will change what is on screen without
    /// any input to wake the loop for it: an evaluation, an export, a cut into
    /// pieces, or a simplification.
    ///
    /// Each of these hands its result back over a channel, which is not an
    /// event the toolkit knows about -- so a frame that is not asked for here
    /// is a result that sits in the channel until the pointer happens to move.
    /// For the simplify tool that is the whole picture: its result *is* the
    /// preview (issue 106).
    pub(crate) fn work_in_flight(&self) -> bool {
        // A scene marked for re-evaluation counts as work: the submission
        // happens at the top of a frame and the marking usually happens in the
        // middle of one, so the evaluation a tool asked for does not start
        // until the frame after -- and if nothing asks for that frame, it never
        // starts at all. That is not hypothetical: it is what left the simplify
        // tool's window reporting a result the viewport never showed.
        self.dirty
            || self.worker.is_busy()
            || self.export_job.is_some()
            || self.split_job.is_some()
            || self.simplify_tool.as_ref().is_some_and(|tool| tool.job.is_some())
    }
}
