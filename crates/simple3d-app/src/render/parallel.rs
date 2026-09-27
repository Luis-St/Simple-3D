//! Spreading a frame's per-triangle preparation over the cores without changing its output.

use std::ops::Range;

/// The fewest items worth a thread; below this spawning costs more than it saves.
const MIN_CHUNK: usize = 16_384;

/// Run `work` over `0..count` and append its output to `out` in single-pass order, since drawing
/// order is part of the picture (see `Frame`). Chunks run per thread and are concatenated, also in
/// parallel, since copying a million steps on one core cost much of the gain.
pub(crate) fn extend_in_order<T: Copy + Send + Sync>(
    out: &mut Vec<T>,
    count: usize,
    work: impl Fn(Range<usize>, &mut Vec<T>) + Sync,
) {
    let chunks = chunk_count(count);
    if chunks <= 1 {
        work(0..count, out);
        return;
    }
    let work = &work;
    let parts: Vec<Vec<T>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..chunks)
            .map(|i| {
                let range = count * i / chunks..count * (i + 1) / chunks;
                scope.spawn(move || {
                    let mut part = Vec::new();
                    work(range, &mut part);
                    part
                })
            })
            .collect();
        handles.into_iter().map(|handle| handle.join().expect("a preparation thread panicked")).collect()
    });

    let total: usize = parts.iter().map(Vec::len).sum();
    let start = out.len();
    out.reserve(total);
    std::thread::scope(|scope| {
        let mut rest = &mut out.spare_capacity_mut()[..total];
        for part in &parts {
            let (mine, tail) = rest.split_at_mut(part.len());
            rest = tail;
            scope.spawn(move || {
                for (slot, value) in mine.iter_mut().zip(part) {
                    slot.write(*value);
                }
            });
        }
    });
    // Every slot in `start..start + total` was written above: the parts cover exactly that stretch of
    // the spare capacity, end to end.
    unsafe { out.set_len(start + total) };
}

/// `f` over `0..count`, collected in order.
pub(crate) fn map_in_order<T: Copy + Send + Sync>(count: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let mut out = Vec::with_capacity(count);
    extend_in_order(&mut out, count, |range, part| part.extend(range.map(&f)));
    out
}

fn chunk_count(count: usize) -> usize {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    (count / MIN_CHUNK).clamp(1, cores)
}
