//! Evaluation and export off the interaction path (spec sections 2.6, 5.2, 9).
//!
//! The interface must never freeze. Both the geometry evaluation and the export
//! run on their own threads, report progress, and can be cancelled; an
//! evaluation is additionally *superseded* cleanly when the user edits again
//! while one is running -- the worker drops the stale job rather than finishing it
//! and then throwing the answer away.

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
    /// The node renderables to have ready in `renderables` when the result
    /// arrives -- see [`EvalWorker::want`].
    wanted: Vec<Wanted>,
    renderables: RenderableCache,
}

pub struct Finished {
    pub result: Evaluated,
    /// The whole scene prepared for drawing. Made here rather than on the
    /// interface thread: on a large model it is a weld and several passes over
    /// every triangle, which the interface used to spend on the frame the
    /// result arrived in -- every frame, during a drag.
    pub renderable: Renderable,
    pub generation: u64,
    pub elapsed: Duration,
}

/// Owns the evaluation thread. The `Evaluator` -- and so the whole subtree cache
/// -- lives on that thread, which is what makes a one-dimension edit cheap: only
/// the subtrees whose content hash changed are recomputed.
pub struct EvalWorker {
    jobs: Sender<Job>,
    done: Receiver<Finished>,
    current: Option<Cancel>,
    generation: u64,
    /// The generation whose result we are still waiting for.
    outstanding: Option<u64>,
    /// The newest scene, waiting for the run in flight to finish.
    ///
    /// Only ever one: a drag submits on every frame, and what the viewport owes
    /// the user is the newest of those, not each of them.
    pending: Option<Scene>,
    /// When the job in flight was submitted, so the footer can say how long the
    /// user has been waiting. An evaluation has no honest progress to report --
    /// a boolean does not know how much of itself is left -- but it can always
    /// say how long it has been going.
    started: Option<Instant>,
    pub last_elapsed: Option<Duration>,
    /// What the interface draws single nodes with, and which of them it is
    /// drawing now: the evaluation thread prepares those for each result
    /// before handing it over.
    pub renderables: RenderableCache,
    wanted: Vec<Wanted>,
}
