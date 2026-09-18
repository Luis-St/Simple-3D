//! Putting what was found into the document, and putting the tool away.

use super::*;
use crate::app::{App, Status};
use simple3d_core::primitive::{self, ParamValue, Params};
use simple3d_core::scene::{Body, GroupOp};
use simple3d_geom::reassemble::{Part, Shape};

impl App {
    /// Whether there is an answer on screen that is worth putting in the
    /// document: one the numbers asked for, about a mesh that is still there,
    /// and that is not simply the mesh over again.
    pub(crate) fn reassemble_ready(&self) -> bool {
        let Some(tool) = self.reassemble_tool.as_ref() else { return false };
        let Some(found) = tool.found.as_ref() else { return false };
        found.plan == tool.plan
            && self.scene.contains(tool.target)
            && !found.assembly.is_nothing()
            && found.assembly.objects() > 0
    }

    /// Stand the objects that were found where the mesh stood (issue 108).
    ///
    /// The mesh's own node becomes the group holding them, rather than being
    /// replaced by a fresh one: it keeps its name, its place in the tree, its
    /// transform and its colour, so nothing that pointed at it has to be told
    /// anything.
    ///
    /// A group even for a mesh that turned out to be one box. Folding the box
    /// into the node itself would mean composing the fit's own frame into the
    /// node's transform, and a node carries a scale: a rotated fit composed
    /// with a scale that is not the same on every axis is not a position, a
    /// rotation and a scale any more, and there would be nowhere to put the
    /// difference. The group keeps the two apart, which is what a group is for.
    pub fn apply_reassemble(&mut self) {
        if !self.reassemble_ready() {
            return;
        }
        let tool = self.reassemble_tool.take().expect("it was there a line ago");
        if let Some(job) = &tool.job {
            job.cancel();
        }
        let found = tool.found.expect("it was there a line ago");
        let name = self.scene.node(tool.target).name.clone();
        let assembly = &found.assembly;
        self.edit("Reassemble into objects", None);
        if !self.scene.make_group(tool.target, GroupOp::Union) {
            self.history.discard_last();
            self.status = Status::Warning(format!("{name} could not be taken apart"));
            return;
        }
        // One group in the whole assembly with nothing left over *is* the node
        // standing over it, and wrapping it in a second group would be a row of
        // the outliner that holds one row and says nothing.
        let flat = assembly.groups.len() == 1 && assembly.rest.is_none();
        let mut at = 0;
        for group in &assembly.groups {
            let holder = if flat || group.len() == 1 {
                tool.target
            } else {
                let holder = self.scene.add_group(GroupOp::Union, tool.target, at);
                at += 1;
                holder
            };
            let mut inside = if holder == tool.target { at } else { 0 };
            for &index in group {
                self.place_part(&assembly.parts[index], &name, holder, inside);
                inside += 1;
            }
            if holder == tool.target {
                at = inside;
            }
            self.collapsed.remove(&holder);
        }
        if let Some(rest) = &assembly.rest {
            let mesh = simple3d_core::mesh_data::MeshData::new(rest.clone());
            self.scene.add_mesh(&format!("{name} Remainder"), mesh, tool.target, at);
        }
        self.collapsed.remove(&tool.target);
        self.select_only(tool.target);
        self.touch();
        self.settings.last_reassemble = tool.plan;
        self.persist();
        // Not `way_back`, which names the command that joins a split's pieces
        // back together: a reassembly leaves no split, and the way back from
        // one is the undo step it has just taken.
        let undo = self.keymap.shortcut_text(simple3d_core::keymap::Command::Undo);
        let back =
            if undo.is_empty() { "undo puts the mesh back".to_string() } else { format!("{undo} puts the mesh back") };
        self.status = Status::Info(format!("Reassembled {name} into {} -- {back}", tally(assembly)));
    }

    /// Put one body into the tree: the shape it was recognised as, or its
    /// triangles where it was recognised as nothing.
    fn place_part(&mut self, part: &Part, base: &str, parent: NodeId, index: usize) {
        let id = match recipe(part.shape) {
            Some((type_id, params)) => {
                let Some(id) = self.scene.add_primitive(type_id, parent, index) else { return };
                if let Some(node) = self.scene.get_mut(id) {
                    node.body = Body::Primitive { type_id: type_id.to_string(), params };
                    // The count the body was tessellated with, not the
                    // document's default: rebuilt at thirty-two segments a
                    // twelve-segment cylinder is a wider solid than the
                    // triangles described, and a part reassembled to be
                    // measured would measure wrong.
                    node.segments = part.shape.segments();
                }
                id
            }
            None => {
                let mesh = simple3d_core::mesh_data::MeshData::new(part.mesh.clone());
                self.scene.add_mesh(&format!("{base} Part"), mesh, parent, index)
            }
        };
        if let Some(node) = self.scene.get_mut(id) {
            node.position = part.centre;
            node.rotation = part.rotation;
        }
    }

    /// Put the tool away. Nothing to undo: the document was never written to.
    pub fn cancel_reassemble_tool(&mut self) {
        let Some(tool) = self.reassemble_tool.take() else { return };
        if let Some(job) = &tool.job {
            job.cancel();
        }
    }
}

/// The registry shape a recognised body is rebuilt as, and the parameters to
/// build it from -- or nothing for a body that is staying a mesh.
///
/// Started from the shape's own defaults and written over, so a parameter the
/// recognition has nothing to say about -- a sweep, a measuring convention --
/// comes out as the shape would come out of the Add menu rather than as zero.
fn recipe(shape: Shape) -> Option<(&'static str, Params)> {
    let length = ParamValue::Length;
    let (type_id, values): (&str, Vec<(&str, ParamValue)>) = match shape {
        Shape::Mesh => return None,
        Shape::Box { width, depth, height } => {
            ("box", vec![("width", length(width)), ("depth", length(depth)), ("height", length(height))])
        }
        Shape::Sphere { diameter_x, diameter_y, diameter_z, .. } => (
            "sphere",
            vec![
                ("diameter_x", length(diameter_x)),
                ("diameter_y", length(diameter_y)),
                ("diameter_z", length(diameter_z)),
            ],
        ),
        Shape::Cylinder { diameter, height, .. } => (
            "cylinder",
            vec![("diameter_x", length(diameter)), ("diameter_y", length(diameter)), ("height", length(height))],
        ),
        Shape::Cone { bottom_diameter, top_diameter, height, .. } => (
            "cone",
            vec![
                ("bottom_diameter", length(bottom_diameter)),
                ("top_diameter", length(top_diameter)),
                ("height", length(height)),
            ],
        ),
        Shape::Prism { sides, diameter, height } => (
            "prism",
            vec![
                ("sides", ParamValue::Count(sides)),
                ("diameter", length(diameter)),
                ("height", length(height)),
                // Across corners, which is the diameter the fit measured: the
                // circle the ring of vertices sits on.
                ("measure", ParamValue::Choice(0)),
            ],
        ),
    };
    let spec = primitive::lookup(type_id)?;
    let mut params = spec.default_params();
    for (key, value) in values {
        params.insert(key.to_string(), value);
    }
    Some((type_id, params))
}
