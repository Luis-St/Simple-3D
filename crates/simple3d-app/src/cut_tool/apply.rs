//! Running the cut on its own thread, and standing the halves where the objects were.

use super::*;
use crate::app::{App, Status};
use crate::worker::{CutJob, CutTarget};
use simple3d_core::mesh_data::MeshData;

impl App {
    /// Start cutting and close the window; the document is untouched until the halves arrive.
    pub fn start_cut(&mut self) {
        if self.cut_job.is_some() {
            self.status = Status::Warning("A cut is already running".into());
            return;
        }
        let planned = self.cut_planned();
        let targets: Vec<CutTarget> = planned
            .iter()
            .filter_map(|planned| {
                // Snapshot to compare when the halves land; halves of an edited object would be of one that is gone.
                let before = self.scene.export_subtree(planned.node)?;
                Some(CutTarget { node: planned.node, before, mesh: planned.mesh.clone(), plane: planned.plane })
            })
            .collect();
        if targets.is_empty() {
            return;
        }
        self.cut_tool = None;
        let count = targets.len();
        self.status = Status::Info(format!("Cutting {count} object{} in two\u{2026}", crate::ui::plural(count)));
        self.cut_job = Some(CutJob::spawn(self.active, targets));
    }

    /// Take the halves and stand them where each object was, as one undo step. Objects deleted or
    /// edited meanwhile are left as they are, and a switched document drops the cut.
    pub fn poll_cut(&mut self) {
        let Some(job) = &self.cut_job else { return };
        let Some(outcome) = job.poll() else { return };
        let job = self.cut_job.take().expect("it was there a line ago");
        let Some(halves) = outcome else {
            self.status = Status::Info("Cut stopped; nothing was changed".into());
            return;
        };
        if job.tab != self.active {
            self.status = Status::Warning("The cut was dropped: it was for another document".into());
            return;
        }
        let landing: Vec<(NodeId, [Mesh; 2])> = job
            .targets
            .iter()
            .zip(halves)
            .filter(|(target, _)| self.scene.export_subtree(target.node).as_ref() == Some(&target.before))
            .filter_map(|(target, halves)| Some((target.node, halves?)))
            .collect();
        if landing.is_empty() {
            self.status = Status::Warning("Nothing was cut: the plane left every object whole, or they changed".into());
            return;
        }
        self.edit("Cut on a face", None);
        let mut splits = Vec::new();
        for (node, halves) in landing {
            let parent = self.scene.node(node).parent;
            if parent.is_some_and(|parent| self.scene.node(parent).is_split()) {
                splits.push(self.halve_piece(node, halves));
            } else if self.hold_pieces(node, halves.into(), None).is_some() {
                splits.extend(self.primary());
            }
        }
        splits.dedup();
        let count = splits.len();
        self.selection = splits;
        self.on_selection_changed();
        self.status =
            Status::Info(format!("Cut {count} object{} in two -- {}", crate::ui::plural(count), self.way_back()));
    }

    /// Stand a split's piece's halves in its place, where it stood, and number the split's pieces
    /// again. The split no longer holds its tiling's cells, so it forgets the tiling. Returns the split.
    fn halve_piece(&mut self, piece: NodeId, halves: [Mesh; 2]) -> NodeId {
        let node = self.scene.node(piece).clone();
        let split = node.parent.expect("a piece is in its split");
        let index = self.scene.node(split).children.iter().position(|&c| c == piece).unwrap_or(0);
        self.scene.remove(piece);
        for (offset, half) in halves.into_iter().enumerate() {
            let id = self.scene.add_mesh_named(
                format!("{} {}", node.name, offset + 1),
                MeshData::new(half),
                split,
                index + offset,
            );
            if let Some(added) = self.scene.get_mut(id) {
                (added.position, added.rotation, added.scale) = (node.position, node.rotation, node.scale);
            }
        }
        if let Some(simple3d_core::scene::Body::Split { plan, .. }) = self.scene.get_mut(split).map(|n| &mut n.body) {
            *plan = None;
        }
        self.number_pieces(split);
        split
    }

    /// Name a split's pieces "<split> Piece n" in order, leaving any renamed by hand alone.
    fn number_pieces(&mut self, split: NodeId) {
        let name = self.scene.node(split).name.clone();
        let prefix = format!("{name} Piece ");
        let numbered = |piece: &str| {
            piece.strip_prefix(&prefix).is_some_and(|rest| {
                rest.split(' ').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
            })
        };
        for (index, piece) in self.scene.node(split).children.clone().into_iter().enumerate() {
            if let Some(node) = self.scene.get_mut(piece).filter(|node| numbered(&node.name)) {
                node.name = format!("{prefix}{}", index + 1);
            }
        }
    }
}
