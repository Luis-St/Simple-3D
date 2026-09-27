//! What an export would contain, and running one.

use super::*;
use crate::worker::ExportJob;
use simple3d_core::scene::{NodeId, Scene};
use simple3d_core::xform::Xform;
use simple3d_geom::Mesh;
use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;

/// What the export dialog last counted, reused until an input changes.
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

/// The parts an export writes, prepared ahead, with the dialog's count; reused by Export.
#[derive(Clone)]
pub(crate) struct ExportPrepared {
    pub summary: ExportSummary,
    pub parts: Arc<Vec<(String, Arc<Mesh>)>>,
}

/// The export dialog's count, computed on its own thread (issue 111), since evaluating every
/// body froze the window. One count at a time; a change during one waits and then asks again.
#[derive(Default)]
pub(crate) struct ExportPreview {
    /// The newest finished count and its key; `None` inside if the thread gave no answer.
    pub done: Option<(ExportPreviewKey, Option<ExportPrepared>)>,
    pub running: Option<(ExportPreviewKey, Receiver<ExportPrepared>)>,
}

/// What preparing an export needs, copied off the document for a background thread.
pub(crate) struct ExportInput {
    scene: Scene,
    roots: Vec<NodeId>,
    frames: BTreeMap<NodeId, Xform>,
    bodies: simple3d_export::BodyMode,
    /// The whole evaluated scene, when that is the single body, so nothing needs computing.
    whole: Option<Arc<Mesh>>,
}

impl ExportInput {
    /// The single mesh a merged export writes. A selection is re-evaluated per subtree, since
    /// merging `Evaluated::node_meshes` (primitives only) wrote booleans as their raw operands.
    pub fn mesh(&self) -> Arc<Mesh> {
        match &self.whole {
            Some(mesh) => mesh.clone(),
            None => Arc::new(simple3d_core::eval::selection_mesh(&self.scene, &self.roots, &self.frames)),
        }
    }

    /// The objects an export keeps apart; a boolean group is one object, as the viewport draws it.
    pub fn parts(&self) -> Vec<simple3d_core::eval::Part> {
        match self.bodies {
            simple3d_export::BodyMode::One => Vec::new(),
            simple3d_export::BodyMode::TopLevel => {
                simple3d_core::eval::part_meshes(&self.scene, &self.roots, &self.frames)
            }
            simple3d_export::BodyMode::Selected => {
                simple3d_core::eval::body_meshes(&self.scene, &self.roots, &self.frames)
            }
        }
    }

    /// Exactly what the export job writes, and its count. Separate bodies are counted unmerged,
    /// as written, since merging drops triangles buried in joins.
    pub fn prepare(&self) -> ExportPrepared {
        let parts: Vec<(String, Arc<Mesh>)> = if self.bodies.separates() {
            self.parts().into_iter().map(|part| (part.name, Arc::new(part.mesh))).collect()
        } else {
            vec![(String::new(), self.mesh())]
        };
        let triangles = parts.iter().map(|(_, mesh)| mesh.triangle_count()).sum();
        let bodies = if self.bodies.separates() { parts.len() } else { 1 };
        ExportPrepared { summary: ExportSummary { triangles, bodies }, parts: Arc::new(parts) }
    }
}

impl App {
    // -- export -------------------------------------------------------------

    /// The mesh an export writes, computed synchronously; tests only.
    #[cfg(test)]
    pub fn export_mesh(&self) -> Arc<Mesh> {
        self.export_input().mesh()
    }

    /// The objects an export keeps apart, computed synchronously; tests only.
    #[cfg(test)]
    pub fn export_parts(&self) -> Vec<simple3d_core::eval::Part> {
        self.export_input().parts()
    }

    pub(crate) fn export_input(&self) -> ExportInput {
        ExportInput {
            scene: self.scene.clone(),
            roots: self.export_roots(),
            frames: self.evaluated.node_frames.clone(),
            bodies: self.export_body_mode(),
            whole: (!self.export_selection_only).then(|| self.evaluated.mesh.clone()),
        }
    }

