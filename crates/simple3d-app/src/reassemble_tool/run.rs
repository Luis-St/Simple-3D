//! Putting what was found into the document, and putting the tool away.

use super::*;
use crate::app::{App, Status};
use simple3d_core::primitive::{self, ParamValue, Params};
use simple3d_core::scene::{Body, GroupOp};
use simple3d_geom::reassemble::{Part, Shape};

impl App {
    /// Whether the on-screen answer is worth applying: current, about a mesh that still exists, and
    /// not just the mesh again.
    pub(crate) fn reassemble_ready(&self) -> bool {
        let Some(tool) = self.reassemble_tool.as_ref() else { return false };
        let Some(found) = tool.found.as_ref() else { return false };
        found.plan == tool.plan
            && self.scene.contains(tool.target)
            && !found.assembly.is_nothing()
            && found.assembly.objects() > 0
    }

    /// Place the found objects where the mesh stood (issue 108). The mesh's node becomes their group,
    /// keeping its name, place, transform and colour, or the one shape itself when that is all there is
    /// (`App::become_single`).
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
        if !self.become_single(tool.target, assembly) {
            if !self.scene.make_group(tool.target, GroupOp::Union) {
                self.history.discard_last();
                self.status = Status::Warning(format!("{name} could not be taken apart"));
                return;
            }
            self.place_assembly(tool.target, assembly, &name);
        }
        self.collapsed.remove(&tool.target);
        self.select_only(tool.target);
        self.touch();
        self.settings.last_reassemble = tool.plan;
        self.persist();
        // Not `way_back` (Join for splits): a reassembly's way back is the undo step.
        let undo = self.keymap.shortcut_text(simple3d_core::keymap::Command::Undo);
        let back =
            if undo.is_empty() { "undo puts the mesh back".to_string() } else { format!("{undo} puts the mesh back") };
        self.status = Status::Info(format!("Reassembled {name} into {} -- {back}", tally(assembly)));
    }

    /// Fill the group the mesh's node has become.
    fn place_assembly(&mut self, target: NodeId, assembly: &simple3d_geom::reassemble::Assembly, name: &str) {
        // One group and no leftover is the node itself; a second wrapping group would say nothing.
        let flat = assembly.groups.len() == 1 && assembly.rest.is_none();
        let mut at = 0;
        for group in &assembly.groups {
            let holder = if flat || group.len() == 1 {
                target
            } else {
                let holder = self.scene.add_group(GroupOp::Union, target, at);
                at += 1;
                holder
            };
            let mut inside = if holder == target { at } else { 0 };
            for &index in group {
                self.place_part(&assembly.parts[index], name, holder, inside);
                inside += 1;
            }
            if holder == target {
                at = inside;
            }
            self.collapsed.remove(&holder);
        }
        if let Some(rest) = &assembly.rest {
            let mesh = simple3d_core::mesh_data::MeshData::new(rest.clone());
            self.scene.add_mesh(&format!("{name} Remainder"), mesh, target, at);
        }
    }

    /// Put one body into the tree: its recognised shape, or its triangles.
    fn place_part(&mut self, part: &Part, base: &str, parent: NodeId, index: usize) {
        let id = match recipe(part.shape) {
            Some((type_id, params)) => {
                let Some(id) = self.scene.add_primitive(type_id, parent, index) else { return };
                if let Some(node) = self.scene.get_mut(id) {
                    node.body = Body::Primitive { type_id: type_id.to_string(), params };
                    // The body's own segment count, not the default, or the rebuilt solid would differ.
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

    /// Put the tool away; nothing to undo, since the document was never written.
    pub fn cancel_reassemble_tool(&mut self) {
        let Some(tool) = self.reassemble_tool.take() else { return };
        if let Some(job) = &tool.job {
            job.cancel();
        }
    }
}

/// The registry shape and parameters to rebuild a recognised body, or nothing for a mesh. Starts
/// from the shape's defaults so unrecognised parameters match the Add menu rather than zero.
pub(super) fn recipe(shape: Shape) -> Option<(&'static str, Params)> {
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
                // Across corners, the diameter the fit measured.
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
