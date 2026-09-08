//! Loops pinched at a shared vertex, and the fans that separate them.

use crate::vec3::Vec3;

/// Every vertex of the mesh lying strictly inside edge `e` of `tri`, in order
/// along the edge. Written into `out` rather than returned so the walk over a
/// large mesh allocates nothing per triangle.
/// Split a triangle's boundary into simple loops wherever it touches the same
/// vertex twice.
///
/// A vertex is spliced into the boundary of *every* edge it lies on, and near a
/// sharp corner it can genuinely lie on both edges meeting there. The BSP
/// produces needles routinely -- two long, nearly parallel sides and a very
/// short end, 2.3 mm long and 5 microns wide in the case that prompted this --
/// and around one of those a vertex sitting exactly on one long side is inside
/// the `tol` band of the other as well.
///
/// Removing one of the two occurrences is what the pass used to do, and it is
/// the reason a boolean of two finely tessellated operands came out
/// non-manifold. The decision is made per triangle, but an edge is shared with a
/// neighbour that has no reason to make the same one: whichever occurrence is
/// dropped, the triangle across that edge still splits there, and the two sides
/// no longer agree. It cannot be repaired by running the pass again either --
/// each run manufactures a fresh disagreement somewhere else, which is why
/// repeating it diverged instead of converging.
///
/// Keeping both occurrences makes every triangle agree with its neighbours,
/// because whether a vertex lies on a segment depends on the segment alone. What
/// it costs is a boundary that is pinched at that vertex, and a pinched loop
/// cannot be fanned as one polygon -- the two triangles either side of the pinch
/// would share an edge in the same direction. So it is cut into simple loops
/// here, and each is fanned separately.
///
/// Every vertex of the boundary lies on the original triangle's own sides, so
/// each loop is convex and its own centroid is strictly inside it -- which is
/// what makes fanning each piece from its own centre sound.
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

/// Triangulate a triangle's boundary once its edges have been subdivided, by
/// fanning it from a new vertex at the triangle's centre.
///
/// The obvious triangulations both fail here. A fan from one of the corners
/// leaves every split point on the two edges meeting at that corner sitting in
/// the interior of an emitted edge -- the T-junction is not removed, only
/// moved. An ear clipper stalls: a boundary that is a triangle's own sides is
/// convex but full of collinear triples, and on a boolean's sliver triangles
/// *every* triple comes out collinear to within the tolerance, so it gives up
/// and drops the face, which tears a hole in the surface. Both were measured
/// doing exactly that before this was written.
///
/// The centre point is inside the triangle by construction, a third of the
/// height away from each side, so every triangle of the fan has real area and
/// every split point is a corner of two of them. It costs one vertex per
/// subdivided face, and `retriangulate_flat_regions` -- which runs immediately
/// after and rebuilds each flat region from its boundary alone -- drops them
/// again.
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

/// Fan a loop from its own centroid, for the pieces a pinched boundary is cut
/// into. Unlike [`fan_from_centre`] there is no original triangle to take the
/// centre from -- the piece is only part of one -- but the piece is convex, so
/// the average of its own vertices is inside it.
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
