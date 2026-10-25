//! Opening the editor, putting the sketch on its plane, and keeping or dropping it.

use super::*;
use crate::app::{App, Status};
use simple3d_core::scene::sketch_frame;

impl App {
    /// Open the editor: on the selected sketch node if there is one, else on a new sketch whose plane
    /// the next click picks. Puts it away, dropping what was drawn, if it is out (issue 74).
    pub fn toggle_sketch_tool(&mut self) {
        if self.sketch_tool.is_some() {
            self.close_sketch_tool();
            self.status = Status::Info("Left the sketch without keeping it".into());
            return;
        }
        if let Some(id) = self.primary().filter(|&id| self.scene.node(id).is_sketch()) {
            self.edit_sketch(id);
            return;
        }
        self.open_sketch_tool(SketchTool::new());
        let plane = self.workplane().map_or("the ground".to_string(), |plane| plane.name.clone());
        self.status = Status::Info(format!(
            "Sketch: click an orange origin plane or a face to draw on it, or anywhere else for {plane}"
        ));
    }

    /// Open the editor on sketch node `id`, looking straight at its plane.
    pub fn edit_sketch(&mut self, id: NodeId) {
        let node = self.scene.node(id);
        let (Some(sketch), Some(params)) = (node.sketch(), node.params()) else { return };
        let Some(frame) = sketch_frame(&self.scene, id, &self.evaluated.node_frames) else { return };
        let name = node.name.clone();
        let tool = SketchTool::editing(id, (**sketch).clone(), params.clone(), frame);
        self.open_sketch_tool(tool);
        self.look_at_sketch();
        self.status = Status::Info(format!("Changing {name}; Finish puts it back"));
    }

    fn open_sketch_tool(&mut self, tool: SketchTool) {
        // These take the viewport's clicks too, so only one is out.
        if self.measure.active {
            self.toggle_measure();
        }
        self.arrange_tool = None;
        self.mirror_tool = None;
        self.cut_tool = None;
        self.workplane_tool = None;
        self.sketch_tool = Some(tool);
    }

    /// Put the new sketch on `frame`, drawn on a face of `owner` when there is one, and look at it.
    pub(crate) fn place_sketch(&mut self, frame: Xform, owner: Option<NodeId>) {
        let Some(tool) = self.sketch_tool.as_mut() else { return };
        tool.frame = Some(frame);
        tool.owner = owner;
        self.look_at_sketch();
        self.status = Status::Info(match owner {
            Some(owner) => format!("Sketching on a face of {}", self.scene.node(owner).name),
            None => "Sketching; pick a tool in the window and draw".into(),
        });
    }

    /// Turn the camera to look straight down onto the sketch plane, its X running right.
    pub(crate) fn look_at_sketch(&mut self) {
        let Some(frame) = self.sketch_tool.as_ref().and_then(|tool| tool.frame) else { return };
        let (x, normal) = (frame.axis(0), frame.axis(2));
        let pitch = normal.z.clamp(-1.0, 1.0).asin().to_degrees();
        // Screen right is (-sin yaw, cos yaw, 0); looking straight up or down, X picks the yaw.
        let yaw = if normal.z.abs() > 0.999 { (-x.x).atan2(x.y) } else { normal.y.atan2(normal.x) }.to_degrees();
        self.scene.camera.target = frame.t;
        self.turn_camera_to(yaw, pitch);
    }

    pub(crate) fn close_sketch_tool(&mut self) {
        self.sketch_tool = None;
    }

    /// Keep the sketch as one undo step: a new sketch node, or the edited one changed. Refused with a
    /// reason while there is nothing to make a solid from.
    pub(crate) fn finish_sketch_tool(&mut self) {
        let Some(tool) = self.sketch_tool.as_ref() else { return };
        if let Some(why) = finish_blocked(tool) {
            self.status = Status::Warning(why.into());
            return;
        }
        let Some(tool) = self.sketch_tool.take() else { return };
        if let Some(id) = tool.editing.filter(|&id| self.scene.contains(id)) {
            self.edit("Change sketch", None);
            self.scene.set_sketch(id, tool.sketch, tool.params);
            self.status = Status::Info(format!("Changed {}", self.scene.node(id).name));
            return;
        }
        let Some(frame) = tool.frame else { return };
        self.edit("Add sketch", None);
        let root = self.scene.root();
        let owner = match tool.owner {
            Some(owner) => Some((owner, tool.placing)),
            // A cut with no object under it is a negative part at the top, cutting what is there.
            None => (tool.placing == Placing::Cut).then_some((root, Placing::Cut)),
        };
        let frames = self.evaluated.node_frames.clone();
        let name = if tool.placing == Placing::Cut { "Sketch cut" } else { "Sketch" };
        match self.scene.add_sketch(name, tool.sketch, tool.params, &frame, owner, &frames) {
            Some(id) => {
                self.select_only(id);
                self.status = Status::Info(format!("Added {}", self.scene.node(id).name));
            }
            None => self.status = Status::Warning("The sketch could not be added there".into()),
        }
    }
}

/// Why Finish cannot keep the sketch yet, or `None` when it can.
pub(crate) fn finish_blocked(tool: &SketchTool) -> Option<&'static str> {
    if tool.frame.is_none() {
        Some("Click where to sketch first")
    } else if tool.sketch.regions.is_empty() {
        Some("Pick at least one closed region to make the solid from")
    } else if tool.params.get("feature").map(|v| v.as_u32()) == Some(1) && tool.sketch.axis.is_none() {
        Some("Pick a line of the sketch to revolve about")
    } else {
        None
    }
}
