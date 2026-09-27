#![cfg(test)]
//! Timings and soundness checks for the boolean kernel. `#[ignore]`d for their runtime; run in release:
//!
//! ```text
//! cargo test --release -p simple3d-geom -- --ignored --nocapture bench
//! ```

mod audit;
mod holes;
mod operands;

use crate::BooleanOp;

use crate::mesh::Mesh;

fn union_of(a: &Mesh, b: &Mesh) -> Mesh {
    crate::evaluate_boolean(BooleanOp::Union, &[a.clone(), b.clone()])
}
