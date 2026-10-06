//! An edge carried on along its line to where the body ends (issue 88).
//!
//! Treating one edge of a corner and then the other in a later step finds the second shortened: the
//! first treatment cut its end away, so its treatment stops short of the corner and leaves a stub of
//! it standing. Carried on to where the body near its line ends, it is treated as if whole.

use super::*;
use crate::mesh::Mesh;

/// `edge` lengthened at either end along its line while `mesh` has vertices within `reach` of the
/// line no further than `reach` beyond the end so far: to where the body around the edge ends.
pub fn extend_edge(mesh: &Mesh, edge: &FeatureEdge, reach: f64) -> FeatureEdge {
    let t = edge.direction();
    let length = edge.length();
    let mut along: Vec<f64> = mesh
        .positions
        .iter()
        .filter_map(|&p| {
            let s = (p - edge.a).dot(t);
            ((p - edge.a - t * s).length() <= reach + 1e-9).then_some(s)
        })
        .collect();
    along.sort_by(f64::total_cmp);
    let mut end = length;
    for &s in along.iter().filter(|&&s| s > length) {
        if s - end > reach + 1e-9 {
            break;
        }
        end = s;
    }
    let mut start = 0.0;
    for &s in along.iter().rev().filter(|&&s| s < 0.0) {
        if start - s > reach + 1e-9 {
            break;
        }
        start = s;
    }
    FeatureEdge { a: edge.a + t * start, b: edge.a + t * end, ..*edge }
}
