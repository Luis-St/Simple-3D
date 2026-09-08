//! Cutting a body into its cells, and measuring what came out.

use super::*;
use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// Cut a solid into the pieces one cell of the tiling each, in a stable order.
///
/// `report` is called once per cell as it is finished, and `give_up` is asked
/// often enough that a split of thousands of cells can be abandoned promptly;
/// giving up returns `None`, and what had been cut so far is dropped rather
/// than handed back as a half-cut shape.
///
/// Every returned piece has geometry in it: a cell the shape does not reach is
/// not a piece, and neither is one it touches with no volume.
pub fn cut(
    mesh: &Mesh,
    tiling: &Tiling,
    report: &(dyn Fn() + Sync),
    give_up: &(dyn Fn() -> bool + Sync),
) -> Option<Vec<Mesh>> {
    let Some(bounds) = mesh.bounds() else { return Some(Vec::new()) };
    cut_within(mesh, tiling, bounds, report, give_up)
}

/// Cut a solid by a tiling laid out over `frame` rather than over the solid
/// itself.
///
/// The two are the same thing for a shape being cut on its own, and they are
/// not for the second cut of a plan: that one cuts the *pieces* the first left,
/// and a lattice anchored on each piece in turn is a different lattice for
/// every piece -- nine columns of a plate would each be cut into a grid centred
/// on themselves, and the cuts would not line up across the shape. The frame is
/// the whole shape, so every piece is cut by the same grid.
pub(crate) fn cut_within(
    mesh: &Mesh,
    tiling: &Tiling,
    frame: (Vec3, Vec3),
    report: &(dyn Fn() + Sync),
    give_up: &(dyn Fn() -> bool + Sync),
) -> Option<Vec<Mesh>> {
    let Some(bounds) = mesh.bounds() else { return Some(Vec::new()) };
    if tiling.refusal(frame).is_some() {
        return Some(Vec::new());
    }
    // Planned over the whole frame, kept where it reaches this solid: what is
    // left is this piece's share of the one grid.
    let cells: Vec<Cell> = plan(tiling, frame).into_iter().filter(|cell| reaches(cell.bounds, bounds)).collect();
    // The box of every triangle, once. A cell that overlaps none of them holds
    // no surface at all, which is the question asked of every cell and the one
    // that keeps the inside of a large shape free.
    let faces: Vec<(Vec3, Vec3)> = mesh
        .indices
        .iter()
        .map(|t| {
            let (a, b, c) =
                (mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]);
            (a.min(b).min(c), a.max(b).max(c))
        })
        .collect();

    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, cells.len().max(1));
    let mut batches: Vec<Vec<(usize, Mesh)>> = Vec::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let (cells, faces, mesh) = (&cells, &faces, &mesh);
                scope.spawn(move || {
                    let mut mine = Vec::new();
                    // Every nth cell rather than a block of them: the cells that
                    // cost anything are the ones along the surface, and they
                    // arrive in runs, so a block each would leave one thread
                    // with all of them.
                    for index in (t..cells.len()).step_by(threads) {
                        if give_up() {
                            break;
                        }
                        if let Some(piece) = cut_cell(mesh, faces, bounds, &cells[index], tiling, give_up) {
                            mine.push((index, piece));
                        }
                        report();
                    }
                    mine
                })
            })
            .collect();
        batches = handles.into_iter().map(|h| h.join().unwrap_or_default()).collect();
    });
    if give_up() {
        return None;
    }
    // Back into the order the cells were planned in, so the same split of the
    // same shape always names the same piece "3".
    let mut pieces: Vec<(usize, Mesh)> = batches.into_iter().flatten().collect();
    pieces.sort_by_key(|(index, _)| *index);
    Some(pieces.into_iter().map(|(_, mesh)| mesh).collect())
}

/// The piece one cell holds, if it holds one.
pub(crate) fn cut_cell(
    mesh: &Mesh,
    faces: &[(Vec3, Vec3)],
    bounds: (Vec3, Vec3),
    cell: &Cell,
    tiling: &Tiling,
    give_up: &(dyn Fn() -> bool + Sync),
) -> Option<Mesh> {
    if !crate::boxes_overlap(cell.bounds, bounds) {
        return None;
    }
    // A cell no face comes near is wholly inside the shape or wholly outside
    // it, and one ray says which. Inside, the piece *is* the cell -- no boolean
    // is run at all, which is what makes a solid block affordable to cut up.
    if !faces.iter().any(|face| crate::boxes_overlap(*face, cell.bounds)) {
        let centre = tiling.to_world(cell.centre.0, cell.centre.1, (cell.span.0 + cell.span.1) / 2.0);
        return inside(mesh, centre).then(|| cell.prism(tiling));
    }
    let piece = crate::csg_bsp::intersect_until(mesh, &cell.prism(tiling), &|| give_up());
    // A cell that only grazes the surface comes back as a sliver of no volume,
    // or as nothing at all. Neither is a piece anybody asked for.
    (volume(&piece) > 1e-9).then_some(piece)
}

/// Whether a point is inside a closed mesh, by counting the faces a ray from it
/// crosses: an odd count is inside.
///
/// The direction is a fixed skew one so that it cannot run along a face or
/// through an edge of an axis-aligned shape, which is what every shape in a
/// document like this one is.
pub(crate) fn inside(mesh: &Mesh, point: Vec3) -> bool {
    let dir = Vec3::new(0.5773502691896258, 0.3313, 0.7443).normalized();
    let mut crossings = 0;
    for tri in &mesh.indices {
        let (a, b, c) =
            (mesh.positions[tri[0] as usize], mesh.positions[tri[1] as usize], mesh.positions[tri[2] as usize]);
        let (e1, e2) = (b - a, c - a);
        let h = dir.cross(e2);
        let det = e1.dot(h);
        if det.abs() < 1e-12 {
            continue;
        }
        let inv = 1.0 / det;
        let s = point - a;
        let u = s.dot(h) * inv;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = s.cross(e1);
        let v = dir.dot(q) * inv;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        if e2.dot(q) * inv > 1e-9 {
            crossings += 1;
        }
    }
    crossings % 2 == 1
}

/// The volume a closed mesh encloses, as the signed tetrahedra its faces make
/// with the origin.
pub(crate) fn volume(mesh: &Mesh) -> f64 {
    let sum: f64 = mesh
        .indices
        .iter()
        .map(|t| {
            let (a, b, c) =
                (mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]);
            a.dot(b.cross(c))
        })
        .sum();
    (sum / 6.0).abs()
}
