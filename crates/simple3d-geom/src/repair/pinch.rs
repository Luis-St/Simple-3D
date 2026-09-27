//! Loops pinched at a shared vertex, and the fans that separate them.

use crate::vec3::Vec3;

/// Split a triangle's boundary into simple loops wherever it touches the same vertex twice.
///
/// Near a sharp corner of a BSP needle a vertex can lie on both adjacent edges and is spliced into
/// both. Dropping one occurrence made neighbours disagree and the mesh non-manifold (re-running
/// diverged). Keeping both keeps neighbours consistent but pinches the boundary, so it is cut into
/// simple loops, each convex and fanned from its own centroid.
pub(crate) fn split_pinched_loops(boundary: &[u32]) -> Vec<Vec<u32>> {
    let mut loops: Vec<Vec<u32>> = Vec::new();
    let mut stack: Vec<u32> = Vec::with_capacity(boundary.len());
    for &v in boundary {
        if let Some(at) = stack.iter().position(|&w| w == v) {
            // Everything since the last visit to `v` closes a loop of its own.
            let piece: Vec<u32> = stack[at..].to_vec();
            if piece.len() >= 3 {
                loops.push(piece);
            }
            stack.truncate(at);
        }
        stack.push(v);
    }
    if stack.len() >= 3 {
        loops.push(stack);
    }
    loops
}

/// Triangulate a subdivided triangle's boundary by fanning from a new centre vertex.
///
/// A corner fan only moves the T-junctions, and an ear clipper stalls on sliver triangles whose
/// triples all look collinear, dropping the face. The centre is a third of the height from each
/// side, so every fan triangle has area. `retriangulate_flat_regions` removes the extra vertex.
pub(crate) fn fan_from_centre(
    pos: &[Vec3],
    tri: &[u32; 3],
    boundary: &[u32],
    tag: u32,
    positions: &mut Vec<Vec3>,
    indices: &mut Vec<[u32; 3]>,
    tags: &mut Vec<u32>,
) {
    let centre = (pos[tri[0] as usize] + pos[tri[1] as usize] + pos[tri[2] as usize]) / 3.0;
    positions.push(centre);
    let c = (positions.len() - 1) as u32;
    for i in 0..boundary.len() {
        let (a, b) = (boundary[i], boundary[(i + 1) % boundary.len()]);
        indices.push([c, a, b]);
        tags.push(tag);
    }
}

/// Fan a loop from its own centroid, for the pieces of a pinched boundary; each piece is convex.
pub(crate) fn fan_loop_from_own_centre(
    pos: &[Vec3],
    loop_: &[u32],
    tag: u32,
    positions: &mut Vec<Vec3>,
    indices: &mut Vec<[u32; 3]>,
    tags: &mut Vec<u32>,
) {
    if loop_.len() < 3 {
        return;
    }
    let mut centre = Vec3::ZERO;
    for &v in loop_ {
        centre = centre + pos[v as usize];
    }
    let centre = centre / loop_.len() as f64;
    positions.push(centre);
    let c = (positions.len() - 1) as u32;
    for i in 0..loop_.len() {
        let (a, b) = (loop_[i], loop_[(i + 1) % loop_.len()]);
        indices.push([c, a, b]);
        tags.push(tag);
    }
}
