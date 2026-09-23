//! What an export would contain, and running one.

use super::*;
use crate::worker::ExportJob;
use simple3d_core::scene::{NodeId, Scene};
use simple3d_core::xform::Xform;
use simple3d_geom::Mesh;
use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;

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

/// The parts an export writes, worked out ahead of time, and the count the
/// dialog shows of them. Kept whole so pressing Export reuses them instead of
/// working them out a second time.
#[derive(Clone)]
pub(crate) struct ExportPrepared {
    pub summary: ExportSummary,
    pub parts: Arc<Vec<(String, Arc<Mesh>)>>,
}

/// The export dialog's count, worked out on a thread of its own (issue 111).
///
/// Counting means evaluating every body, booleans and all, and unioning the
/// ones marked to share a body. Done on the interface thread, as it was, that
/// froze the window each time a body was picked or the selection changed with
/// the dialog open. One count runs at a time: a change while one is running
/// waits for it and then asks again, so picking through a list of bodies
/// does not start one thread per click.
#[derive(Default)]
pub(crate) struct ExportPreview {
    /// The newest finished count, and what it counted; `None` inside when the
    /// thread working it out stopped without an answer.
    pub done: Option<(ExportPreviewKey, Option<ExportPrepared>)>,
    pub running: Option<(ExportPreviewKey, Receiver<ExportPrepared>)>,
}

/// Everything working out an export's parts needs, copied off the document so
/// the work can happen away from the interface thread.
pub(crate) struct ExportInput {
    scene: Scene,
    roots: Vec<NodeId>,
    frames: BTreeMap<NodeId, Xform>,
    bodies: simple3d_export::BodyMode,
    /// The whole evaluated scene, when that is what a single body is: the
    /// evaluation already made it, so there is nothing to work out.
    whole: Option<Arc<Mesh>>,
}

impl ExportInput {
    /// The single mesh a merged export writes. A selection is re-evaluated
    /// subtree by subtree rather than merged out of `Evaluated::node_meshes`,
    /// which holds primitives only -- merging those wrote a selected boolean
    /// group as its raw operands, so a difference kept its cutter and a union
    /// was refused as non-manifold.
    pub fn mesh(&self) -> Arc<Mesh> {
        match &self.whole {
            Some(mesh) => mesh.clone(),
            None => Arc::new(simple3d_core::eval::selection_mesh(&self.scene, &self.roots, &self.frames)),
        }
    }

    /// The objects an export keeps apart. A node is one object however deep
    /// it goes -- a boolean group is the shape it evaluates to, the same body
    /// the viewport draws, not its operands.
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

    /// Exactly what the export job writes, and the count of it.
    ///
    /// Separated bodies are counted unmerged, which is what will be written:
    /// merging two touching bodies drops the triangles buried inside the join,
    /// and keeping them apart does not.
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

    /// The mesh an export writes: the whole scene, or just what is selected.
    /// Worked out on the spot, which only a test can afford.
    #[cfg(test)]
    pub fn export_mesh(&self) -> Arc<Mesh> {
        self.export_input().mesh()
    }

    /// The objects an export keeps apart: the scene's own top-level nodes, or
    /// the top-level nodes of the selection. Worked out on the spot, as above.
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
    /// watertight, and how many bodies they will be written as. `None` while
    /// that is still being worked out, see [`ExportPreview`].
    ///
    /// The key carries the export body marks as well as the evaluation, since
    /// regrouping changes the answer without changing any geometry.
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
        // The dialog's count already worked the parts out; when it has not
        // finished, the export's own thread does it, never this one (issue 111).
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
