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
pub use ops::{copy, insert, paste};
#[cfg(test)]
mod tests;

use crate::scene::NodeData;
use serde::{Deserialize, Serialize};

pub const CLIP_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Clip {
    pub format: u32,
    /// A multi-selection copies as a set and pastes as a set, preserving
    /// relative positions and original order.
    pub nodes: Vec<NodeData>,
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
