//! Cutting a body into its cells, and measuring what came out.

use super::*;
use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// Cut a solid into one piece per cell, in a stable order. `report` is called per finished cell;
/// giving up returns `None` and drops partial results. Every piece has volume.
pub fn cut(
    mesh: &Mesh,
    tiling: &Tiling,
    report: &(dyn Fn() + Sync),
    give_up: &(dyn Fn() -> bool + Sync),
) -> Option<Vec<Mesh>> {
    let Some(bounds) = mesh.bounds() else { return Some(Vec::new()) };
    cut_within(mesh, tiling, bounds, report, give_up)
}

/// Cut a solid by a tiling laid over `frame` rather than the solid, so later passes of a plan cut
/// every piece with the same grid instead of one centred on each piece.
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
    // Planned over the whole frame, kept where it reaches this solid.
    let cells: Vec<Cell> = plan(tiling, frame).into_iter().filter(|cell| reaches(cell.bounds, bounds)).collect();
    // Each triangle's box, once: a cell overlapping none holds no surface, keeping large interiors cheap.
    let faces: Vec<(Vec3, Vec3)> = mesh
        .indices
        .iter()
        .map(|t| {
            let [a, b, c] = mesh.corners(*t);
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
                    // Every nth cell rather than a block, since costly surface cells come in runs.
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
    // Back into planned order, so the same split always names the same piece.
    let mut pieces: Vec<(usize, Mesh)> = batches.into_iter().flatten().collect();
    pieces.sort_by_key(|(index, _)| *index);
    Some(pieces.into_iter().map(|(_, mesh)| mesh).collect())
}

/// The piece one cell holds, if any.
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
    // A cell no face nears is wholly in or out; one ray decides, and inside the piece is the cell with
    // no boolean at all.
    if !faces.iter().any(|face| crate::boxes_overlap(*face, cell.bounds)) {
        let centre = tiling.to_world(cell.centre.0, cell.centre.1, (cell.span.0 + cell.span.1) / 2.0);
        return inside(mesh, centre).then(|| cell.prism(tiling));
    }
    let piece = crate::csg_bsp::intersect_until(mesh, &cell.prism(tiling), &|| give_up());
    // A grazing cell yields a volumeless sliver or nothing; neither is a piece.
    (volume(&piece) > 1e-9).then_some(piece)
}

/// Whether a point is inside a closed mesh, by ray-crossing parity. The fixed direction is skew so
/// it never runs along axis-aligned faces or edges.
pub(crate) fn inside(mesh: &Mesh, point: Vec3) -> bool {
    let dir = Vec3::new(0.5773502691896258, 0.3313, 0.7443).normalized();
    let mut crossings = 0;
    for tri in &mesh.indices {
        let [a, b, c] = mesh.corners(*tri);
        if crate::ray::line_triangle(point, dir, [a, b, c], 0.0).is_some_and(|t| t > 1e-9) {
            crossings += 1;
        }
    }
    crossings % 2 == 1
}

/// The volume a closed mesh encloses, from signed tetrahedra to the origin.
pub(crate) fn volume(mesh: &Mesh) -> f64 {
    mesh.signed_volume().abs()
}
