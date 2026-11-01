//! The keyboard while sketching: Esc steps back one thing at a time, single letters pick tools as in
//! other CAD programs -- pressed again they step through the family's variants -- Q switches new
//! curves to helper geometry and back, V toggles the 2D lock and Home turns back to the sketch, all
//! winning over the keymap while the sketch is open.

use super::*;
use crate::app::App;

/// Switches the drawing tools between profile and helper geometry.
pub(crate) const CONSTRUCTION_KEY: egui::Key = egui::Key::Q;

/// The tool a family's key picks: its remembered variant, or the next one when the family is in use
/// already.
pub(crate) fn next_variant(current: Draw, remembered: Draw, family: &[Draw]) -> Draw {
    match family.iter().position(|&d| d == current) {
        Some(i) => family[(i + 1) % family.len()],
        None => remembered,
    }
}

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
        if plain && ctx.input(|i| i.key_pressed(CONSTRUCTION_KEY)) {
            tool.settings.construction = !tool.settings.construction;
            return true;
        }
        let pressed = Draw::keyed().find(|&(_, key)| plain && ctx.input(|i| i.key_pressed(key)));
        match pressed {
            Some((family, _)) => {
                tool.set_draw(next_variant(tool.draw, tool.settings.variant(family), family));
                true
            }
            None => false,
        }
    }
}
