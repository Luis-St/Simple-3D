//! Undo and redo across every model-mutating action (spec section 7.4).
//!
//! Snapshots rather than inverse operations. A whole `Scene` for a 200-primitive
//! model is a few hundred kilobytes, and taking a copy is the only approach that
//! is *automatically* correct for every edit -- including reparenting, grouping a
//! multi-selection and cutting a subtree, which are exactly the operations an
//! inverse-command scheme gets subtly wrong.
//!
//! Rapid edits to one field coalesce into a single step: the caller passes a
//! coalesce key (`"param:7:width"`), and a second edit with the same key inside
//! the coalesce window reuses the snapshot already taken. A whole drag is one
//! step because the caller records once, before the drag starts.

use crate::scene::Scene;
use std::time::{Duration, Instant};

const DEFAULT_DEPTH: usize = 200;
const COALESCE_WINDOW: Duration = Duration::from_millis(900);

#[derive(Clone)]
struct Snapshot {
    label: String,
    scene: Scene,
}

pub struct History {
    past: Vec<Snapshot>,
    future: Vec<Snapshot>,
    depth: usize,
    open_key: Option<String>,
    open_at: Option<Instant>,
    /// Bumped on every recorded edit; the app compares it against the value it
    /// last saved to decide whether the title bar shows unsaved changes.
    revision: u64,
}

impl Default for History {
    fn default() -> Self {
        History::new()
    }
}

