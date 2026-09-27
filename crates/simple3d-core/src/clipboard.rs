//! Copy, cut and paste of whole subtrees (spec section 8.1).
//!
//! The payload is text in the project file's schema, held in the app's own clipboard. Pasting into
//! the same parent applies no offset, so copy, move, repeat works; only the name gets a suffix.

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
    /// A multi-selection copies and pastes as a set, keeping relative positions and order.
    pub nodes: Vec<NodeData>,
    /// Every component the clip's integrations need, transitively (issue 113), so the clip is whole
    /// anywhere; absent without integrations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ClipComponent>,
    /// The source project while open, so pasting back reuses its components. Never written.
    #[serde(skip)]
    pub origin: Option<u64>,
}

/// One component a clip carries, under its id at the source.
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
