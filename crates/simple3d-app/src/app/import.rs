//! Bringing a model file into the document on screen (issue 105).
//!
//! Imports become stored meshes (like conversions, issue 80). One object becomes one node;
//! several come in as a group named after the file, preserving the file's structure.

use super::*;
use crate::worker::ImportJob;
use simple3d_core::scene::GroupOp;

impl App {
    /// Ask for a file, and start reading it once the dialog answers.
    pub fn start_import(&mut self) {
        let mut dialog = rfd::FileDialog::new();
        // A combined filter first, then one per format.
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
        // The tab is fixed now, since the answer arrives later and belongs to the asking document.
        let tab = self.active;
        self.ask_for_file("Import", dialog, false, move |app, path| {
            app.settings.last_import_dir = path.parent().map(|p| p.to_path_buf());
            app.status = Status::Info(format!("Reading {}\u{2026}", path.display()));
            app.import_job = Some(ImportJob::spawn(path, tab, IMPORT_LIMIT));
        });
    }

    /// Take the read model and place it in the document.
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

    /// Put a read model into the scene, select it, and report it. Separate so tests can call it
    /// without a thread or file.
    pub(crate) fn place_import(&mut self, stem: &str, model: simple3d_import::Model) {
        let root = self.scene.root();
        let index = self.scene.node(root).children.len();
        let triangles = model.triangle_count();
        // A single body is named after the file, since its internal name is usually the writer's banner;
        // several keep their own names to tell them apart.
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
            // An assembly, not a union: the bodies stay separate, and unioning face-sharing parts is the
            // kernel's slowest case (see `GroupOp::Assembly`).
            let group = self.scene.add_group(GroupOp::Assembly, root, index);
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
        // Frame the view on it, since imported models are often far from the origin.
        self.frame_when_evaluated = true;
        self.status = Status::Info(import_summary(&model, triangles));
    }
}

/// The footer's import report: geometry, format, unit, and whether it is closed. An open mesh is
/// reported rather than refused, since export will refuse it until repaired.
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
