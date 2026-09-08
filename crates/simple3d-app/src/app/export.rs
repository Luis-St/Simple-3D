//! What an export would contain, and running one.

use super::*;
use crate::worker::ExportJob;
use simple3d_core::scene::NodeId;

/// What the export dialog was last asked to count, so the answer can be reused
/// until something it depends on changes.
#[derive(Clone, PartialEq)]
pub(crate) struct ExportPreviewKey {
    pub selection_only: bool,
    pub selection: Vec<NodeId>,
    pub generation: u64,
    pub bodies: simple3d_export::BodyMode,
    pub marks: Vec<(NodeId, simple3d_core::scene::ExportBody)>,
}

/// What an export is about to write.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ExportSummary {
    pub triangles: usize,
    pub bodies: usize,
}

impl App {
    // -- export -------------------------------------------------------------

    /// The mesh an export writes: the whole scene, or just what is selected.
    ///
    /// A selection is re-evaluated subtree by subtree rather than merged out of
    /// `Evaluated::node_meshes`, which holds primitives only -- merging those
    /// wrote a selected boolean group as its raw operands, so a difference kept
    /// its cutter and a union was refused as non-manifold.
    pub fn export_mesh(&self) -> std::sync::Arc<simple3d_geom::Mesh> {
        if self.export_selection_only {
            let tops = self.top_level_selection();
            std::sync::Arc::new(simple3d_core::eval::selection_mesh(&self.scene, &tops, &self.evaluated.node_frames))
        } else {
            self.evaluated.mesh.clone()
        }
    }

    /// The objects an export keeps apart: the scene's own top-level nodes, or
    /// the top-level nodes of the selection.
    ///
    /// A node is one object however deep it goes -- a boolean group is the
    /// shape it evaluates to, the same body the viewport draws, not its
    /// operands. Evaluated fresh rather than merged out of `Evaluated`, for the
    /// reason `export_mesh` gives.
    pub fn export_parts(&self) -> Vec<simple3d_core::eval::Part> {
        let roots = self.export_roots();
        let frames = &self.evaluated.node_frames;
        match self.export_body_mode() {
            simple3d_export::BodyMode::One => Vec::new(),
            simple3d_export::BodyMode::TopLevel => simple3d_core::eval::part_meshes(&self.scene, &roots, frames),
            simple3d_export::BodyMode::Selected => simple3d_core::eval::body_meshes(&self.scene, &roots, frames),
        }
    }

    /// What this export's bodies really are: the mode chosen, unless the format
    /// has nowhere to put more than one, in which case there is only ever the
    /// single merged body.
    pub fn export_body_mode(&self) -> simple3d_export::BodyMode {
        if self.export_format.keeps_objects_separate() {
            self.export_bodies
        } else {
            simple3d_export::BodyMode::One
        }
    }

    /// The nodes an export starts from: the scene's own top level, or the
    /// top-level nodes of the selection. Also what the body picker's tree
    /// shows, and where `Scene::body_lock` stops walking upwards.
    pub fn export_roots(&self) -> Vec<NodeId> {
        if self.export_selection_only {
            self.top_level_selection()
        } else {
            self.scene.node(self.scene.root()).children.clone()
        }
    }

    /// What the export dialog promises: how many triangles will be verified as
    /// watertight, and how many bodies they will be written as.
    ///
    /// Separated bodies are counted unmerged, which is what will be written:
    /// merging two touching bodies drops the triangles buried inside the join,
    /// and keeping them apart does not.
    ///
    /// Cached, because working it out in the user-chosen mode means unioning
    /// every body -- far too much to do on each frame the window is open. The
    /// key carries the export body marks as well as the evaluation, since
    /// regrouping changes the answer without changing any geometry.
    pub fn export_summary(&mut self) -> ExportSummary {
        let key = ExportPreviewKey {
            selection_only: self.export_selection_only,
            selection: self.selection.clone(),
            generation: self.evaluation_generation,
            bodies: self.export_body_mode(),
            marks: self.scene.export_body_marks(),
        };
        if let Some((cached, summary)) = &self.export_preview {
            if *cached == key {
                return *summary;
            }
        }
        let summary = if key.bodies.separates() {
            let parts = self.export_parts();
            ExportSummary { triangles: parts.iter().map(|part| part.mesh.triangle_count()).sum(), bodies: parts.len() }
        } else {
            ExportSummary { triangles: self.export_mesh().triangle_count(), bodies: 1 }
        };
        self.export_preview = Some((key, summary));
        summary
    }

