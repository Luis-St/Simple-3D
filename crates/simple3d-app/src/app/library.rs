//! The user's own saved shapes.

use super::*;
use simple3d_core::clipboard::{self, Clip};
use simple3d_core::library;
use simple3d_core::scene::NodeId;

impl App {
    // -- the saved-primitive library ----------------------------------------

    pub fn refresh_library(&mut self) {
        self.library = library::list(&self.config_dir);
    }

    /// Open the naming window for the current selection.
    pub fn save_selection_as_primitive(&mut self) {
        match clipboard::copy(&self.scene, &self.selection) {
            Some(clip) => {
                let name = self.primary().map(|id| self.scene.node(id).name.clone()).unwrap_or_default();
                self.begin_save_primitive(clip, name);
            }
            None => self.status = Status::Warning("Select something to save as a primitive".into()),
        }
    }

    /// The same, for everything in the document. A project *is* a primitive as
    /// far as another project is concerned.
    pub fn save_project_as_primitive(&mut self) {
        let top: Vec<NodeId> = self.scene.node(self.scene.root()).children.clone();
        match clipboard::copy(&self.scene, &top) {
            Some(clip) => {
                let name = self
                    .path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Untitled".to_string());
                self.begin_save_primitive(clip, name);
            }
            None => self.status = Status::Warning("There is nothing in this project to save".into()),
        }
    }

    pub(super) fn begin_save_primitive(&mut self, clip: Clip, name: String) {
        self.primitive_name = name;
        self.primitive_clip = Some(clip);
        self.modal = Modal::SavePrimitive;
    }

    /// Write the pending subtree to the library under the typed name.
    pub fn confirm_save_primitive(&mut self) {
        let Some(clip) = self.primitive_clip.clone() else { return };
        let name = self.primitive_name.clone();
        match library::save(&self.config_dir, &name, &clip) {
            Ok(_) => {
                self.primitive_clip = None;
                self.modal = Modal::None;
                self.refresh_library();
                self.status =
                    Status::Info(format!("Saved \u{201C}{}\u{201D} to the palette", library::sanitise(&name)));
            }
            Err(e) => self.fail("Could not save the primitive", &format!("{name}\n\n{e}")),
        }
    }

    pub fn cancel_save_primitive(&mut self) {
        self.primitive_clip = None;
        self.modal = Modal::None;
    }

    /// Drop a saved primitive into the scene, at the current placement.
    pub fn add_library_entry(&mut self, entry: &library::Entry) {
        let Some(clip) = library::load(&entry.path) else {
            return self.fail(
                "Could not read that saved primitive",
                &format!("{}\n\nIt may have been written by a newer version, or edited by hand.", entry.path.display()),
            );
        };
        self.edit("Add", None);
        let target = self.primary();
        let created = clipboard::insert(&mut self.scene, &clip, target, false);
        self.size_fresh_patterns();
        if created.is_empty() {
            self.status = Status::Warning(format!("\u{201C}{}\u{201D} has nothing in it", entry.name));
            return;
        }
        // The saved subtree keeps its own internal arrangement; what moves is
        // where the whole thing sits.
        let anchor = self.scene.node(created[0]).position;
        // Its near side is measured across every node in it, so a saved
        // primitive stands clear of the selection as a whole rather than
        // leading with whichever node happens to be first.
        let near = self.near_face_x(&created).map_or(0.0, |x| x - anchor.x);
        let at = self.insertion_point_world(near);
        for id in &created {
            if let Some(node) = self.scene.get_mut(*id) {
                node.position = node.position - anchor + at;
            }
        }
        self.selection = created;
        self.on_selection_changed();
        self.status = Status::Info(format!("Added {}", entry.name));
    }

    pub fn delete_library_entry(&mut self, entry: &library::Entry) {
        match library::remove(&entry.path) {
            Ok(()) => {
                self.refresh_library();
                self.status = Status::Info(format!("Removed {} from the palette", entry.name));
            }
            Err(e) => self.fail("Could not remove that saved primitive", &format!("{}\n\n{e}", entry.path.display())),
        }
    }
}
