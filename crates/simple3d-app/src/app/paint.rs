//! Colour: painting a selection, and the recent colours that follow from it.

use super::*;
use simple3d_core::scene::{Colour, NodeId};

impl App {
    /// Paint every node in `targets`, and remember the colour so it can be
    /// offered again. `None` clears the paint instead. One place, because the
    /// property editor and the outliner's menu both do this and both have to
    /// remember it.
    pub(crate) fn paint(&mut self, targets: &[NodeId], colour: Option<Colour>, coalesce: Option<&str>) {
        self.apply_paint(targets, colour, coalesce);
        if let Some(Colour(rgb)) = colour {
            // Only a colour the user picked out for themselves. The eight
            // presets are already on the palette above this row; repeating one
            // of them here spends the recent list on colours that were never
            // hard to find (issue 35).
            if !is_preset(rgb) {
                self.settings.remember_colour(rgb);
            }
        }
    }

    /// Paint from the colour picker, where the colour is still being chosen.
    ///
    /// Every frame of a drag through the picker comes through here, so nothing
    /// is remembered yet: what the picker has reached is held until it is put
    /// away, and [`App::picker_closed`] puts that one colour on the recent row
    /// (issue 85). Before this, a drag deposited a swatch every time it paused
    /// for longer than the undo history's coalescing window -- which is how a
    /// single wander through the dark corner of the picker left eight shades of
    /// black on a row meant to hold eight colours.
    pub(crate) fn paint_from_picker(&mut self, targets: &[NodeId], rgb: [u8; 3]) {
        self.apply_paint(targets, Some(Colour(rgb)), Some("colour"));
        self.picker_colour = Some(rgb);
    }

    /// The picker is closed: whatever it ended on is the colour that was
    /// chosen, and the only one of the drag worth offering again.
    ///
    /// Called on every frame the picker is not open, so it has to cost nothing
    /// when there is nothing waiting.
    pub(crate) fn picker_closed(&mut self) {
        let Some(rgb) = self.picker_colour.take() else { return };
        if !is_preset(rgb) {
            self.settings.remember_colour(rgb);
        }
    }

    /// The paint itself, without the question of what to remember.
    pub(super) fn apply_paint(&mut self, targets: &[NodeId], colour: Option<Colour>, coalesce: Option<&str>) {
        self.edit(if colour.is_some() { "Colour" } else { "Clear colour" }, coalesce);
        for target in targets {
            self.scene.paint_subtree(*target, colour);
        }
    }

    /// The recent colours worth offering: the ones that are not already a
    /// preset, and not a shade of one further up the row.
    ///
    /// Filtered on the way out as well as on the way in, so a list saved by an
    /// earlier version stops showing them too -- which matters here, because
    /// the version that filled a row with eight shades of black wrote them to
    /// the settings file and they would otherwise sit there forever (issue 85).
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
