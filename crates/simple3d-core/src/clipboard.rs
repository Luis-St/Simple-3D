//! Copy, cut and paste of whole subtrees (spec section 8.1).
//!
//! The payload is text in the same schema as the project file, so a selection
//! can be pasted into a text editor and back again. It is held in the
//! application's own clipboard rather than the system one -- the spec does not
//! require exchanging with other applications -- but the text form means doing
//! so later is a matter of handing this string to the platform.
//!
//! Pasting into the same parent applies **no offset**: the copy lands exactly on
//! the original. That is deliberate -- it is what makes copy, move, repeat work.
//! Only the name gets a suffix, so the outliner stays readable.

mod ops;
pub use ops::{carried_components, copy, insert, paste};
#[cfg(test)]
mod tests;

use crate::scene::{ComponentId, NodeData, SceneSettings};
use serde::{Deserialize, Serialize};

pub const CLIP_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Clip {
    pub format: u32,
    /// A multi-selection copies as a set and pastes as a set, preserving
    /// relative positions and original order.
    pub nodes: Vec<NodeData>,
    /// Every component an integration in `nodes` stands for, and every one
    /// those stand for in turn (issue 113), so a clip is whole wherever it is
    /// put: pasted into another project, or saved to the library and placed
    /// long after this one is closed. Absent for a clip without integrations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ClipComponent>,
    /// The project the clip was taken from, while it is open -- a paste back
    /// into it means the components it already has rather than copies of them.
    /// Never written anywhere: a saved primitive belongs to no project.
    #[serde(skip)]
    pub origin: Option<u64>,
}

/// One component a clip carries, under the id it had where it was copied.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClipComponent {
    pub id: ComponentId,
    pub settings: SceneSettings,
    pub root: NodeData,
}

impl Clip {
    pub fn to_text(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("a clip always serialises");
        text.push('\n');
        text
    }

    pub fn from_text(text: &str) -> Option<Clip> {
        let clip: Clip = serde_json::from_str(text).ok()?;
        if clip.format > CLIP_VERSION || clip.nodes.is_empty() {
            return None;
        }
        Some(clip)
    }
}
