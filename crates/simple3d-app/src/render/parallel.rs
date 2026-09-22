//! Spreading the per-triangle work of preparing a frame over the cores,
//! without changing what comes out.

use std::ops::Range;

/// How many items a thread must be given before starting one is worth it.
/// Below this the work is over in well under a millisecond on one core, and
/// spawning threads to share it out costs more than it saves.
const MIN_CHUNK: usize = 16_384;

/// Run `work` over `0..count` and append what it produces to `out`, in the
/// order a single pass from 0 to `count` would have produced it.
///
/// The range is cut into consecutive chunks, one per thread, and the chunks'
/// output is laid end to end afterwards -- so the result is the very list the
/// single-threaded loop makes, not merely the same set of steps. The drawing
/// order is part of the picture (see `Frame`), which is why the work is not
/// simply shared out and gathered as it finishes.
///
/// The gathering is itself done in parallel: a dense mesh prepares close to a
/// million steps, and copying those end to end on one core cost a good part of
/// what splitting the work had saved.
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
    // Every slot in `start..start + total` was written above: the parts are
    // laid over exactly that stretch of the spare capacity, end to end.
    unsafe { out.set_len(start + total) };
}

/// `f` applied to every index in `0..count`, collected in order.
pub(crate) fn map_in_order<T: Copy + Send + Sync>(count: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let mut out = Vec::with_capacity(count);
    extend_in_order(&mut out, count, |range, part| part.extend(range.map(&f)));
    out
}

fn chunk_count(count: usize) -> usize {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    (count / MIN_CHUNK).clamp(1, cores)
}
