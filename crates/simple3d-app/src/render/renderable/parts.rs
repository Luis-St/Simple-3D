//! Which of the whole scene's welded vertices each node owns.

use super::*;

/// Which welded vertices are each node's ([`Renderable::parts`]).
///
/// Welded vertices are numbered by first appearance, so an unshared node range welds to a
/// contiguous range. A node is dropped when its vertices weld to ones outside it, as with two
/// touching bodies.
pub(super) fn parts_of(
    remap: &[u32],
    welded: usize,
    ranges: &BTreeMap<NodeId, Range<u32>>,
) -> BTreeMap<NodeId, Range<u32>> {
    if ranges.is_empty() {
        return BTreeMap::new();
    }
    // For each welded vertex, the first and last mesh vertices that went into it.
    let mut first = vec![u32::MAX; welded];
    let mut last = vec![0u32; welded];
    for (index, &to) in remap.iter().enumerate() {
        let to = to as usize;
        first[to] = first[to].min(index as u32);
        last[to] = index as u32;
    }
    let mut parts = BTreeMap::new();
    for (&id, range) in ranges {
        let (start, end) = (range.start as usize, range.end as usize);
        if start >= end || end > remap.len() {
            continue;
        }
        let lo = remap[start..end].iter().copied().min().unwrap_or(0) as usize;
        let hi = remap[start..end].iter().copied().max().unwrap_or(0) as usize + 1;
        // First copies come in order, so the lowest and highest decide; no copy may come after the range.
        let owned = first[lo] >= range.start && first[hi - 1] < range.end;
        if owned && last[lo..hi].iter().all(|&at| at < range.end) {
            parts.insert(id, lo as u32..hi as u32);
        }
    }
    parts
}
