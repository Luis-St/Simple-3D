//! Running the cut on its own thread, and taking the result.

use super::*;
use crate::app::{App, Status};
use crate::worker::SplitJob;

impl App {
    /// Start cutting, and put the window away. The document is not touched until
    /// the pieces arrive.
    pub fn start_split(&mut self) {
        let Some(tool) = self.split_tool.take() else { return };
        if tool.plan.refusal(tool.bounds).is_some() || !self.scene.contains(tool.target) {
            return;
        }
        // The shape as it is now, to be compared against the shape as it is when
        // the pieces land: a split applied to something that was edited while it
        // was being cut would be pieces of a shape that no longer exists.
        let Some(before) = self.scene.export_subtree(tool.target) else { return };
        let name = self.scene.node(tool.target).name.clone();
        self.settings.last_split = tool.plan.clone();
        self.persist();
        let job = SplitJob::spawn(tool.target, self.active, before, tool.mesh, tool.plan);
        self.status = Status::Info(format!("Splitting {name} into {} cells\u{2026}", job.cells));
        self.split_job = Some(job);
    }

    /// Take the pieces once they are cut, and stand a split where the shape was.
    ///
    /// Everything that could have changed while the cutting ran is checked here
    /// rather than assumed: the document may have been switched, the shape may
    /// have been deleted or edited, and none of those is a reason to change
    /// anything -- the split is dropped and says so.
    pub fn poll_split(&mut self) {
        let Some(job) = &self.split_job else { return };
        let Some(outcome) = job.poll() else { return };
        let job = self.split_job.take().expect("it was there a line ago");
        let Some(pieces) = outcome else {
            self.status = Status::Info("Split stopped; nothing was changed".into());
            return;
        };
        if job.tab != self.active || !self.scene.contains(job.node) {
            self.status = Status::Warning("The split was dropped: the object it was cutting is no longer there".into());
            return;
        }
        if self.scene.export_subtree(job.node).as_ref() != Some(&job.before) {
            self.status = Status::Warning("The split was dropped: the object changed while it was being cut".into());
            return;
        }
        let kind = job.plan.first().kind;
        if pieces.len() < 2 {
            self.status = Status::Warning(format!(
                "{} that size leave the shape in one piece -- try a smaller cell",
                kind.label()
            ));
            return;
        }
        self.edit("Split into smaller pieces", None);
        let Some((name, count)) = self.hold_pieces(job.node, pieces, Some(job.plan.clone())) else {
            self.history.discard_last();
            return;
        };
        self.status =
            Status::Info(format!("Split {name} into {count} {} -- {}", plural_cells(kind, count), self.way_back()));
    }

    pub fn cancel_split_tool(&mut self) {
        self.split_tool = None;
    }
}
