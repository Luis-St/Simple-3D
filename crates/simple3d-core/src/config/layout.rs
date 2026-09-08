//! Where the panels sit, and moving one from dock to dock.

use serde::{Deserialize, Serialize};

/// Where a new shape lands (spec section 8.1 leaves this to the application).
///
/// The origin used to be the only answer, with the 3D cursor as an override
/// nobody could see they had. Making it a named choice puts the four reasonable
/// answers in one control, and says which one is in force.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// 0, 0, 0.
    #[default]
    Origin,
    /// Where Shift+right-click put the 3D cursor; the origin until it is placed.
    Cursor,
    /// What the camera is looking at, rounded to the step.
    ViewCentre,
    /// Clear of what is selected, along +X, so the new shape does not land
    /// inside it.
    BesideSelection,
}

impl Placement {
    pub const ALL: [Placement; 4] =
        [Placement::Origin, Placement::Cursor, Placement::ViewCentre, Placement::BesideSelection];

    pub fn label(self) -> &'static str {
        match self {
            Placement::Origin => "Origin",
            Placement::Cursor => "3D cursor",
            Placement::ViewCentre => "View centre",
            Placement::BesideSelection => "Beside selection",
        }
    }
}

/// A dock: which side of the window a panel sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub const ALL: [Side; 2] = [Side::Left, Side::Right];

    pub fn label(self) -> &'static str {
        match self {
            Side::Left => "Left dock",
            Side::Right => "Right dock",
        }
    }
}

/// A movable panel. Everything in a dock is one of these; the viewport, the
/// tool rail, the menu bar and the status bar are the window's frame and do not
/// move, because a rail that can end up somewhere else is a rail you have to
/// look for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Panel {
    Outliner,
    Primitives,
    Properties,
}

impl Panel {
    pub const ALL: [Panel; 3] = [Panel::Outliner, Panel::Primitives, Panel::Properties];

    pub fn label(self) -> &'static str {
        match self {
            Panel::Outliner => "Outliner",
            Panel::Primitives => "Primitives",
            Panel::Properties => "Properties",
        }
    }
}

/// Where each panel lives, which of them are rolled up to their header, and
/// whether the docks are showing at all.
///
/// Hiding is one flag rather than a saved copy of the arrangement: `Tab` cannot
/// lose a layout it never touched, so restoring it is exact by construction
/// rather than by care.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub left: Vec<Panel>,
    pub right: Vec<Panel>,
    pub collapsed: Vec<Panel>,
    pub docks_hidden: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            left: vec![Panel::Outliner, Panel::Primitives],
            right: vec![Panel::Properties],
            collapsed: Vec::new(),
            docks_hidden: false,
        }
    }
}

impl Layout {
    pub fn side_of(&self, panel: Panel) -> Side {
        if self.left.contains(&panel) {
            Side::Left
        } else {
            Side::Right
        }
    }

    pub fn panels(&self, side: Side) -> &[Panel] {
        match side {
            Side::Left => &self.left,
            Side::Right => &self.right,
        }
    }

    pub(super) fn panels_mut(&mut self, side: Side) -> &mut Vec<Panel> {
        match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
        }
    }

    /// Move a panel to `side`, at `index` among the panels already there.
    /// Moving a panel within its own dock is a reorder, and the index is read
    /// after the panel has been lifted out, so dragging something down one place
    /// puts it one place down rather than back where it started.
    pub fn move_to(&mut self, panel: Panel, side: Side, index: usize) {
        for list in [&mut self.left, &mut self.right] {
            list.retain(|p| *p != panel);
        }
        let list = self.panels_mut(side);
        let index = index.min(list.len());
        list.insert(index, panel);
    }

    pub fn is_collapsed(&self, panel: Panel) -> bool {
        self.collapsed.contains(&panel)
    }

    pub fn toggle_collapsed(&mut self, panel: Panel) {
        if let Some(at) = self.collapsed.iter().position(|p| *p == panel) {
            self.collapsed.remove(at);
        } else {
            self.collapsed.push(panel);
        }
    }

    /// Which panel in a dock takes the leftover height: the last one that is not
    /// rolled up. `None` when every panel there is collapsed, in which case the
    /// dock is nothing but headers.
    pub fn filler(&self, side: Side) -> Option<Panel> {
        self.panels(side).iter().rev().find(|p| !self.is_collapsed(**p)).copied()
    }

    /// Put a layout read from disk back into a state the window can draw.
    ///
    /// A settings file from another version -- or one edited by hand -- can name
    /// a panel twice or leave one out entirely. A panel that appears nowhere
    /// would be unreachable, with no menu entry able to bring it back, so it is
    /// returned to the dock it starts in rather than lost.
    pub fn repair(&mut self) {
        let mut seen: Vec<Panel> = Vec::new();
        for list in [&mut self.left, &mut self.right] {
            list.retain(|p| {
                if seen.contains(p) {
                    false
                } else {
                    seen.push(*p);
                    true
                }
            });
        }
        let default = Layout::default();
        for panel in Panel::ALL {
            if !seen.contains(&panel) {
                let side = default.side_of(panel);
                self.panels_mut(side).push(panel);
            }
        }
        self.collapsed.retain(|p| Panel::ALL.contains(p));
        self.collapsed.dedup();
    }
}
