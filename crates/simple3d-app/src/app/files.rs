//! Opening and saving, and the file dialog that runs while the application
//! keeps drawing.

use super::*;
use simple3d_core::project;
use std::path::Path;

impl App {
    /// Put a file dialog up and say what to do with the path it answers with.
    ///
    /// `what` names the wait for the footer, and is what a cancelled dialog
    /// reports having cancelled -- silence there is the other half of the bug
    /// this fixes: a portal that answered with nothing left Ctrl+S on an unsaved
    /// document with no dialog *and* no message, which is indistinguishable from
    /// a save the user cancelled on purpose.
    ///
    /// One at a time. A second dialog while one is up would be two windows
    /// asking the same question, and only one answer could be acted on.
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

    /// Act on a file dialog that has answered. Called once a frame, beside the
    /// export job's own poll.
    pub(crate) fn poll_file_prompt(&mut self) {
        let Some(prompt) = &self.file_prompt else { return };
        let answer = match prompt.answer.try_recv() {
            Ok(answer) => answer,
            // The dialog thread went away without answering, which is the same
            // outcome as a cancel and is reported as one.
            Err(std::sync::mpsc::TryRecvError::Disconnected) => None,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
        };
        let prompt = self.file_prompt.take().expect("checked just above");
        match answer {
            Some(path) => (prompt.then)(self, path),
            None => self.status = Status::Info(format!("{} cancelled", prompt.what)),
        }
    }

    /// Stop waiting on the dialog in flight.
    ///
    /// The thread stays parked until the portal answers, if it ever does, and
    /// its answer is dropped: what it is holding is a channel nobody is
    /// listening to any more.
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

    /// Read `path` into the document on screen, replacing whatever it held.
    /// Which tab that is, `crate::tabs::open_path` has already decided.
    pub(crate) fn load_into_active(&mut self, path: &Path) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                self.settings.forget_recent(path);
                return self.fail("Could not open the project", &format!("{}\n\n{e}", path.display()));
            }
        };
        match project::from_str(&text) {
            Ok(scene) => {
                self.scene = scene;
                // The file's own camera stands, whether it was opened from the
                // menu or handed to the binary on the command line.
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

    pub(super) fn save_to(&mut self, path: &Path) {
        let text = project::to_string(&self.scene);
        match std::fs::write(path, text) {
            Ok(()) => {
                self.path = Some(path.to_path_buf());
                self.saved_revision = self.history.revision();
                self.settings.remember_recent(path);
                self.status = Status::Info(format!("Saved {}", path.display()));
            }
            Err(e) => self.fail("Could not save the project", &format!("{}\n\n{e}", path.display())),
        }
    }

    pub fn fail(&mut self, title: &str, detail: &str) {
        self.error_title = title.to_string();
        self.error_detail = detail.to_string();
        self.modal = Modal::Error;
        self.status = Status::Warning(title.to_string());
    }
}

/// A file dialog that has been put up, and what to do with the path it comes
/// back with.
///
/// The dialog used to be called straight from `App::update`. `rfd` 0.17 is in
/// the lock file with neither `ashpd` nor `gtk`, so on Linux it is the raw
/// D-Bus portal backend: it talks to the portal itself and waits on `pollster`,
/// on the calling thread. A portal that is slow, absent or confused therefore
/// took the whole application down with it -- the last frame stayed on screen
/// with its hover states frozen mid-frame, and the process had to be killed.
///
/// It waits on its own thread now. The window keeps drawing, the footer says
/// what is being waited for and offers a way to stop waiting, and an answer
/// that never comes costs nothing but a parked thread.
pub(crate) struct FilePrompt {
    pub(super) what: &'static str,
    pub(super) answer: std::sync::mpsc::Receiver<Option<std::path::PathBuf>>,
    pub(super) then: FollowUp,
    pub(super) started: std::time::Instant,
}

/// What to do with the path a dialog answers with, on the frame it arrives.
/// Runs on the interaction thread, so it can touch the whole application.
pub(crate) type FollowUp = Box<dyn FnOnce(&mut App, std::path::PathBuf)>;

impl FilePrompt {
    pub(crate) fn what(&self) -> &'static str {
        self.what
    }

    pub(crate) fn waiting_for(&self) -> std::time::Duration {
        self.started.elapsed()
    }
}

/// Put the dialog up on a thread of its own and hand back the channel its
/// answer will arrive on.
///
/// Not on macOS, where AppKit requires a file dialog to be run from the main
/// thread and moving it would be a crash rather than a fix. The bug this
/// addresses is the Linux portal's, and the platform that cannot have the fix
/// is the one that does not have the bug.
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
