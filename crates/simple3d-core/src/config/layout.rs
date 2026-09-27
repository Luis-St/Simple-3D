//! Where the panels sit, and moving one between docks.

use serde::{Deserialize, Serialize};

/// Where a new shape lands (spec section 8.1 leaves this to the application), as one visible choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// 0, 0, 0.
    #[default]
    Origin,
    /// Where Shift+right-click put the 3D cursor; the origin until placed.
    Cursor,
    /// What the camera is looking at, rounded to the step.
    ViewCentre,
    /// Clear of the selection along +X, so the new shape is not inside it.
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

/// A movable panel. The viewport, rail, menu bar and status bar are fixed frame.
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

/// Where each panel lives, which are rolled up, and whether the docks show. Hiding is one flag, so
/// restoring is exact.
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

    /// Move a panel to `side` at `index`; within its own dock the index is read after lifting it out.
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

    /// The panel taking a dock's leftover height: the last not rolled up, or `None` if all are.
    pub fn filler(&self, side: Side) -> Option<Panel> {
        self.panels(side).iter().rev().find(|p| !self.is_collapsed(**p)).copied()
    }

    /// Repair a layout from disk: a panel named twice or missing is returned to its starting dock, so
    /// none becomes unreachable.
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
