//! The edits the builder and the window make to a pattern's scatter.

use super::*;
use crate::app::App;
use simple3d_core::pattern;
use simple3d_core::primitive::{ParamValue, ParamsExt};
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    /// Whether anything here has been moved off what a fresh pattern holds.
    ///
    /// Asked of all of them rather than of the scatter itself: a seed stepped
    /// past a scatter that looked wrong is something the user set and something
    /// Reset puts back, so a button that went grey while it still said something
    /// would be lying about what it does.
    pub(crate) fn noise_is_set(&self, id: NodeId) -> bool {
        let Some(params) = self.scene.get(id).and_then(|node| node.params()) else { return false };
        pattern::noise_keys()
            .iter()
            .any(|key| default_of(key).is_some_and(|default| params.get(*key).is_some_and(|value| *value != default)))
    }

    /// Take the scatter off: every one of the six back to what a fresh pattern
    /// holds.
    ///
    /// One undo step for the lot. Four numbers typed back to zero by hand is
    /// four of them, and stopping halfway through leaves a pattern that is still
    /// scattered by whatever is left.
    pub(crate) fn reset_noise(&mut self, id: NodeId) {
        if !self.noise_is_set(id) {
            return;
        }
        self.edit("Reset noise", None);
        self.reset_noise_keys(id, pattern::noise_keys());
        self.noise_parts_open = (None, [false; Part::COUNT]);
        self.touch();
    }

    /// Put `keys` back to what a fresh pattern holds, as part of an edit already
    /// begun.
    fn reset_noise_keys(&mut self, id: NodeId, keys: &[&str]) {
        let Some(params) = self.scene.get_mut(id).and_then(|node| node.params_mut()) else { return };
        for key in keys {
            if let Some(default) = default_of(key) {
                params.insert(key.to_string(), default);
            }
        }
    }

    /// Whether the builder is keeping `part` on screen for `id` although it is
    /// at nothing.
    pub(super) fn noise_part_open(&self, id: NodeId, part: Part) -> bool {
        self.noise_parts_open.0 == Some(id) && self.noise_parts_open.1[part.index()]
    }

    fn set_noise_part_open(&mut self, id: NodeId, part: Part, open: bool) {
        if self.noise_parts_open.0 != Some(id) {
            self.noise_parts_open = (Some(id), [false; Part::COUNT]);
        }
        self.noise_parts_open.1[part.index()] = open;
    }

    /// Add one part to the scatter, at an amount that shows what it does.
    ///
    /// A nudge is a twentieth of the shape's narrower side, along the two axes
    /// a plank laid on a floor wanders in: enough to see, and well short of the
    /// gap a fresh pattern leaves between its copies, so adding it does not
    /// weld them together. A turn is three degrees, a size five percent.
    pub(crate) fn add_noise_part(&mut self, id: NodeId, part: Part) {
        let size = self.pattern_content_size(id).unwrap_or(Vec3::ZERO);
        self.edit("Add noise", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|node| node.params_mut()) {
            match part {
                Part::Nudge => {
                    let narrow = size.x.min(size.y);
                    let amount = if narrow > 1e-9 { narrow * 0.05 } else { 0.5 };
                    params.insert("noise_x".to_string(), ParamValue::Length(amount));
                    params.insert("noise_y".to_string(), ParamValue::Length(amount));
                }
                Part::Turn(axis) => {
                    params.insert(pattern::NOISE_TURN_KEYS[axis.min(2)].to_string(), ParamValue::Angle(3.0));
                }
                Part::Size => {
                    params.insert("noise_scale".to_string(), ParamValue::Count(5));
                }
            }
        }
        self.set_noise_part_open(id, part, true);
        self.touch();
    }

    /// Take one part of the scatter off, leaving the others as they are.
    pub(crate) fn drop_noise_part(&mut self, id: NodeId, part: Part) {
        self.edit("Remove noise", None);
        self.reset_noise_keys(id, part.keys());
        self.set_noise_part_open(id, part, false);
        self.touch();
    }

    /// Move the turn about `from` onto `to`, amount and all -- unless `to` has a
    /// turn of its own already.
    pub(crate) fn move_noise_turn(&mut self, id: NodeId, from: usize, to: usize) {
        let (from, to) = (from.min(2), to.min(2));
        if from == to || self.noise_part_open(id, Part::Turn(to)) {
            return;
        }
        let Some(params) = self.scene.get(id).and_then(|node| node.params()) else { return };
        if Part::Turn(to).in_use(params) {
            return;
        }
        let amount = params.num(pattern::NOISE_TURN_KEYS[from]);
        self.edit("Set noise", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|node| node.params_mut()) {
            params.insert(pattern::NOISE_TURN_KEYS[from].to_string(), ParamValue::Angle(0.0));
            params.insert(pattern::NOISE_TURN_KEYS[to].to_string(), ParamValue::Angle(amount));
        }
        self.set_noise_part_open(id, Part::Turn(from), false);
        self.set_noise_part_open(id, Part::Turn(to), true);
        self.touch();
    }

    /// Step the seed on to the next scatter of the same size.
    ///
    /// One undo step however many times it is pressed in a row: shuffling is
    /// looking for a scatter that looks right, and the way back from a search
    /// that found nothing is to where it started, not one seed back.
    pub(crate) fn shuffle_noise(&mut self, id: NodeId) {
        let Some(seed) = self.scene.get(id).and_then(|node| node.params()).map(|params| params.int("noise_seed"))
        else {
            return;
        };
        self.edit("Shuffle noise", Some(&format!("noise-seed:{id}")));
        if let Some(params) = self.scene.get_mut(id).and_then(|node| node.params_mut()) {
            params.insert("noise_seed".to_string(), ParamValue::Count(seed % 9999 + 1));
        }
        self.touch();
    }
}

/// What a fresh pattern holds for one of the scatter's parameters.
fn default_of(key: &str) -> Option<ParamValue> {
    pattern::param_spec(key).map(|param| param.default)
}
