//! The saved-primitive library: groups or whole projects kept for reuse in any project.
//!
//! Each entry is a file in `library/` under the config directory holding a clipboard `Clip`, in the
//! project file's schema. Per user, not per document, so nothing here is undoable.

use crate::clipboard::Clip;
use crate::named_files;
pub use crate::named_files::{remove, sanitise, Entry};
use std::io;
use std::path::{Path, PathBuf};

const DIRECTORY: &str = "library";

pub fn dir(config_dir: &Path) -> PathBuf {
    config_dir.join(DIRECTORY)
}

/// Every saved primitive by name; unreadable files are skipped so the palette still draws.
pub fn list(config_dir: &Path) -> Vec<Entry> {
    named_files::list(&dir(config_dir))
}

/// Whether a name is taken, so saving can warn before overwriting.
pub fn exists(config_dir: &Path, name: &str) -> bool {
    named_files::path_for(&dir(config_dir), name).exists()
}

/// Write a clip to the library as `name`, replacing any same-named entry.
pub fn save(config_dir: &Path, name: &str, clip: &Clip) -> io::Result<PathBuf> {
    let path = named_files::prepare_save(&dir(config_dir), name, "saved primitive")?;
    std::fs::write(&path, clip.to_text())?;
    Ok(path)
}

pub fn load(path: &Path) -> Option<Clip> {
    Clip::from_text(&std::fs::read_to_string(path).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard;
    use crate::scene::Scene;
    use crate::temp_dir;

    fn a_clip() -> (Scene, Clip) {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(crate::scene::GroupOp::Union, root, 0);
        let child = scene.add_primitive("box", group, 0).unwrap();
        scene.get_mut(child).unwrap().position = simple3d_geom::Vec3::new(3.0, 0.0, 0.0);
        let clip = clipboard::copy(&scene, &[group]).unwrap();
        (scene, clip)
    }

    #[test]
    fn a_saved_primitive_comes_back_as_the_same_subtree() {
        // What was saved from one project arrives in another intact.
        let dir = temp_dir("round-trip");
        let (_, clip) = a_clip();
        save(&dir, "Bracket", &clip).unwrap();

        let entries = list(&dir);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Bracket");

        let loaded = load(&entries[0].path).expect("the saved entry could not be read back");
        let mut fresh = Scene::new();
        let created = clipboard::insert(&mut fresh, &loaded, None, false);
        assert_eq!(created.len(), 1);
        let group = created[0];
        assert_eq!(fresh.node(group).name, "Group", "a saved primitive arrived renamed as a copy of something");
        assert!(fresh.node(group).is_group());
        assert_eq!(fresh.node(group).children.len(), 1);
        let child = fresh.node(group).children[0];
        assert_eq!(fresh.node(child).position, simple3d_geom::Vec3::new(3.0, 0.0, 0.0));
    }

    #[test]
    fn saving_the_same_name_twice_replaces_the_entry_rather_than_adding_one() {
        let dir = temp_dir("replace");
        let (_, clip) = a_clip();
        save(&dir, "Bracket", &clip).unwrap();
        assert!(exists(&dir, "Bracket"));
        save(&dir, "Bracket", &clip).unwrap();
        assert_eq!(list(&dir).len(), 1, "the second save added a second entry");
    }

    #[test]
    fn a_name_that_would_not_be_a_file_name_is_tidied_rather_than_refused() {
        let dir = temp_dir("names");
        let (_, clip) = a_clip();
        let path = save(&dir, "part/two: draft?", &clip).unwrap();
        assert_eq!(path.parent().unwrap(), super::dir(&dir), "the name escaped the library directory");
        assert_eq!(list(&dir)[0].name, "part-two- draft-");

        // An empty name is refused, not saved as a dotfile.
        assert!(save(&dir, "   ", &clip).is_err());
        assert!(save(&dir, "...", &clip).is_err());
    }

    #[test]
    fn an_empty_or_unreadable_library_is_simply_empty() {
        let dir = temp_dir("empty");
        assert!(list(&dir).is_empty(), "a library directory that does not exist is not an error");

        std::fs::create_dir_all(super::dir(&dir)).unwrap();
        std::fs::write(super::dir(&dir).join("junk.json"), "not a clip").unwrap();
        std::fs::write(super::dir(&dir).join("notes.txt"), "ignored").unwrap();
        let entries = list(&dir);
        assert_eq!(entries.len(), 1, "the .txt should not be listed");
        assert!(load(&entries[0].path).is_none(), "unreadable content should not load as an entry");
    }

    #[test]
    fn an_entry_can_be_removed() {
        let dir = temp_dir("remove");
        let (_, clip) = a_clip();
        save(&dir, "Bracket", &clip).unwrap();
        remove(&list(&dir)[0].path).unwrap();
        assert!(list(&dir).is_empty());
    }
}
