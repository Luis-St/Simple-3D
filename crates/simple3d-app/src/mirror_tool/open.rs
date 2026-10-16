//! Opening the tool, keeping it in step with the document, and what it would do.

use super::*;
use crate::app::{App, Status};
use simple3d_core::scene::Mirrored;
use simple3d_core::xform::Xform;

impl App {
    /// Open the tool on the selection, or put it away if it is out (issue 122).
    pub fn toggle_mirror_tool(&mut self) {
        if self.mirror_tool.is_some() {
            self.close_mirror_tool();
            return;
        }
        // These take the viewport's clicks too, so only one is out.
        if self.measure.active {
            self.toggle_measure();
        }
        self.arrange_tool = None;
        let tool = MirrorTool::on(self.mirror_targets());
        self.status = Status::Info(match tool.choosing {
            true => "Mirror: select the objects to mirror, then choose what to mirror them across".into(),
            false => "Mirror: nothing changes until Apply".into(),
        });
        self.mirror_tool = Some(tool);
    }

    pub(crate) fn close_mirror_tool(&mut self) {
        self.mirror_tool = None;
    }

    /// The top of the selection, without the document's root.
    fn mirror_targets(&self) -> Vec<NodeId> {
        let root = self.scene.root();
        self.top_level_selection().into_iter().filter(|&id| id != root).collect()
    }

    /// Follow the document: while the objects follow the selection they are the selection; otherwise
    /// objects deleted meanwhile drop out.
    pub(crate) fn refresh_mirror_tool(&mut self) {
        let follows = self.mirror_tool.as_ref().is_some_and(MirrorTool::follows_selection);
        let targets = if follows { Some(self.mirror_targets()) } else { None };
        let Some(tool) = self.mirror_tool.as_mut() else { return };
        match targets {
            Some(targets) => tool.targets = targets,
            None => tool.targets.retain(|&id| self.scene.contains(id)),
        }
        // Across an axis nothing is left to pick, so the tool is choosing exactly while nothing is chosen.
        if tool.targets.is_empty() || tool.by == MirrorBy::Axis {
            tool.choosing = tool.targets.is_empty();
        }
    }

    /// Hand the viewport back to the selection to change the objects, starting from the ones chosen,
    /// or keep the selection as the objects.
    pub(crate) fn toggle_mirror_choosing(&mut self) {
        let Some(tool) = self.mirror_tool.as_mut() else { return };
        tool.choosing = !tool.choosing;
        if tool.choosing {
            let targets = tool.targets.clone();
            self.selection = targets;
            self.on_selection_changed();
        }
    }

    /// Each object and where its mirror image stands, or why there is none yet.
    pub(crate) fn mirror_plan(&self) -> Result<Vec<(NodeId, Mirrored)>, String> {
        let Some(tool) = self.mirror_tool.as_ref() else { return Ok(Vec::new()) };
        if tool.targets.is_empty() {
            return Err("Select the objects to mirror".into());
        }
        let (point, normal) = tool.plane()?;
        Ok(tool
            .targets
            .iter()
            .filter_map(|&id| {
                let node = self.scene.get(id)?;
                let parent = self.evaluated.node_frames.get(&id).copied().unwrap_or(Xform::IDENTITY);
                Some((id, node.mirrored(parent, point, normal)))
            })
            .collect())
    }
}
