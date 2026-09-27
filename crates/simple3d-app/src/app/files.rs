//! Opening and saving, with a file dialog that runs while the application keeps drawing.

use super::*;
use simple3d_core::project;
use std::path::Path;

impl App {
    /// Put up a file dialog and say what to do with the answer. `what` names the wait in the footer
    /// and the cancellation message, so an unanswered portal is not silent. One at a time.
    pub(crate) fn ask_for_file(
        &mut self,
        what: &'static str,
        dialog: rfd::FileDialog,
        saving: bool,
        then: impl FnOnce(&mut App, std::path::PathBuf) + 'static,
    ) {
        if let Some(waiting) = &self.file_prompt {
            self.status = Status::Warning(format!("Still choosing a file for {}", waiting.what().to_lowercase()));
            return;
        }
        self.file_prompt = Some(FilePrompt {
            what,
            answer: ask_for_path(dialog, saving),
            then: Box::new(then),
            started: std::time::Instant::now(),
        });
    }

    /// Act on an answered file dialog; polled once a frame.
    pub(crate) fn poll_file_prompt(&mut self) {
        let Some(prompt) = &self.file_prompt else { return };
        let answer = match prompt.answer.try_recv() {
            Ok(answer) => answer,
            // The dialog thread went away without answering: treated as a cancel.
            Err(std::sync::mpsc::TryRecvError::Disconnected) => None,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
        };
        let prompt = self.file_prompt.take().expect("checked just above");
        match answer {
            Some(path) => (prompt.then)(self, path),
            None => self.status = Status::Info(format!("{} cancelled", prompt.what)),
        }
    }

    /// Stop waiting on the dialog; its thread stays parked and its answer is dropped.
    pub(crate) fn stop_waiting_for_file(&mut self) {
        if let Some(prompt) = self.file_prompt.take() {
            self.status = Status::Warning(format!("Stopped waiting for the file dialog ({})", prompt.what));
        }
    }

    pub fn open_dialog(&mut self) {
        let mut dialog = rfd::FileDialog::new().add_filter("Simple 3D project", &[PROJECT_EXTENSION]);
        if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        self.ask_for_file("Open", dialog, false, |app, path| app.open_path(&path));
    }

    /// Read `path` into the on-screen document, replacing it (`crate::tabs::open_path` chose the tab).
    pub(crate) fn load_into_active(&mut self, path: &Path) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                self.settings.forget_recent(path);
                return self.fail("Could not open the project", &format!("{}\n\n{e}", path.display()));
            }
        };
        match project::project_from_str(&text) {
            Ok(loaded) => {
                self.take_loaded_project(loaded);
                // The file's own camera stands.
                self.frame_when_evaluated = false;
                self.selection.clear();
                self.history.clear();
                self.saved_revision = self.history.revision();
                self.path = Some(path.to_path_buf());
                self.settings.remember_recent(path);
                self.fields.clear();
                self.dirty = true;
                self.status = Status::Info(format!("Opened {}", path.display()));
            }
            Err(e) => {
                self.settings.forget_recent(path);
                self.fail("Could not read the project", &format!("{}\n\n{e}", path.display()));
            }
        }
    }

    pub fn save(&mut self) {
        match self.path.clone() {
            Some(path) => self.save_to(&path),
            None => self.save_as(),
        }
    }

    pub fn save_as(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Simple 3D project", &[PROJECT_EXTENSION])
            .set_file_name(format!("model.{PROJECT_EXTENSION}"));
        if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        self.ask_for_file("Save as", dialog, true, |app, mut path| {
            if path.extension().is_none() {
                path.set_extension(PROJECT_EXTENSION);
            }
            app.save_to(&path);
        });
    }

    pub(crate) fn save_to(&mut self, path: &Path) {
        // Saved without a tool's preview, which is not yet a result (issue 106).
        let lifted = self.lift_preview();
        let text = self.project_text();
        self.drop_preview_back(lifted);
        match std::fs::write(path, text) {
            Ok(()) => {
                self.path = Some(path.to_path_buf());
                self.saved_revision = self.history.revision();
                self.project.saved_structure = self.project.structure_revision;
                for component in &mut self.project.components {
                    component.saved_revision = component.history.revision();
                }
                self.settings.remember_recent(path);
                self.status = Status::Info(format!("Saved {}", path.display()));
            }
            Err(e) => self.fail("Could not save the project", &format!("{}\n\n{e}", path.display())),
        }
    }

    /// The whole project as its file holds it, every component included (issue 113).
    fn project_text(&self) -> String {
        let root = self.component_scene(simple3d_core::scene::ROOT_COMPONENT).expect("a project always has its root");
        let others: Vec<(simple3d_core::scene::ComponentId, &simple3d_core::scene::Scene)> = self
            .project
            .components
            .iter()
            .filter(|c| c.id != simple3d_core::scene::ROOT_COMPONENT)
            .filter_map(|c| Some((c.id, self.component_scene(c.id)?)))
            .collect();
        project::project_to_string(root, &others)
    }

    /// Make a loaded project the one on screen, on its root component; only called on scratch tabs.
    fn take_loaded_project(&mut self, loaded: project::ProjectData) {
        let mut project = crate::components::Project::new();
        for (id, scene) in loaded.components {
            project.next_id = project.next_id.max(id + 1);
            project.components.push(crate::components::Component::new(id, scene));
        }
        self.project = project;
        self.scene = loaded.root;
        self.relink_components();
    }

    pub fn fail(&mut self, title: &str, detail: &str) {
        self.error_title = title.to_string();
        self.error_detail = detail.to_string();
        self.modal = Modal::Error;
        self.status = Status::Warning(title.to_string());
    }
}

/// A file dialog in flight, and what to do with its answer.
///
/// On Linux `rfd` uses the raw D-Bus portal and blocks the calling thread, so a slow or missing
/// portal froze the application. It now waits on its own thread, and the footer offers a way to
/// stop waiting.
pub(crate) struct FilePrompt {
    pub(super) what: &'static str,
    pub(super) answer: std::sync::mpsc::Receiver<Option<std::path::PathBuf>>,
    pub(super) then: FollowUp,
    pub(super) started: std::time::Instant,
}

/// What to do with the answered path, run on the UI thread with the whole application.
pub(crate) type FollowUp = Box<dyn FnOnce(&mut App, std::path::PathBuf)>;

impl FilePrompt {
    pub(crate) fn what(&self) -> &'static str {
        self.what
    }

    pub(crate) fn waiting_for(&self) -> std::time::Duration {
        self.started.elapsed()
    }
}

/// Put the dialog up on its own thread and return the answer channel. Not on macOS, where AppKit
/// needs the main thread (and the portal bug does not exist).
pub(crate) fn ask_for_path(
    dialog: rfd::FileDialog,
    saving: bool,
) -> std::sync::mpsc::Receiver<Option<std::path::PathBuf>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let run = move || if saving { dialog.save_file() } else { dialog.pick_file() };
    #[cfg(target_os = "macos")]
    let _ = tx.send(run());
    #[cfg(not(target_os = "macos"))]
    std::thread::Builder::new()
        .name("simple3d-file-dialog".into())
        .spawn(move || {
            let _ = tx.send(run());
        })
        .expect("the platform can start a thread");
    rx
}
