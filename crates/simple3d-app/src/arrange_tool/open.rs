//! Opening the tool on the selection, keeping it in step with the document, and closing it.

use super::*;
use crate::app::{App, Status};
use simple3d_core::scene::{Body, Scene};
use simple3d_core::xform::Xform;

impl ArrangeTool {
    /// The tool on `targets`, set up for them: one object has nothing to line up with, so it starts
    /// along a path, the only way it can be spread.
    pub fn on(targets: Vec<NodeId>, key: Option<NodeId>) -> ArrangeTool {
        let alone = targets.len() == 1;
        ArrangeTool {
            mode: if alone { Arrange::Path } else { Arrange::Align },
            count: if alone { 5 } else { targets.len() },
            targets,
            key,
            align: [None, None, None],
            to_key: false,
            axis: 0,
            spacing: Spacing::Gaps,
            gap: 1.0,
            source: PathSource::Edges,
            runs: Vec::new(),
            whole_run: true,
            points: Vec::new(),
            closed: false,
            follow: false,
            landing: None,
            onto: OntoAt::Centre,
            turn: true,
            lift: 0.0,
        }
    }
}

impl ArrangeTool {
    /// The group whose contents are lined up or distributed: the tool was opened on that one group
    /// alone, and one object has nothing to line up with. Along a path or onto a face the group is one
    /// object.
    pub fn inside(&self, scene: &Scene) -> Option<NodeId> {
        let [only] = self.targets[..] else { return None };
        let node = scene.get(only)?;
        let holds = matches!(node.body, Body::Group { .. }) && node.children.len() >= 2;
        (!matches!(self.mode, Arrange::Path | Arrange::OntoFace) && holds).then_some(only)
    }

    /// What the tool arranges now: the objects it was opened on, or that lone group's contents.
    pub fn arranged(&self, scene: &Scene) -> Vec<NodeId> {
        match self.inside(scene) {
            Some(group) => scene.node(group).children.clone(),
            None => self.targets.clone(),
        }
    }
}

impl App {
    /// Open the tool on the selection, or put it away if it is out (issue 70).
    pub fn toggle_arrange_tool(&mut self) {
        if self.arrange_tool.is_some() {
            self.close_arrange_tool();
            return;
        }
        let root = self.scene.root();
        let targets: Vec<NodeId> = self.top_level_selection().into_iter().filter(|&id| id != root).collect();
        if targets.is_empty() {
            self.status = Status::Warning("Select the objects to align or distribute".into());
            return;
        }
        // These take the viewport's clicks too, so only one is out.
        if self.measure.active {
            self.toggle_measure();
        }
        self.mirror_tool = None;
        self.cut_tool = None;
        self.workplane_tool = None;
        self.sketch_tool = None;
        let key = self.primary().filter(|id| targets.contains(id));
        let mut tool = ArrangeTool::on(targets, key);
        // A lone group is most likely meant as what it holds, which is what lining up needs.
        let alone = tool.mode;
        tool.mode = Arrange::Align;
        if tool.inside(&self.scene).is_none() {
            tool.mode = alone;
        }
        self.arrange_tool = Some(tool);
        self.status = Status::Info("Align and distribute: nothing moves until Apply".into());
    }

    /// Open the tool on the selection straight in its "onto a face" mode (issue 118), or switch an
    /// open one to it.
    pub fn open_onto_face(&mut self) {
        if self.arrange_tool.is_none() {
            self.toggle_arrange_tool();
        }
        if let Some(tool) = self.arrange_tool.as_mut() {
            tool.mode = Arrange::OntoFace;
            self.status = Status::Info("Onto a face: click the face to put the selection on".into());
        }
    }

    pub(crate) fn close_arrange_tool(&mut self) {
        self.arrange_tool = None;
    }

    /// Follow the document: objects deleted meanwhile drop out, and with none left the tool closes.
    pub(crate) fn refresh_arrange_tool(&mut self) {
        let Some(tool) = self.arrange_tool.as_mut() else { return };
        tool.targets.retain(|&id| self.scene.contains(id));
        if tool.key.is_some_and(|id| !self.scene.contains(id)) {
            tool.key = None;
        }
        if tool.targets.is_empty() {
            self.arrange_tool = None;
            self.status = Status::Warning("The objects being arranged are no longer there".into());
        }
    }

    /// The tool's objects as the plan sees them: those with a shape evaluated, in tree order.
    pub(crate) fn arrange_subjects(&self) -> Vec<Subject> {
        let Some(tool) = self.arrange_tool.as_ref() else { return Vec::new() };
        tool.arranged(&self.scene)
            .iter()
            .filter_map(|&id| {
                let bounds = *self.evaluated.node_world_bounds.get(&id)?;
                let node = self.scene.get(id)?;
                let parent = self.evaluated.node_frames.get(&id).copied().unwrap_or(Xform::IDENTITY);
                // Only putting onto a face needs the body itself.
                let body = match tool.mode {
                    Arrange::OntoFace => {
                        self.evaluated.node_meshes.get(&id).map(|mesh| (mesh.clone(), Xform::IDENTITY)).or_else(|| {
                            Some((self.evaluated.group_meshes.get(&id)?.clone(), *self.evaluated.node_frames.get(&id)?))
                        })
                    }
                    _ => None,
                };
                Some(Subject { id, bounds, parent, position: node.position, rotation: node.rotation, body })
            })
            .collect()
    }

    /// What the tool would do now, or why it would do nothing.
    pub(crate) fn arrange_plan(&self) -> Result<Vec<Placement>, String> {
        let Some(tool) = self.arrange_tool.as_ref() else { return Ok(Vec::new()) };
        plan(tool, &self.arrange_subjects())
    }
}
