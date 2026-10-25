//! Opening and closing the tool.

use super::*;
use crate::app::{App, Status};

impl App {
    /// Open the tool, or put it away if it is out (issue 130).
    pub fn toggle_cut_tool(&mut self) {
        if self.cut_tool.is_some() {
            self.close_cut_tool();
            return;
        }
        // These take the viewport's clicks too, so only one is out.
        if self.measure.active {
            self.toggle_measure();
        }
        self.arrange_tool = None;
        self.mirror_tool = None;
        self.workplane_tool = None;
        self.sketch_tool = None;
        self.cut_tool = Some(CutTool::new());
        self.status = Status::Info("Cut on a face: click the face to cut along; nothing changes until Cut".into());
    }

    pub(crate) fn close_cut_tool(&mut self) {
        self.cut_tool = None;
    }
}
