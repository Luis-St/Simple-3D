//! A renderable edited in place, and what of it changed: the brush's working copy (issue 127), whose
//! vertices move many times a second. The GPU renderer re-uploads only the changed stretches
//! (`gpu/resident/edit.rs`); the software renderer reads the renderable afresh every frame anyway.

use std::ops::Range;

/// Stretches closer than this many entries are sent as one: one upload call costs about as much as
/// sending a few thousand unchanged entries along.
const JOIN: u32 = 2048;

/// At most this many stretches go up at once; past it the closest ones are joined.
const MOST: usize = 64;

/// What changed since the card last had the renderable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edits {
    /// The revision the changes start from and the one they bring it to. A copy on the card at
    /// any other revision is out of step and is uploaded whole.
    pub since: u64,
    pub revision: u64,
    /// Vertices whose positions changed, as sorted, disjoint stretches.
    pub vertices: Vec<Range<u32>>,
    /// Triangles whose corner normals changed (`Renderable::corner_normals`).
    pub triangles: Vec<Range<u32>>,
}

impl Edits {
    /// Note a change to `vertices` and `triangles`, each sorted and without repeats.
    pub fn mark(&mut self, vertices: &[u32], triangles: &[u32]) {
        join(&mut self.vertices, vertices);
        join(&mut self.triangles, triangles);
        self.revision += 1;
    }

    /// The card has caught up: nothing is outstanding.
    pub fn sent(&mut self) {
        self.since = self.revision;
        self.vertices.clear();
        self.triangles.clear();
    }
}

/// Add the sorted `indices` to the stretches in `ranges`, joining those within [`JOIN`] and keeping
/// at most [`MOST`].
fn join(ranges: &mut Vec<Range<u32>>, indices: &[u32]) {
    let mut fresh: Vec<Range<u32>> = Vec::new();
    for &index in indices {
        match fresh.last_mut() {
            Some(last) if index <= last.end + JOIN => last.end = last.end.max(index + 1),
            _ => fresh.push(index..index + 1),
        }
    }
    ranges.extend(fresh);
    ranges.sort_unstable_by_key(|range| range.start);
    let mut gap = JOIN;
    loop {
        let mut joined: Vec<Range<u32>> = Vec::with_capacity(ranges.len());
        for range in ranges.drain(..) {
            match joined.last_mut() {
                Some(last) if range.start <= last.end + gap => last.end = last.end.max(range.end),
                _ => joined.push(range),
            }
        }
        *ranges = joined;
        if ranges.len() <= MOST {
            return;
        }
        gap *= 4;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearby_changes_are_sent_as_one_stretch_and_far_ones_apart() {
        let mut edits = Edits::default();
        edits.mark(&[5, 9, 100_000], &[]);
        edits.mark(&[7, 2_000, 50_000], &[3]);
        assert_eq!(edits.vertices, vec![5..2_001, 50_000..50_001, 100_000..100_001]);
        assert_eq!(edits.triangles, vec![3..4]);
        assert_eq!((edits.since, edits.revision), (0, 2));
        edits.sent();
        assert!(edits.since == edits.revision && edits.vertices.is_empty());
    }

    #[test]
    fn too_many_stretches_are_joined_until_few_enough() {
        let mut edits = Edits::default();
        let scattered: Vec<u32> = (0..1_000).map(|i| i * 10_000).collect();
        edits.mark(&scattered, &[]);
        assert!(edits.vertices.len() <= MOST);
        assert!(scattered.iter().all(|i| edits.vertices.iter().any(|range| range.contains(i))));
    }
}