    /// The effective body mode: the chosen one, or a single merged body if the format holds only one.
    pub fn export_body_mode(&self) -> simple3d_export::BodyMode {
        if self.export_format.keeps_objects_separate() {
            self.export_bodies
        } else {
            simple3d_export::BodyMode::One
        }
    }

    /// The nodes an export starts from: the scene's or the selection's top level. Also the body
    /// picker's roots and where `Scene::body_lock` stops.
    pub fn export_roots(&self) -> Vec<NodeId> {
        if self.export_selection_only {
            self.top_level_selection()
        } else {
            self.scene.node(self.scene.root()).children.clone()
        }
    }

    /// The dialog's promise: triangles to verify and bodies to write; `None` while still computing
    /// (see [`ExportPreview`]). The key includes body marks, since regrouping changes the answer.
    pub fn export_summary(&mut self) -> Option<ExportSummary> {
        self.export_prepared().map(|prepared| prepared.summary)
    }

    pub(crate) fn export_prepared(&mut self) -> Option<ExportPrepared> {
        let key = ExportPreviewKey {
            selection_only: self.export_selection_only,
            selection: self.selection.clone(),
            generation: self.evaluation_generation,
            bodies: self.export_body_mode(),
            marks: self.scene.export_body_marks(),
        };
        let preview = &mut self.export_preview;
        if let Some((counted, receiver)) = &preview.running {
            match receiver.try_recv() {
                Ok(prepared) => preview.done = Some((counted.clone(), Some(prepared))),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => preview.done = Some((counted.clone(), None)),
            }
            if preview.done.as_ref().is_some_and(|(done, _)| done == counted) {
                preview.running = None;
            }
        }
        if let Some((counted, prepared)) = &preview.done {
            if *counted == key {
                return prepared.clone();
            }
        }
        if preview.running.is_none() {
            let input = self.export_input();
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::Builder::new()
                .name("simple3d-export-count".into())
                .spawn(move || {
                    let _ = sender.send(input.prepare());
                })
                .expect("the platform can start a thread");
            self.export_preview.running = Some((key, receiver));
        }
        None
    }

    pub fn start_export(&mut self) {
        let scale = simple3d_core::unit::parse_number(&self.export_scale).unwrap_or(1.0);
        if scale <= 0.0 {
            self.fail("The export scale must be greater than zero", "Enter a positive scale factor.");
            return;
        }
        // An unevaluated boolean makes the mesh untrustworthy; refuse and name the node.
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
        // Reuse the dialog's prepared parts; if unfinished, the export thread computes them (issue 111).
        let prepared = self.export_prepared();
        if prepared.as_ref().is_some_and(|prepared| prepared.summary.triangles == 0) {
            self.fail("There is nothing to export", "The scene, or the selection, has no visible geometry.");
            return;
        }
        let input = self.export_input();

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
        // Settled before the dialog opens, so the job is what was asked for even if the document
        // changes while a folder is being chosen.
        let options = simple3d_export::Options {
            format: self.export_format,
            scale,
            unit: simple3d_export::Unit3mf::Millimeter,
            allow_invalid: false,
            bodies,
            compress: self.export_compress,
        };
        let extension = self.export_format.extension();
        let format_id = self.export_format.id().to_string();
        let bodies_id = self.export_bodies.id().to_string();
        let compress = self.export_compress;
        self.ask_for_file("Export", dialog, true, move |app, mut path| {
            if path.extension().is_none() {
                path.set_extension(extension);
            }
            app.settings.last_export_dir = path.parent().map(|p| p.to_path_buf());
            app.settings.last_export_format = format_id;
            app.settings.last_export_scale = scale;
            app.settings.last_export_bodies = bodies_id;
            app.settings.last_export_compress = compress;
            let build = move || match prepared {
                Some(prepared) => Arc::unwrap_or_clone(prepared.parts),
                None => Arc::unwrap_or_clone(input.prepare().parts),
            };
            app.export_job = Some(ExportJob::spawn_building(path, build, options, EXPORT_LIMIT));
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
