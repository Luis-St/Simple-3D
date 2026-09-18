//! Bringing a model file into the document on screen (issue 105).
//!
//! The other half of the export: every format an export writes can be read
//! back, and what arrives is a stored mesh -- a file from another program has
//! no parameters to recover, only a surface. So an import stands where a
//! conversion to a mesh does (issue 80): a node the user can move, rotate,
//! paint, group, cut into pieces and export again, with no recipe behind it.
//!
//! What the file was structured as is kept. One object comes in as one node;
//! several -- a 3MF's objects, an OBJ's groups, an STL's several solids -- come
//! in as a group of nodes named after the file, so the rows that were exported
//! are the rows that come back rather than one bag of triangles.

use super::*;
use crate::worker::ImportJob;
use simple3d_core::scene::GroupOp;

impl App {
    /// Ask for a file, and start reading it once the dialog answers.
    pub fn start_import(&mut self) {
        let mut dialog = rfd::FileDialog::new();
        // Every format under one filter first, because a user importing a model
        // knows what they have and not which of four filters it is under; the
        // individual ones follow for narrowing a crowded folder.
        let every: Vec<&str> = simple3d_import::Format::ALL.iter().flat_map(|f| f.extensions()).copied().collect();
        dialog = dialog.add_filter("Model files", &every);
        for format in simple3d_import::Format::ALL {
            dialog = dialog.add_filter(format.label(), format.extensions());
        }
        if let Some(dir) = self
            .settings
            .last_import_dir
            .clone()
            .or_else(|| self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())))
        {
            dialog = dialog.set_directory(dir);
        }
        // The tab is settled here, before the dialog goes up: the answer arrives
        // on a later frame, and the file belongs to the document that asked for
        // it rather than to whichever one is on screen by then.
        let tab = self.active;
        self.ask_for_file("Import", dialog, false, move |app, path| {
            app.settings.last_import_dir = path.parent().map(|p| p.to_path_buf());
            app.status = Status::Info(format!("Reading {}\u{2026}", path.display()));
            app.import_job = Some(ImportJob::spawn(path, tab, IMPORT_LIMIT));
        });
    }

    /// Take the model once it has been read, and stand it in the document.
    pub(crate) fn poll_import(&mut self) {
        let Some(job) = &self.import_job else { return };
        let Some(outcome) = job.poll() else { return };
        let job = self.import_job.take().expect("it was there a line ago");
        let model = match outcome {
            Ok(model) => model,
            Err(simple3d_import::ImportError::Cancelled) => {
                self.status = Status::Info("Import cancelled; nothing was brought in".into());
                return;
            }
            Err(e) => {
                self.fail("Could not import the model", &format!("{}\n\n{e}", job.path.display()));
                return;
            }
        };
        if job.tab != self.active {
            self.status =
                Status::Warning("The import was dropped: it was read for a document that is no longer open".into());
            return;
        }
        self.place_import(&job.stem(), model);
    }

    /// Put a read model into the scene, select what it became, and say what
    /// arrived. Separate from the polling so a test can hand it a model without
    /// a thread and a file in between.
    pub(crate) fn place_import(&mut self, stem: &str, model: simple3d_import::Model) {
        let root = self.scene.root();
        let index = self.scene.node(root).children.len();
        let triangles = model.triangle_count();
        // A file of one body is named after the file, whatever the body inside
        // it is called: the name in a single-object file is nearly always the
        // writing program's own banner -- this application's STL says `solid
        // simple3d` -- and the file name is the one the user chose and will
        // recognise in the outliner. A file of several keeps each body's own
        // name, because there those names are what tells them apart.
        let named = |part: &simple3d_import::Part| -> String {
            match part.name.trim() {
                "" => format!("{stem} part"),
                name => name.to_string(),
            }
        };
        self.edit("Import a model", None);
        let landed = if model.parts.len() == 1 {
            let part = &model.parts[0];
            let mesh = simple3d_core::mesh_data::MeshData::new(part.mesh.clone());
            self.scene.add_mesh(stem, mesh, root, index)
        } else {
            // A union, which is what a group of separate bodies is: they are
            // one model, and the evaluation leaves disjoint parts alone rather
            // than running them through the boolean kernel.
            let group = self.scene.add_group(GroupOp::Union, root, index);
            if let Some(node) = self.scene.get_mut(group) {
                node.name = stem.to_string();
            }
            for (at, part) in model.parts.iter().enumerate() {
                let mesh = simple3d_core::mesh_data::MeshData::new(part.mesh.clone());
                self.scene.add_mesh(&named(part), mesh, group, at);
            }
            group
        };
        self.select_only(landed);
        // The model arrives wherever the file says it stands, which for a file
        // from another program is often nowhere near the origin -- so the view
        // is taken to it rather than leaving the user to hunt for what they
        // just imported.
        self.frame_when_evaluated = true;
        self.status = Status::Info(import_summary(&model, triangles));
    }
}

/// What the footer says about an import that has just landed: how much geometry
/// arrived, as what, from which unit, and whether it is a closed solid.
///
/// The last of those is said rather than refused. An export verifies before it
/// writes, because a file that fails in a slicer is the export's fault; an
/// import has no such choice -- the mesh is what somebody else wrote, and a
/// model with a hole in it is still the model the user asked for. What it does
/// mean is that exporting it again will be refused until it is repaired, which
/// is worth knowing now rather than then.
pub(crate) fn import_summary(model: &simple3d_import::Model, triangles: usize) -> String {
    let mut message = format!("Imported {triangles} triangle{}", plural(triangles));
    if model.parts.len() > 1 {
        message.push_str(&format!(" as {} bodies", model.parts.len()));
    }
    message.push_str(&format!(" from {}", model.format.label()));
    match model.unit {
        Some(unit) if unit != simple3d_import::Unit::Millimetre => {
            message.push_str(&format!(", converted from {}", unit.label()))
        }
        _ => {}
    }
    if model.merged().manifold_issue().is_some() {
        message.push_str(" -- it is not a closed solid, so it will need repairing before it can be exported");
    }
    message
}
