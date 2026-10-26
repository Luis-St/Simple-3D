//! The keyboard while sketching: Esc steps back one thing at a time, single letters pick tools as in
//! other CAD programs, V toggles the 2D lock and Home turns back to the sketch, all winning over the
//! keymap while the sketch is open.

use super::*;
use crate::app::App;

impl App {
    /// Take Esc and the tool keys while the sketch is open; `true` when one was taken, so the keymap
    /// does not act on it as well.
    pub(crate) fn sketch_keys(&mut self, ctx: &egui::Context) -> bool {
        let Some(tool) = self.sketch_tool.as_mut() else { return false };
        // Esc ends the gesture, then closes the popup, then drops the selection, then goes back to
        // selecting; leaving takes Cancel, so a stray Esc cannot throw a drawing away (issue 74).
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if !tool.clicks.is_empty() {
                tool.end_gesture();
            } else if tool.inspect.is_some() {
                tool.inspect = None;
            } else if !tool.selection.is_empty() {
                tool.selection.clear();
            } else {
                tool.set_draw(Draw::Select);
            }
            return true;
        }
        if tool.frame.is_none() {
            return false;
        }
        let plain = ctx.input(|i| i.modifiers.is_none());
        if plain && ctx.input(|i| i.key_pressed(super::view::LOCK_KEY)) {
            self.toggle_sketch_lock();
            return true;
        }
        if plain && ctx.input(|i| i.key_pressed(super::view::LOOK_KEY)) {
            self.return_to_sketch();
            return true;
        }
        let Some(tool) = self.sketch_tool.as_mut() else { return false };
        let pressed = Draw::keyed().find(|&(_, key)| plain && ctx.input(|i| i.key_pressed(key)));
        match pressed {
            Some((draw, _)) => {
                tool.set_draw(draw);
                true
            }
            None => false,
        }
    }
}
