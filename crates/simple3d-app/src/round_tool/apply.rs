//! Making the draft an edit of the document.

use super::*;

impl App {
    /// Keep the picks' edits on their objects as one undo step, and close the tool.
    pub(crate) fn apply_round_tool(&mut self) {
        self.sync_round_draft();
        let Some(tool) = self.round_tool.as_mut() else { return };
        if tool.is_empty() {
            self.status = Status::Info("Pick an edge or a corner first".into());
            return;
        }
        if tool.draft.is_empty() {
            self.status = Status::Warning("None of the picks belong to an object of this document".into());
            return;
        }
        let rounding = tool.kind == Kind::Round;
        let (edges, corners) = (tool.edges.len(), tool.corners.len() + tool.joints.len());
        // Kept in the edit, so a smaller size later treats them, but making nothing at this one.
        let skipped = tool.too_small();
        // The snapshot is taken without the draft, so the step undoes it; the draft then stays as the edit.
        let draft = self.lift_round_draft();
        if let Some(tool) = self.round_tool.as_mut() {
            (tool.draft, tool.taken) = (Vec::new(), Vec::new());
        }
        self.edit(if rounding { "Round edges" } else { "Bevel edges" }, None);
        self.put_round_draft_back(&draft);
        self.round_tool = None;
        // A rounding shared by objects of the scene is held by the scene, shown with nothing selected.
        match draft.added.first() {
            Some(&(holder, _)) if holder != self.scene.root() => self.select_only(holder),
            _ => self.selection.clear(),
        }
        let mut text = format!(
            "{} {edges} edge{} and {corners} corner{}",
            if rounding { "Rounded" } else { "Bevelled" },
            if edges == 1 { "" } else { "s" },
            if corners == 1 { "" } else { "s" }
        );
        if skipped > 0 {
            text.push_str(&format!("; {skipped} too small for that size were left as they are"));
        }
        self.status = Status::Info(text);
    }
}
