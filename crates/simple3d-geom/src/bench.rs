#![cfg(test)]
//! Timings and soundness checks for the boolean kernel, so a change to it is
//! measured rather than argued about.
//!
//! `#[ignore]`d: the larger segment counts take minutes and there is nothing
//! here to assert in the ordinary suite. Run with
//!
//! ```text
//! cargo test --release -p simple3d-geom -- --ignored --nocapture bench
//! ```
//!
//! and always in release -- the kernel is float-heavy, and a debug build
//! measures the absence of optimisation rather than the algorithm.

mod audit;
mod holes;
mod operands;

use crate::BooleanOp;

use crate::mesh::Mesh;

fn union_of(a: &Mesh, b: &Mesh) -> Mesh {
    crate::evaluate_boolean(BooleanOp::Union, &[a.clone(), b.clone()])
}
