//! Carrying the mirror out, as one undo step.

use crate::app::{App, Status};

impl App {
    /// Mirror the objects, or make mirrored copies beside them, then close the tool. Everything
    /// mirrored is left selected.
    pub fn apply_mirror(&mut self) {
        let Some(keep) = self.mirror_tool.as_ref().map(|tool| tool.keep) else { return };
        let plan = match self.mirror_plan() {
            Ok(plan) => plan,
            Err(why) => {
                self.status = Status::Warning(why);
                return;
            }
        };
        self.edit(if keep { "Mirror copies" } else { "Mirror" }, None);
        let mut placed = Vec::new();
        for (id, mirrored) in plan {
            // A copy goes right after its original, in the same group, so the parent frame still holds.
            let target = match keep {
                true => match self.scene.duplicate(id) {
                    Some(copy) => copy,
                    None => continue,
                },
                false => id,
            };
            if let Some(node) = self.scene.get_mut(target) {
                node.position = mirrored.position;
                node.rotation = mirrored.rotation;
                node.scale = mirrored.scale;
            }
            placed.push(target);
        }
        // A copied pattern is sized like a duplicated one.
        self.size_fresh_patterns();
        let count = placed.len();
        self.selection = placed;
        self.on_selection_changed();
        self.mirror_tool = None;
        let objects = format!("{count} object{}", crate::ui::plural(count));
        self.status = Status::Info(match keep {
            true => format!("Made mirrored copies of {objects}"),
            false => format!("Mirrored {objects}"),
        });
    }
}
