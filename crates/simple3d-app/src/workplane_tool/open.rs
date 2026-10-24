//! Opening and closing the tool, and keeping what it set up.

use super::*;
use crate::app::{App, Status};

impl App {
    /// Open the tool on a new workplane, or put it away if it is out (issue 125).
    pub fn toggle_workplane_tool(&mut self) {
        if self.workplane_tool.is_some() {
            self.close_workplane_tool();
            return;
        }
        let tool = WorkplaneTool::new(self.scene.settings.free_workplane_name(), self.scene.settings.grid_spacing);
        self.open_workplane_tool(tool);
        self.status = Status::Info("Workplane: click the face to put it on; nothing is kept until Place".into());
    }

    /// Open the tool on workplane `id`, to move it or change its grid.
    pub fn edit_workplane(&mut self, id: WorkplaneId) {
        let Some(plane) = self.scene.settings.workplane(id).cloned() else { return };
        let name = plane.name.clone();
        self.open_workplane_tool(WorkplaneTool::editing(plane));
        self.status = Status::Info(format!("Changing {name}: click another face to move it there"));
    }

    fn open_workplane_tool(&mut self, tool: WorkplaneTool) {
        // These take the viewport's clicks too, so only one is out.
        if self.measure.active {
            self.toggle_measure();
        }
        self.arrange_tool = None;
        self.mirror_tool = None;
        self.cut_tool = None;
        self.workplane_tool = Some(tool);
    }

    pub(crate) fn close_workplane_tool(&mut self) {
        self.workplane_tool = None;
    }

    /// Keep the workplane as one undo step: a new one is added and made active, a changed one replaced.
    pub(crate) fn apply_workplane_tool(&mut self) {
        let Some(tool) = self.workplane_tool.take() else { return };
        if !tool.picked {
            self.workplane_tool = Some(tool);
            return;
        }
        let mut plane = tool.plane;
        let name = plane.name.trim().to_string();
        plane.name = if name.is_empty() { self.scene.settings.free_workplane_name() } else { name };
        match tool.editing.filter(|&id| self.scene.settings.workplane(id).is_some()) {
            Some(id) => {
                self.edit("Change workplane", None);
                plane.id = id;
                self.status = Status::Info(format!("Changed {}", plane.name));
                if let Some(held) = self.scene.settings.workplane_mut(id) {
                    *held = plane;
                }
            }
            None => {
                self.edit("Add workplane", None);
                self.status = Status::Info(format!("Put {} down; new shapes land on it", plane.name));
                let id = self.scene.settings.add_workplane(plane);
                self.scene.settings.active_workplane = Some(id);
            }
        }
    }

    /// Make workplane `id` the one everything works on, or the ground for `None`.
    pub(crate) fn activate_workplane(&mut self, id: Option<WorkplaneId>) {
        let id = id.filter(|&id| self.scene.settings.workplane(id).is_some());
        self.scene.settings.active_workplane = id;
        self.status = Status::Info(match self.workplane() {
            Some(plane) => format!("Working on {}", plane.name),
            None => "Working on the scene's ground".into(),
        });
    }

    /// Remove workplane `id`, as one undo step.
    pub(crate) fn delete_workplane(&mut self, id: WorkplaneId) {
        let Some(name) = self.scene.settings.workplane(id).map(|plane| plane.name.clone()) else { return };
        if self.workplane_tool.as_ref().is_some_and(|tool| tool.editing == Some(id)) {
            self.close_workplane_tool();
        }
        self.edit("Delete workplane", None);
        self.scene.settings.remove_workplane(id);
        self.status = Status::Info(format!("Deleted {name}"));
    }
}
