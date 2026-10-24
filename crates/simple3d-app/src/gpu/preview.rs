//! A tool's preview loops, drawn on the card as resident lines.

use super::*;
use crate::render::{Renderable, Request};

impl Gpu {
    /// A tool's preview loops as card-resident lines, rebuilt only when the loops change.
    pub(super) fn refresh_preview(&mut self, request: &Request<'_>) {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for loop_ in &request.preview {
            loop_.len().hash(&mut hasher);
            for p in loop_ {
                [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()].hash(&mut hasher);
            }
        }
        let key = hasher.finish();
        if request.preview.is_empty() {
            self.preview = None;
            return;
        }
        if self.preview.as_ref().is_some_and(|(held, _)| *held == key) {
            return;
        }
        let mut positions = Vec::new();
        let mut edges = Vec::new();
        for loop_ in &request.preview {
            let first = positions.len() as u32;
            positions.extend_from_slice(loop_);
            let count = loop_.len() as u32;
            edges.extend((0..count).map(|index| [first + index, first + (index + 1) % count]));
        }
        self.preview = Some((key, Renderable::lines(positions, edges)));
    }
}
