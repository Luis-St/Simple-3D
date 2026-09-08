//! The depth of the stack, and the revision it reports.

use super::*;
use crate::scene::Scene;

#[test]
pub(crate) fn the_stack_is_bounded() {
    let mut scene = Scene::new();
    let mut history = History::new();
    for _ in 0..DEFAULT_DEPTH + 50 {
        history.record(&scene, "Add", None);
        shape(&mut scene);
    }
    assert_eq!(history.past.len(), DEFAULT_DEPTH);
}

#[test]
pub(crate) fn revision_changes_on_every_edit_and_on_undo() {
    let mut scene = Scene::new();
    let mut history = History::new();
    let start = history.revision();
    history.record(&scene, "Add", None);
    shape(&mut scene);
    assert_ne!(history.revision(), start);
    let after_edit = history.revision();
    history.undo(&mut scene);
    assert_ne!(history.revision(), after_edit);
}
