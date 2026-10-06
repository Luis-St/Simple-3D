//! What an object holds on top of its own shape (issues 73 and 88): push/pull's swept faces and the
//! rounded or bevelled edges of the round tool, applied to the body's result in the order made.

use super::*;
use serde::{Deserialize, Serialize};

/// One edit kept on an object. Stored untagged, so files holding only push/pull edits read as before.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObjectEdit {
    Push(FaceEdit),
    Round(RoundEdit),
}

impl ObjectEdit {
    /// Whether the edit adds material anywhere, so the object's result can stand outside its operands.
    pub fn adds(&self) -> bool {
        match self {
            ObjectEdit::Push(edit) => edit.placing == Placing::Add,
            ObjectEdit::Round(edit) => edit.edges.iter().any(|edge| !edge.convex),
        }
    }

    /// The edit's one number: how far a face was swept, or the rounding's radius or bevel's distance.
    pub fn size(&self) -> f64 {
        match self {
            ObjectEdit::Push(edit) => edit.distance,
            ObjectEdit::Round(edit) => edit.size,
        }
    }

    /// The edit as a push or pull, if it is one.
    pub fn as_push(&self) -> Option<&FaceEdit> {
        match self {
            ObjectEdit::Push(edit) => Some(edit),
            ObjectEdit::Round(_) => None,
        }
    }

    /// The edit read from a file, or `None` when it could not be a solid.
    pub(crate) fn sane(self) -> Option<ObjectEdit> {
        match self {
            ObjectEdit::Push(edit) => edit.sane().map(ObjectEdit::Push),
            ObjectEdit::Round(edit) => edit.sane().map(ObjectEdit::Round),
        }
    }
}

impl Scene {
    /// Change one edit's number, kept above the smallest solid. `false` when there is no such edit.
    pub fn set_edit_size(&mut self, id: NodeId, index: usize, size: f64) -> bool {
        let Some(edit) = self.nodes.get_mut(&id).and_then(|node| node.edits.get_mut(index)) else { return false };
        if !size.is_finite() {
            return false;
        }
        match edit {
            ObjectEdit::Push(edit) => edit.distance = size.max(0.01),
            ObjectEdit::Round(edit) => edit.size = size.max(0.01),
        }
        true
    }

    /// Drop one edit, reverting the object to how it was without it.
    pub fn remove_edit(&mut self, id: NodeId, index: usize) -> Option<ObjectEdit> {
        let node = self.nodes.get_mut(&id)?;
        (index < node.edits.len()).then(|| node.edits.remove(index))
    }

    /// Add `edit` last on `id`. `false` when there is no such node.
    pub fn push_edit(&mut self, id: NodeId, edit: ObjectEdit) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else { return false };
        node.edits.push(edit);
        true
    }

    /// Put `edit` back at `index` on `id`, for a tool's draft that took it off. `false` when there is
    /// no such node or place.
    pub fn insert_edit(&mut self, id: NodeId, index: usize, edit: ObjectEdit) -> bool {
        let Some(node) = self.nodes.get_mut(&id).filter(|node| index <= node.edits.len()) else { return false };
        node.edits.insert(index, edit);
        true
    }

    /// Take the last edit on `id` equal to `edit` back off, for a tool's draft (issue 88). `false`
    /// when `id` holds no such edit.
    pub fn take_edit(&mut self, id: NodeId, edit: &ObjectEdit) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else { return false };
        let Some(at) = node.edits.iter().rposition(|e| e == edit) else { return false };
        node.edits.remove(at);
        true
    }
}
