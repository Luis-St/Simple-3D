//! The view while sketching: the 2D lock, which keeps the camera straight on the sketch with its X
//! running right and its Y up, and turning back to that view after orbiting away from it.

use super::*;
use crate::app::{App, Status};

/// The key that toggles the 2D lock while sketching.
pub(crate) const LOCK_KEY: egui::Key = egui::Key::V;
/// The key that turns the view back onto the sketch and its origin.
pub(crate) const LOOK_KEY: egui::Key = egui::Key::Home;

const HELD_BACK: &str = "The 2D lock keeps the view on the sketch; V turns it off";

impl App {
    /// Whether the 2D lock holds the view: sketching on a plane with the lock on.
    pub(crate) fn sketch_flat(&self) -> bool {
        self.sketch_tool.as_ref().is_some_and(|tool| tool.frame.is_some() && tool.flat)
    }

    /// Turn the 2D lock on or off.
    pub(crate) fn toggle_sketch_lock(&mut self) {
        let Some(mut tool) = self.sketch_tool.take() else { return };
        let on = !tool.flat;
        set_lock(self, &mut tool, on);
        self.sketch_tool = Some(tool);
    }

    /// The Look at sketch button and its key: straight on the sketch again, centred on its origin.
    pub(crate) fn return_to_sketch(&mut self) {
        let Some(frame) = self.sketch_tool.as_ref().and_then(|tool| tool.frame) else { return };
        self.scene.camera.target = frame.t;
        self.look_at_frame(frame);
    }

    /// What a view change does under the 2D lock: turn back onto the sketch, and say why.
    pub(crate) fn look_at_sketch_locked(&mut self) {
        self.look_at_sketch();
        self.status = held_back();
    }
}

/// Status line for an orbit the 2D lock held back.
pub(crate) fn held_back() -> Status {
    Status::Info(HELD_BACK.into())
}

/// Set the 2D lock on `tool`, lifted out of the app; turning it on also turns back onto the sketch.
fn set_lock(app: &mut App, tool: &mut SketchTool, on: bool) {
    tool.flat = on;
    if let (true, Some(frame)) = (on, tool.frame) {
        app.look_at_frame(frame);
    }
    app.status = Status::Info(
        if on { "2D lock on: the view stays on the sketch" } else { "2D lock off: the view orbits freely" }.into(),
    );
}

/// The panel's view row: the 2D lock, lit while it is on, and the button back to the sketch's own view.
pub(crate) fn group(app: &mut App, ui: &mut egui::Ui, tool: &mut SketchTool) {
    super::window::group(ui, "View", |ui| {
        ui.horizontal(|ui| {
            let size = egui::vec2(super::window::button_width(ui, 2), 0.0);
            let hint = "Keep the view straight on the sketch, its X to the right and its Y up; orbiting and view \
                        changes are held back. (V)";
            if crate::theme::choice_sized(ui, tool.flat, "2D lock", size.x).on_hover_text(hint).clicked() {
                set_lock(app, tool, !tool.flat);
            }
            let look = egui::Button::new("Look at sketch").min_size(size);
            let look = ui.add(look).on_hover_text("Turn back to straight on the sketch, centred on its origin. (Home)");
            if let (true, Some(frame)) = (look.clicked(), tool.frame) {
                app.scene.camera.target = frame.t;
                app.look_at_frame(frame);
            }
        });
    });
}