    pub fn start_export(&mut self) {
        let scale = simple3d_core::unit::parse_number(&self.export_scale).unwrap_or(1.0);
        if scale <= 0.0 {
            self.fail("The export scale must be greater than zero", "Enter a positive scale factor.");
            return;
        }
        // A boolean the kernel could not evaluate means the mesh is not
        // trustworthy; refuse with the specific reason and name the node.
        if !self.evaluated.errors.is_empty() {
            let detail = self
                .evaluated
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.name, e.message))
                .collect::<Vec<_>>()
                .join("\n");
            self.fail("Export refused: the scene has unevaluated geometry", &detail);
            return;
        }
        let bodies = self.export_body_mode();
        let parts: Vec<(String, std::sync::Arc<simple3d_geom::Mesh>)> = if bodies.separates() {
            self.export_parts().into_iter().map(|part| (part.name, std::sync::Arc::new(part.mesh))).collect()
        } else {
            vec![(String::new(), self.export_mesh())]
        };
        if parts.iter().all(|(_, mesh)| mesh.triangle_count() == 0) {
            self.fail("There is nothing to export", "The scene, or the selection, has no visible geometry.");
            return;
        }

        let default_name = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "model".to_string());
        let mut dialog = rfd::FileDialog::new()
            .add_filter(self.export_format.label(), &[self.export_format.extension()])
            .set_file_name(format!("{default_name}.{}", self.export_format.extension()));
        if let Some(dir) = self
            .settings
            .last_export_dir
            .clone()
            .or_else(|| self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())))
        {
            dialog = dialog.set_directory(dir);
        }
        // Everything the export needs is settled here, before the dialog goes
        // up, and travels with it: the answer arrives on a later frame, and the
        // job must be the one the user asked for and not whatever the document
        // looks like by the time they have finished choosing a folder.
        let options = simple3d_export::Options {
            format: self.export_format,
            scale,
            unit: simple3d_export::Unit3mf::Millimeter,
            allow_invalid: false,
            bodies,
        };
        let extension = self.export_format.extension();
        let format_id = self.export_format.id().to_string();
        let bodies_id = self.export_bodies.id().to_string();
        self.ask_for_file("Export", dialog, true, move |app, mut path| {
            if path.extension().is_none() {
                path.set_extension(extension);
            }
            app.settings.last_export_dir = path.parent().map(|p| p.to_path_buf());
            app.settings.last_export_format = format_id;
            app.settings.last_export_scale = scale;
            app.settings.last_export_bodies = bodies_id;
            app.export_job = Some(ExportJob::spawn_parts(path, parts, options, EXPORT_LIMIT));
            app.modal = Modal::None;
        });
    }

    pub(super) fn poll_export(&mut self) {
        let Some(job) = &self.export_job else { return };
        let Some(outcome) = job.poll() else { return };
        let path = job.path.clone();
        let label = job.format_label.clone();
        self.export_job = None;
        match outcome {
            Ok(()) => self.status = Status::Info(format!("Exported {label} to {}", path.display())),
            Err(simple3d_export::ExportError::Cancelled) => {
                self.status = Status::Info("Export cancelled; no file was written".into())
            }
            Err(e) => self.fail("Export failed", &e.to_string()),
        }
    }
}
