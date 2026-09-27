//! Recording steps, and stepping back and forward through them.

use super::*;
use crate::scene::Scene;
use std::time::Instant;

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

    /// How many steps are on the undo stack, for checking gestures in tests.
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

    /// Record that the last step made `components` (issue 113), so undoing it removes them; the placed
    /// one first, then those it holds.
    pub fn mark_created(&mut self, components: &[ComponentId]) {
        if let Some(last) = self.past.last_mut() {
            last.created = components.to_vec();
        }
        // A component-making step stands alone, so later edits do not coalesce into it.
        self.close();
    }

    /// The components made by the step an undo would take back.
    pub fn undo_creates(&self) -> &[ComponentId] {
        self.past.last().map_or(&[], |s| s.created.as_slice())
    }

    /// The components made by the step a redo would restore.
    pub fn redo_creates(&self) -> &[ComponentId] {
        self.future.last().map_or(&[], |s| s.created.as_slice())
    }

    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
        self.open_key = None;
        self.open_at = None;
    }

    /// Call before mutating `scene`. `coalesce` groups consecutive edits of the same thing into one
    /// step; `None` always makes a new one. Returns whether the edit joined the open run.
    pub fn record(&mut self, scene: &Scene, label: &str, coalesce: Option<&str>) -> bool {
        self.revision += 1;
        let coalescing = match (coalesce, &self.open_key, self.open_at) {
            (Some(key), Some(open), Some(at)) => key == open && at.elapsed() < COALESCE_WINDOW,
            _ => false,
        };
        // Refresh the timer either way, so a held key keeps extending one step.
        self.open_key = coalesce.map(|k| k.to_string());
        self.open_at = Some(Instant::now());
        if coalescing {
            return true;
        }
        self.future.clear();
        self.past.push(Snapshot { label: label.to_string(), scene: scene.clone(), created: Vec::new() });
        if self.past.len() > self.depth {
            self.past.remove(0);
        }
        false
    }

    /// Drop the last snapshot without restoring it, for an abandoned edit such as an Escaped drag.
    /// The redo stack stays cleared.
    pub fn discard_last(&mut self) -> bool {
        self.close();
        self.revision += 1;
        self.past.pop().is_some()
    }

    /// End any open coalescing run, so the next edit starts a new step.
    pub fn close(&mut self) {
        self.open_key = None;
        self.open_at = None;
    }

    pub fn undo(&mut self, scene: &mut Scene) -> Option<String> {
        let snapshot = self.past.pop()?;
        self.close();
        self.revision += 1;
        self.future.push(Snapshot {
            label: snapshot.label.clone(),
            scene: scene.clone(),
            created: snapshot.created.clone(),
        });
        restore(scene, snapshot.scene);
        Some(snapshot.label)
    }

    pub fn redo(&mut self, scene: &mut Scene) -> Option<String> {
        let snapshot = self.future.pop()?;
        self.close();
        self.revision += 1;
        self.past.push(Snapshot {
            label: snapshot.label.clone(),
            scene: scene.clone(),
            created: snapshot.created.clone(),
        });
        restore(scene, snapshot.scene);
        Some(snapshot.label)
    }
}

/// Restore a snapshot, keeping the current camera, since undo is about the model, not the view.
/// Integrated components are also kept as they are now (issue 113), since they are edited in
/// their own tabs and a snapshot's copies would be stale.
pub(crate) fn restore(scene: &mut Scene, snapshot: Scene) {
    let camera = scene.camera;
    let components = std::mem::take(&mut scene.components);
    *scene = snapshot;
    scene.camera = camera;
    scene.components = components;
}
