//! Cut, copy, paste, duplicate, delete and group.

use super::*;
use simple3d_core::clipboard::{self};
use simple3d_core::scene::NodeId;

impl App {
    pub(super) fn after_history(&mut self, message: &str) {
        self.selection.retain(|id| self.scene.contains(*id));
        self.fields.clear();
        self.rename = None;
        self.dirty = true;
        self.status = Status::Info(message.into());
    }

    pub(super) fn copy_selection(&mut self, cut: bool) {
        let Some(clip) = clipboard::copy(&self.scene, &self.selection) else {
            self.status = Status::Info("Nothing to copy".into());
            return;
        };
        let count = clip.nodes.len();
        let what = match clip.nodes.as_slice() {
            [only] => only.name.clone(),
            nodes => format!("{} nodes", nodes.len()),
        };
        self.clipboard_text = Some(format!("Simple 3D: {what}"));
        self.clipboard = Some(clip);
        if cut {
            // Cut is an undoable step of its own, so it cannot lose work even if
            // the user never pastes (spec section 8.1).
            self.edit("Cut", None);
            let doomed: Vec<NodeId> = self.top_level_selection();
            for id in doomed {
                self.scene.remove(id);
            }
            self.clear_selection();
            self.status = Status::Info(format!("Cut {count} node{}", if count == 1 { "" } else { "s" }));
        } else {
            self.status = Status::Info(format!("Copied {count} node{}", if count == 1 { "" } else { "s" }));
        }
    }

    pub(super) fn paste(&mut self) {
        let Some(clip) = self.clipboard.clone() else {
            self.status = Status::Info("The clipboard is empty".into());
            return;
        };
        self.edit("Paste", None);
        let target = self.primary();
        let created = clipboard::paste(&mut self.scene, &clip, target);
        if created.is_empty() {
            self.status = Status::Warning("Nothing could be pasted".into());
            return;
        }
        // A pattern pasted into can now be measured (issue 67).
        self.size_fresh_patterns();
        // Left selected, so a nudge or a drag can follow immediately.
        self.selection = created;
        self.on_selection_changed();
        self.status = Status::Info("Pasted".into());
    }

    pub(super) fn duplicate(&mut self) {
        let targets = self.top_level_selection();
        if targets.is_empty() {
            self.status = Status::Info("Nothing to duplicate".into());
            return;
        }
        // A separate action from copy and paste: it does not disturb the
        // clipboard (spec section 8.1).
        self.edit("Duplicate", None);
        let mut created = Vec::new();
        for id in targets {
            if let Some(copy) = self.scene.duplicate(id) {
                created.push(copy);
            }
        }
        self.size_fresh_patterns();
        if !created.is_empty() {
            self.selection = created;
            self.on_selection_changed();
        }
        self.status = Status::Info("Duplicated".into());
    }

    pub(super) fn delete_selection(&mut self) {
        let targets: Vec<NodeId> =
            self.top_level_selection().into_iter().filter(|id| *id != self.scene.root()).collect();
        if targets.is_empty() {
            self.status = Status::Info("Nothing to delete".into());
            return;
        }
        // Deleting a group is two different actions wearing one word: the
        // children can go with it, or stay. Rather than guess, or open a dialog
        // over the model, the outliner asks in place.
        let holds_children = |id: &NodeId| {
            (self.scene.node(*id).is_group() || self.scene.node(*id).is_split())
                && !self.scene.node(*id).children.is_empty()
        };
        if targets.iter().any(holds_children) {
            self.pending_delete = Some(targets);
            return;
        }
        self.delete_now(&targets, false);
    }

    /// How many nodes the pending deletion would take with it, if the children
    /// go too.
    pub fn pending_delete_count(&self) -> usize {
        let Some(targets) = &self.pending_delete else { return 0 };
        targets.iter().map(|id| 1 + self.scene.descendants(*id).len()).sum()
    }

    /// Carry out the deletion the outliner asked about. `promote` keeps the
    /// children by moving them up into the group's own place first.
    pub fn confirm_delete(&mut self, promote: bool) {
        let Some(targets) = self.pending_delete.take() else { return };
        self.delete_now(&targets, promote);
    }

    pub fn cancel_delete(&mut self) {
        if self.pending_delete.take().is_some() {
            self.status = Status::Info("Nothing deleted".into());
        }
    }

    pub(super) fn delete_now(&mut self, targets: &[NodeId], promote: bool) {
        self.edit("Delete", None);
        let mut promoted = 0;
        for id in targets {
            if promote {
                // Into the group's own slot, in order, so the tree reads the
                // same afterwards minus one level of nesting.
                let node = self.scene.node(*id);
                let children = node.children.clone();
                if let Some(parent) = node.parent {
                    let at = self.scene.node(parent).children.iter().position(|c| c == id).unwrap_or(0);
                    for (offset, child) in children.iter().enumerate() {
                        if self.scene.reparent(*child, parent, at + offset).is_ok() {
                            promoted += 1;
                        }
                    }
                }
            }
            self.scene.remove(*id);
        }
        self.clear_selection();
        let removed = targets.len();
        self.status = Status::Info(if promote {
            format!("Deleted {removed} group{}, kept {promoted} child{}", plural(removed), children_plural(promoted))
        } else {
            format!("Deleted {removed} node{}", plural(removed))
        });
    }

    pub(super) fn group_selection(&mut self) {
        if self.selection.is_empty() {
            self.status = Status::Info("Select something to group".into());
            return;
        }
        self.edit("Group", None);
        match self.scene.group_selection(&self.selection.clone()) {
            Some(group) => {
                self.select_only(group);
                self.status = Status::Info("Grouped".into());
            }
            None => self.status = Status::Warning("That selection cannot be grouped".into()),
        }
    }
}
