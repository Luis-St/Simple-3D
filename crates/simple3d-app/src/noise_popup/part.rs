//! The parts a scatter is built from, and the ids of the controls that add, move and drop them.

use simple3d_core::primitive::{Params, ParamsExt};

/// The parts a scatter is built from, in builder order (issue 79).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    /// Moved off where the rule put it, along each axis.
    Nudge,
    /// Turned in place about one axis; one per axis.
    Turn(usize),
    /// Made bigger or smaller.
    Size,
}

/// The order "+ Turn" adds axes in: Z first, as a plank lies askew on a floor.
pub(super) const TURN_ORDER: [usize; 3] = [2, 0, 1];

/// The axes' names, by index.
pub(super) const AXES: [&str; 3] = ["X", "Y", "Z"];

impl Part {
    pub(crate) const ALL: [Part; 5] = [Part::Nudge, Part::Turn(0), Part::Turn(1), Part::Turn(2), Part::Size];

    /// How many parts there are, for what the builder keeps open.
    pub(crate) const COUNT: usize = Part::ALL.len();

    pub(super) fn index(self) -> usize {
        match self {
            Part::Nudge => 0,
            Part::Turn(axis) => 1 + axis.min(2),
            Part::Size => 4,
        }
    }

    pub(super) fn name(self) -> String {
        match self {
            Part::Nudge => "Nudge".to_string(),
            Part::Turn(axis) => format!("Turn {}", AXES[axis.min(2)]),
            Part::Size => "Size".to_string(),
        }
    }

    pub(super) fn hover(self) -> &'static str {
        match self {
            Part::Nudge => "Move each copy a little off where the rule puts it",
            Part::Turn(_) => "Turn each copy a little askew where it stands, about one axis. Add one for each axis.",
            Part::Size => "Make each copy a little bigger or smaller",
        }
    }

    /// The numbers it is made of, reset when it is removed.
    pub(super) fn keys(self) -> &'static [&'static str] {
        match self {
            Part::Nudge => &["noise_x", "noise_y", "noise_z"],
            Part::Turn(0) => &["noise_turn_x"],
            Part::Turn(1) => &["noise_turn_y"],
            Part::Turn(_) => &["noise_turn_z"],
            Part::Size => &["noise_scale"],
        }
    }

    pub(super) fn in_use(self, params: &Params) -> bool {
        self.keys().iter().any(|key| params.num(key).abs() > 1e-9)
    }
}

/// The id of the chip adding `part` in the builder under `scope` (tool and window can both be open).
pub(crate) fn part_id(scope: &str, part: Part) -> egui::Id {
    egui::Id::new(("noise-add-part", scope.to_string(), part.index()))
}

/// The id of the chip adding a turn about the next free axis.
pub(crate) fn add_turn_id(scope: &str) -> egui::Id {
    egui::Id::new(("noise-add-turn", scope.to_string()))
}

/// The id of the chip moving a turn onto `axis`.
pub(crate) fn turn_axis_id(scope: &str, from: usize, to: usize) -> egui::Id {
    egui::Id::new(("noise-turn-axis", scope.to_string(), from, to))
}

/// The id of the cross removing `part`.
pub(crate) fn drop_part_id(scope: &str, part: Part) -> egui::Id {
    egui::Id::new(("noise-drop-part", scope.to_string(), part.index()))
}

/// The id of the button stepping the seed.
pub(crate) fn shuffle_id(scope: &str) -> egui::Id {
    egui::Id::new(("noise-shuffle", scope.to_string()))
}
