//! Length-limited Huffman codes, canonical and fixed.

/// A Huffman table: a code and length per symbol, the code already bit-reversed for writing.
pub(super) struct Codes {
    pub(super) lengths: Vec<u8>,
    pub(super) codes: Vec<u16>,
}

/// Code lengths for `freqs`, none longer than `limit`.
///
/// Builds an ordinary Huffman tree, then clamps over-long lengths and repairs the table until it
/// is exactly full. Cheaper to implement than package-merge and within a fraction of a percent.
pub(super) fn huffman_lengths(freqs: &[u32], limit: usize) -> Vec<u8> {
    let mut lengths = vec![0u8; freqs.len()];
    let used: Vec<usize> = (0..freqs.len()).filter(|&symbol| freqs[symbol] > 0).collect();
    match used.len() {
        // Nothing to code, e.g. a literals-only block's distance alphabet.
        0 => return lengths,
        // A single symbol still needs a one-bit code; the incomplete table is accepted by every decoder
        // and written by every other encoder.
        1 => {
            lengths[used[0]] = 1;
            return lengths;
        }
        _ => {}
    }

    // The tree, as a heap of (weight, tie-breaker, node).
    let mut nodes: Vec<(u64, usize, usize)> = Vec::with_capacity(used.len() * 2);
    // Children of each internal node, and the depth pass that follows.
    let mut children: Vec<(usize, usize)> = Vec::new();
    for (order, &symbol) in used.iter().enumerate() {
        nodes.push((freqs[symbol] as u64, order, symbol));
    }
    let leaves = freqs.len();
    let mut heap: std::collections::BinaryHeap<std::cmp::Reverse<(u64, usize, usize)>> =
        nodes.into_iter().map(std::cmp::Reverse).collect();
    let mut order = used.len();
    while heap.len() > 1 {
        let std::cmp::Reverse((weight_a, _, a)) = heap.pop().expect("two or more nodes");
        let std::cmp::Reverse((weight_b, _, b)) = heap.pop().expect("two or more nodes");
        children.push((a, b));
        let node = leaves + children.len() - 1;
        heap.push(std::cmp::Reverse((weight_a + weight_b, order, node)));
        order += 1;
    }
    let std::cmp::Reverse((_, _, root)) = heap.pop().expect("one node is left");

    // Iterative, since a degenerate tree can be 285 deep.
    let mut depth = vec![0usize; leaves + children.len()];
    let mut stack = vec![(root, 0usize)];
    while let Some((node, at)) = stack.pop() {
        if node < leaves {
            depth[node] = at;
            lengths[node] = at.min(limit) as u8;
        } else {
            let (a, b) = children[node - leaves];
            stack.push((a, at + 1));
            stack.push((b, at + 1));
        }
    }

    // Clamping may over-fill the table. Repair: remove a code at the longest length, split a
    // shorter one to replace it, and repeat until exactly full.
    let mut counts = vec![0u32; limit + 2];
    for &symbol in &used {
        counts[lengths[symbol] as usize] += 1;
    }
    let full = 1u64 << limit;
    let mut total: u64 = (1..=limit).map(|length| (counts[length] as u64) << (limit - length)).sum();
    while total > full {
        counts[limit] -= 1;
        for length in (1..limit).rev() {
            if counts[length] != 0 {
                counts[length] -= 1;
                counts[length + 1] += 2;
                break;
            }
        }
        total -= 1;
    }
    // Under-filling cannot happen: the tree is complete and clamping only over-fills.
    debug_assert_eq!(total, full, "the repaired table is not a complete code");

    // Assign the longest lengths to the least frequent symbols, so the table stays optimal.
    let mut by_frequency = used.clone();
    by_frequency.sort_by_key(|&symbol| (std::cmp::Reverse(freqs[symbol]), symbol));
    let mut at = 0;
    for (length, &count) in counts.iter().enumerate().take(limit + 1).skip(1) {
        for _ in 0..count {
            lengths[by_frequency[at]] = length as u8;
            at += 1;
        }
    }
    debug_assert_eq!(at, used.len(), "every symbol that occurs must get a code");
    lengths
}

/// Canonical codes for a set of lengths.
pub(super) fn canonical(lengths: Vec<u8>) -> Codes {
    let longest = lengths.iter().copied().max().unwrap_or(0) as usize;
    let mut counts = vec![0u16; longest + 2];
    for &length in &lengths {
        if length > 0 {
            counts[length as usize] += 1;
        }
    }
    let mut next = vec![0u16; longest + 2];
    let mut code = 0u16;
    for length in 1..=longest {
        code = (code + counts[length - 1]) << 1;
        next[length] = code;
    }
    let mut codes = vec![0u16; lengths.len()];
    for (symbol, &length) in lengths.iter().enumerate() {
        if length > 0 {
            codes[symbol] = next[length as usize];
            next[length as usize] += 1;
        }
    }
    Codes { lengths, codes }
}

pub(super) fn fixed_tables() -> (Codes, Codes) {
    let mut literals = vec![0u8; 288];
    for (symbol, length) in literals.iter_mut().enumerate() {
        *length = match symbol {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    (canonical(literals), canonical(vec![5u8; 30]))
}
