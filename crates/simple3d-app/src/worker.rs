//! Evaluation and export off the interaction path (spec sections 2.6, 5.2, 9).
//!
//! Both run on their own threads with progress and cancellation; an evaluation is superseded by a
//! newer edit, dropping the stale job rather than finishing it.

mod poll;
mod submit;
use poll::*;
mod export;
pub use export::ExportJob;
mod import;
pub use import::ImportJob;
mod reassemble;
pub use reassemble::ReassembleJob;
mod simplify;
pub use simplify::SimplifyJob;
mod split;
pub use split::SplitJob;
#[cfg(test)]
mod tests;

use crate::render::{Renderable, RenderableCache, Wanted};
use simple3d_core::eval::{Cancel, Evaluated};
use simple3d_core::scene::Scene;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

struct Job {
    scene: Scene,
    cancel: Cancel,
    generation: u64,
    /// The node renderables to prepare before the result arrives (see [`EvalWorker::want`]).
    wanted: Vec<Wanted>,
    renderables: RenderableCache,
}

pub struct Finished {
    pub result: Evaluated,
    /// The whole scene prepared for drawing, made here rather than on the interface thread, which
    /// paid for it on every result during a drag.
    pub renderable: Renderable,
    pub generation: u64,
    pub elapsed: Duration,
}

/// Owns the evaluation thread, where the `Evaluator` and its subtree cache live, so only changed
/// subtrees are recomputed.
pub struct EvalWorker {
    jobs: Sender<Job>,
    done: Receiver<Finished>,
    current: Option<Cancel>,
    generation: u64,
    /// The generation whose result is still awaited.
    outstanding: Option<u64>,
    /// The newest scene waiting for the current run; only one, since only the newest matters.
    pending: Option<Scene>,
    /// When the current job was submitted, so the footer can show the wait (there is no real progress).
    started: Option<Instant>,
    pub last_elapsed: Option<Duration>,
    /// The single-node renderables the interface draws, prepared by the evaluation thread per result.
    pub renderables: RenderableCache,
    wanted: Vec<Wanted>,
}