impl History {
    pub fn new() -> History {
        History {
            past: Vec::new(),
            future: Vec::new(),
            depth: DEFAULT_DEPTH,
            open_key: None,
            open_at: None,
            revision: 0,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// How many steps are on the undo stack. A gesture that should be one step
    /// can be checked against this rather than against how it feels.
    pub fn undo_len(&self) -> usize {
        self.past.len()
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.past.last().map(|s| s.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.future.last().map(|s| s.label.as_str())
    }

    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
        self.open_key = None;
        self.open_at = None;
    }

    /// Call **before** mutating `scene`. `coalesce` groups consecutive edits of
    /// the same thing into one step; pass `None` for an edit that always gets
    /// its own step.
    pub fn record(&mut self, scene: &Scene, label: &str, coalesce: Option<&str>) {
        self.revision += 1;
        let coalescing = match (coalesce, &self.open_key, self.open_at) {
            (Some(key), Some(open), Some(at)) => key == open && at.elapsed() < COALESCE_WINDOW,
            _ => false,
        };
        // Refresh the timer either way, so a held-down arrow key keeps extending
        // one step instead of splitting once the first press ages out.
        self.open_key = coalesce.map(|k| k.to_string());
        self.open_at = Some(Instant::now());
        if coalescing {
            return;
        }
        self.future.clear();
        self.past.push(Snapshot { label: label.to_string(), scene: scene.clone() });
        if self.past.len() > self.depth {
            self.past.remove(0);
        }
    }

    /// Drop the most recent snapshot without restoring it, for an edit that was
    /// recorded and then abandoned -- a drag cancelled with Escape, which puts
    /// the pre-drag values back itself. Keeping the snapshot would leave an undo
    /// step that restores the state it is already in.
    ///
    /// Does *not* touch the redo stack: `record` cleared it when the abandoned
    /// edit opened, and a cancel cannot bring it back.
    pub fn discard_last(&mut self) -> bool {
        self.close();
        self.revision += 1;
        self.past.pop().is_some()
    }

    /// Ends any open coalescing run, so the next edit definitely starts a new
    /// step. Called when the selection changes or a field loses focus.
    pub fn close(&mut self) {
        self.open_key = None;
        self.open_at = None;
    }

    pub fn undo(&mut self, scene: &mut Scene) -> Option<String> {
        let snapshot = self.past.pop()?;
        self.close();
        self.revision += 1;
        self.future.push(Snapshot { label: snapshot.label.clone(), scene: scene.clone() });
        restore(scene, snapshot.scene);
        Some(snapshot.label)
    }

    pub fn redo(&mut self, scene: &mut Scene) -> Option<String> {
        let snapshot = self.future.pop()?;
        self.close();
        self.revision += 1;
        self.past.push(Snapshot { label: snapshot.label.clone(), scene: scene.clone() });
        restore(scene, snapshot.scene);
        Some(snapshot.label)
    }
}

/// Put a snapshot back, **keeping the camera where it is now**.
///
/// A snapshot is the whole `Scene`, camera included, because the camera is
/// saved with the project. Restoring it wholesale would mean undoing a move
/// also threw the view back to wherever it happened to be when the move was
/// made -- which is not what "undo" means to anyone. Undo is over the model;
/// where you are looking from is not part of it.
fn restore(scene: &mut Scene, snapshot: Scene) {
    let camera = scene.camera;
    *scene = snapshot;
    scene.camera = camera;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::ParamValue;
    use crate::scene::{Anchor, GroupOp, NodeId};
    use simple3d_geom::Vec3;

    fn shape(scene: &mut Scene) -> NodeId {
        let root = scene.root();
        let index = scene.node(root).children.len();
        scene.add_primitive("box", root, index).unwrap()
    }

    /// A structural fingerprint of the tree, so "identical" can be asserted
    /// without relying on node ids being reused in the same order.
    fn fingerprint(scene: &Scene) -> String {
        let mut out = String::new();
        for id in scene.depth_first() {
            let node = scene.node(id);
            out.push_str(&format!(
                "{}|{}|{:?}|{:?}|{:?}|{:?}|{}|{}|{:?};",
                scene.depth(id),
                node.name,
                node.position,
                node.rotation,
                node.scale,
                node.anchor,
                node.visible,
                node.segments.unwrap_or(0),
                node.params().cloned().unwrap_or_default(),
            ));
        }
        out
    }

    #[test]
    fn undo_and_redo_leave_the_camera_exactly_where_it_is() {
        // The camera is part of the saved project and therefore part of every
        // snapshot, but it is not part of what an edit did. Undoing a move must
        // not also throw the view back to where it was looking from.
        let mut scene = Scene::new();
        let mut history = History::new();
        let id = shape(&mut scene);
        history.record(&scene, "Move", None);
        scene.get_mut(id).unwrap().position = Vec3::new(10.0, 0.0, 0.0);

        // The user then orbits and zooms.
        let looking = crate::scene::Camera { yaw: 12.5, pitch: -40.0, distance: 999.0, ..Default::default() };
        scene.camera = looking;

        assert_eq!(history.undo(&mut scene).as_deref(), Some("Move"));
        assert_eq!(scene.camera, looking, "undo moved the camera");
        assert_eq!(scene.node(id).position, Vec3::ZERO, "undo did not restore the model");

        assert_eq!(history.redo(&mut scene).as_deref(), Some("Move"));
        assert_eq!(scene.camera, looking, "redo moved the camera");
        assert_eq!(scene.node(id).position, Vec3::new(10.0, 0.0, 0.0));
    }

    #[test]
    fn twenty_mixed_edits_undo_and_redo_to_the_same_tree() {
        // Spec acceptance criterion 11.
        let mut scene = Scene::new();
        let mut history = History::new();
        let root = scene.root();

        let mut ids = Vec::new();
        for i in 0..6 {
            history.record(&scene, "Add box", None);
            let id = shape(&mut scene);
            scene.get_mut(id).unwrap().position = Vec3::new(i as f64 * 10.0, 0.0, 0.0);
            ids.push(id);
        }
        history.record(&scene, "Group", None);
        let group = scene.group_selection(&ids[..3]).unwrap();
        history.record(&scene, "Set operation", None);
        scene.get_mut(group).unwrap().body = crate::scene::Body::Group { op: GroupOp::Difference };
        history.record(&scene, "Reparent", None);
        scene.reparent(ids[3], group, 0).unwrap();
        history.record(&scene, "Rename", None);
        scene.get_mut(ids[4]).unwrap().name = "Renamed".into();
        history.record(&scene, "Hide", None);
        scene.get_mut(ids[5]).unwrap().visible = false;
        history.record(&scene, "Anchor", None);
        scene.get_mut(ids[0]).unwrap().anchor = Anchor::Base;
        history.record(&scene, "Rotate", None);
        scene.get_mut(ids[1]).unwrap().rotation = Vec3::new(0.0, 45.0, 0.0);
        history.record(&scene, "Segments", None);
        scene.get_mut(ids[2]).unwrap().segments = Some(64);
        for i in 0..5 {
            history.record(&scene, "Set width", None);
            scene
                .get_mut(ids[i])
                .unwrap()
                .params_mut()
                .unwrap()
                .insert("width".into(), ParamValue::Length(30.0 + i as f64));
        }
        history.record(&scene, "Duplicate", None);
        scene.duplicate(ids[5]).unwrap();
        history.record(&scene, "Reorder", None);
        scene.reorder(ids[4], -1);
        history.record(&scene, "Delete", None);
        scene.remove(ids[5]);

        let after_all = fingerprint(&scene);
        // Twenty-two edits, comfortably past the criterion's twenty.
        let steps = history.past.len();
        assert!(steps >= 20, "only {steps} snapshots");
        for _ in 0..steps {
            history.undo(&mut scene);
        }
        assert!(!history.can_undo(), "more snapshots than edits");
        assert_eq!(fingerprint(&scene), fingerprint(&Scene::new()), "undoing everything did not empty the scene");
        for _ in 0..steps {
            history.redo(&mut scene);
        }
        assert_eq!(fingerprint(&scene), after_all);
        let _ = root;
    }

    #[test]
    fn rapid_edits_to_one_field_are_one_step() {
        // Spec section 7.4: "Rapid edits to one field coalesce".
        let mut scene = Scene::new();
        let mut history = History::new();
        let id = shape(&mut scene);
        let before = fingerprint(&scene);

        for width in [21.0, 22.0, 23.0, 24.0] {
            history.record(&scene, "Set width", Some("param:1:width"));
            scene.get_mut(id).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(width));
        }
        history.undo(&mut scene);
        assert_eq!(fingerprint(&scene), before);
        assert!(!history.can_undo());
    }

    #[test]
    fn edits_to_different_fields_do_not_coalesce() {
        let mut scene = Scene::new();
        let mut history = History::new();
        let id = shape(&mut scene);
        history.record(&scene, "Set width", Some("param:1:width"));
        scene.get_mut(id).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(30.0));
        history.record(&scene, "Set depth", Some("param:1:depth"));
        scene.get_mut(id).unwrap().params_mut().unwrap().insert("depth".into(), ParamValue::Length(30.0));

        history.undo(&mut scene);
        assert_eq!(scene.node(id).params().unwrap()["depth"], ParamValue::Length(20.0));
        assert_eq!(scene.node(id).params().unwrap()["width"], ParamValue::Length(30.0));
        assert!(history.can_undo());
    }

