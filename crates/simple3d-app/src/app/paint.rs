//! Colour: painting a selection, and the recent colours that follow.

use super::*;
use simple3d_core::scene::{Colour, NodeId};

impl App {
    /// Paint every target and remember the colour; `None` clears paint. Shared by the property editor
    /// and the outliner menu.
    pub(crate) fn paint(&mut self, targets: &[NodeId], colour: Option<Colour>, coalesce: Option<&str>) {
        self.apply_paint(targets, colour, coalesce);
        if let Some(Colour(rgb)) = colour {
            // Only custom colours are remembered; presets are already on the palette (issue 35).
            if !is_preset(rgb) {
                self.settings.remember_colour(rgb);
            }
        }
    }

    /// Paint from the picker while choosing: nothing is remembered until [`App::picker_closed`], so a
    /// drag through the picker adds one swatch, not many (issue 85).
    pub(crate) fn paint_from_picker(&mut self, targets: &[NodeId], rgb: [u8; 3]) {
        self.apply_paint(targets, Some(Colour(rgb)), Some("colour"));
        self.picker_colour = Some(rgb);
    }

    /// The picker closed: remember the colour it ended on. Runs every frame, so it is free when idle.
    pub(crate) fn picker_closed(&mut self) {
        let Some(rgb) = self.picker_colour.take() else { return };
        if !is_preset(rgb) {
            self.settings.remember_colour(rgb);
        }
    }

    /// The paint itself, without remembering.
    pub(super) fn apply_paint(&mut self, targets: &[NodeId], colour: Option<Colour>, coalesce: Option<&str>) {
        self.edit(if colour.is_some() { "Colour" } else { "Clear colour" }, coalesce);
        for target in targets {
            self.scene.paint_subtree(*target, colour);
        }
    }

    /// Recent colours worth offering: not presets or shades already on the row. Filtered on the way
    /// out too, cleaning lists saved by older versions (issue 85).
    pub(crate) fn custom_recent_colours(&self) -> Vec<[u8; 3]> {
        let mut kept: Vec<[u8; 3]> = Vec::new();
        for rgb in self.settings.recent_colours.iter().copied() {
            if is_preset(rgb) || kept.iter().any(|shown| simple3d_core::config::indistinguishable(*shown, rgb)) {
                continue;
            }
            kept.push(rgb);
        }
        kept
    }
}
