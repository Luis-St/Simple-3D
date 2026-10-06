//! Carrying the plan out, as one undo step.

use super::*;
use crate::app::{App, Status};
use std::collections::HashMap;

impl App {
    /// Move the objects and make the copies the plan says, then close the tool. The copies go right
    /// after their originals, in their groups, and everything placed is left selected.
    pub fn apply_arrange(&mut self) {
        let Some(mode) = self.arrange_tool.as_ref().map(|tool| tool.mode) else { return };
        let placements = match self.arrange_plan() {
            Ok(placements) => placements,
            Err(why) => {
                self.status = Status::Warning(why);
                return;
            }
        };
        // Not every placed original: one already where the plan puts it is not said to move.
        let moved = moving(&placements);
        let primary = self.primary();
        self.edit(
            match mode {
                Arrange::Align => "Align",
                Arrange::Distribute => "Distribute",
                Arrange::Path => "Distribute along a path",
            },
            None,
        );
        // Each copy goes after the last one made of the same original, so they read in path order.
        let mut last: HashMap<NodeId, NodeId> = HashMap::new();
        let mut placed = Vec::new();
        let mut copies = 0;
        for placement in placements {
            let id = match placement.copy {
                true => {
                    let after = last.get(&placement.source).copied().unwrap_or(placement.source);
                    let Some(copy) = self.scene.duplicate_after(placement.source, after) else { continue };
                    copies += 1;
                    copy
                }
                false => placement.source,
            };
            if let Some(node) = self.scene.get_mut(id) {
                node.position = placement.position;
                node.rotation = placement.rotation;
            }
            last.insert(placement.source, id);
            placed.push(id);
        }
        // A copied pattern is sized like a duplicated one.
        self.size_fresh_patterns();
        // The last selected stays last, since the tool and the Properties panel measure from it.
        if let Some(at) = primary.and_then(|primary| placed.iter().position(|&id| id == primary)) {
            let primary = placed.remove(at);
            placed.push(primary);
        }
        self.selection = placed;
        self.on_selection_changed();
        self.arrange_tool = None;
        let objects = format!("{moved} object{}", crate::ui::plural(moved));
        self.status = Status::Info(match (mode, copies) {
            (Arrange::Align, _) => format!("Aligned {objects}"),
            (Arrange::Distribute, _) => format!("Distributed {objects}"),
            (Arrange::Path, 0) => format!("Spread {objects} along the path"),
            (Arrange::Path, _) => {
                let made = format!("{copies} cop{}", if copies == 1 { "y" } else { "ies" });
                match moved {
                    0 => format!("Spread {made} along the path"),
                    _ => format!("Spread {objects} and {made} along the path"),
                }
            }
        });
    }
}