    #[test]
    fn closing_the_run_splits_coalescing() {
        let mut scene = Scene::new();
        let mut history = History::new();
        let id = shape(&mut scene);
        history.record(&scene, "Set width", Some("param:1:width"));
        scene.get_mut(id).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(30.0));
        history.close();
        history.record(&scene, "Set width", Some("param:1:width"));
        scene.get_mut(id).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(40.0));
        assert_eq!(history.past.len(), 2);
    }

    #[test]
    fn cutting_and_pasting_undoes_in_two_steps() {
        // Spec acceptance criterion 22.
        let mut scene = Scene::new();
        let mut history = History::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        let child = shape(&mut scene);
        scene.reparent(child, group, 0).unwrap();
        let other = scene.add_group(GroupOp::Union, root, 1);
        let original = fingerprint(&scene);

        history.record(&scene, "Cut", None);
        let data = scene.export_subtree(group).unwrap();
        scene.remove(group);
        history.record(&scene, "Paste", None);
        scene.import_subtree(&data, other, 0).unwrap();

        history.undo(&mut scene);
        history.undo(&mut scene);
        assert_eq!(fingerprint(&scene), original);
    }

    #[test]
    fn a_new_edit_discards_the_redo_stack() {
        let mut scene = Scene::new();
        let mut history = History::new();
        history.record(&scene, "Add", None);
        shape(&mut scene);
        history.undo(&mut scene);
        assert!(history.can_redo());
        history.record(&scene, "Add", None);
        shape(&mut scene);
        assert!(!history.can_redo());
    }

    #[test]
    fn the_stack_is_bounded() {
        let mut scene = Scene::new();
        let mut history = History::new();
        for _ in 0..DEFAULT_DEPTH + 50 {
            history.record(&scene, "Add", None);
            shape(&mut scene);
        }
        assert_eq!(history.past.len(), DEFAULT_DEPTH);
    }

    #[test]
    fn revision_changes_on_every_edit_and_on_undo() {
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
}
