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

    pub fn save_selection_as_primitive(&mut self) {
        match clipboard::copy(&self.scene, &self.selection) {
            Some(clip) => {
                let name = self.primary().map(|id| self.scene.node(id).name.clone()).unwrap_or_default();
                self.begin_save_primitive(clip, name);
            }
            None => self.status = Status::Warning("Select something to save as a primitive".into()),
        }
    }

    /// The same for the whole document; a project is a primitive to another project.
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

    /// Place a saved primitive at the current placement as its own component (issue 113).
    pub fn add_library_entry(&mut self, entry: &library::Entry) {
        let Some(clip) = library::load(&entry.path) else {
            return self.fail(
                "Could not read that saved primitive",
                &format!("{}\n\nIt may have been written by a newer version, or edited by hand.", entry.path.display()),
            );
        };
        self.place_primitive(&clip, &entry.name);
    }

    /// Move freshly added nodes together to where a new shape goes.
    pub(crate) fn stand_clear(&mut self, created: &[NodeId]) {
        let Some(&first) = created.first() else { return };
        let anchor = self.scene.node(first).position;
        // Measured across all nodes, so the addition clears the selection as a whole.
        let near = self.near_face_x(created).map_or(0.0, |x| x - anchor.x);
        let at = self.insertion_point_world(near);
        for id in created {
            if let Some(node) = self.scene.get_mut(*id) {
                node.position = node.position - anchor + at;
            }
        }
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
