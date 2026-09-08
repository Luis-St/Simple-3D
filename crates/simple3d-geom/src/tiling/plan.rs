//! Turning a tiling and a split plan into the cells that will be cut.

use super::*;
use crate::mesh::Mesh;
use crate::revolve::extrude_frustum_polygon;
use crate::vec3::Vec3;

/// Cut a solid by every pass of a plan, in order.
///
/// Each pass cuts what the last one left, so the pieces are the intersection of
/// all of the tilings -- a plate cut into squares through Z and then into slabs
/// through X comes back as the blocks the two grids make between them.
///
/// `report` is called once per cell tried, at every pass, which is what
/// [`SplitPlan::work`] counts.
pub fn cut_plan(
    mesh: &Mesh,
    plan: &SplitPlan,
    report: &(dyn Fn() + Sync),
    give_up: &(dyn Fn() -> bool + Sync),
) -> Option<Vec<Mesh>> {
    let Some((first, rest)) = plan.passes.split_first() else { return Some(Vec::new()) };
    let Some(frame) = mesh.bounds() else { return Some(Vec::new()) };
    let mut pieces = cut_within(mesh, first, frame, report, give_up)?;
    for tiling in rest {
        let mut next = Vec::with_capacity(pieces.len());
        for piece in &pieces {
            // Every pass is laid out over the whole shape, so the cuts line up
            // across the pieces the last one made. A piece the pass leaves
            // whole comes back as itself, so nothing is lost to a cut that
            // misses -- and the order is the order the cells were planned in,
            // at every level, so the same plan on the same shape always names
            // the same piece.
            next.extend(cut_within(piece, tiling, frame, report, give_up)?);
        }
        pieces = next;
    }
    Some(pieces)
}

/// One cell of the tiling: the prism a piece is cut out by.
pub(crate) struct Cell {
    /// The outline in world terms already, one point per corner.
    pub(super) outline: Vec<(f64, f64)>,
    pub(super) centre: (f64, f64),
    /// Where the prism starts and ends along the axis.
    pub(super) span: (f64, f64),
    pub(super) bounds: (Vec3, Vec3),
}

impl Cell {
    /// The prism itself. Built when the cell is cut rather than when it is
    /// planned: most cells of a large split are thrown away without ever being
    /// intersected with anything, and a mesh each for those is pure allocation.
    pub(super) fn prism(&self, tiling: &Tiling) -> Mesh {
        let (lo, hi) = self.span;
        let centred: Vec<(f64, f64)> =
            self.outline.iter().map(|&(x, y)| (x - self.centre.0, y - self.centre.1)).collect();
        let local = extrude_frustum_polygon(&centred, &centred, hi - lo);
        let (cu, cv, cw) = (self.centre.0, self.centre.1, (lo + hi) / 2.0);
        let positions = local.positions.iter().map(|p| tiling.to_world(p.x + cu, p.y + cv, p.z + cw)).collect();
        Mesh { positions, indices: local.indices, tags: local.tags }
    }
}

/// How far past the shape a cell reaches at the ends of its run.
///
/// The cut face of a piece must be the *shape's* own surface there, not the end
/// cap of the cell -- two faces in the same plane are the one case a boolean
/// kernel has to work hardest at, and there is no reason to create it where the
/// cell was never meant to end. So the outermost cells overshoot.
pub(crate) const OVERSHOOT: f64 = 1.0;

/// Lay the tiling over a shape of these bounds, giving every cell that could
/// hold a piece of it.
pub(crate) fn plan(tiling: &Tiling, bounds: (Vec3, Vec3)) -> Vec<Cell> {
    let (lo, hi) = bounds;
    let (ua, va) = tiling.plane_axes();
    let axis = tiling.axis.min(2) as usize;
    let (lo_a, hi_a) = ([lo.x, lo.y, lo.z], [hi.x, hi.y, hi.z]);
    // The grid is anchored on the middle of the shape, so an untouched split is
    // symmetric about it, and the offset moves it from there.
    let origin = ((lo_a[ua] + hi_a[ua]) / 2.0 + tiling.offset[0], (lo_a[va] + hi_a[va]) / 2.0 + tiling.offset[1]);
    let angle = tiling.angle.to_radians();
    let (sin, cos) = (angle.sin(), angle.cos());
    // Into the grid's own frame: the corners of the shape's box, turned back by
    // the grid's angle, say which rows and columns can reach it.
    let corners = [(lo_a[ua], lo_a[va]), (hi_a[ua], lo_a[va]), (lo_a[ua], hi_a[va]), (hi_a[ua], hi_a[va])];
    let (mut p0, mut p1, mut q0, mut q1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for (x, y) in corners {
        let (dx, dy) = (x - origin.0, y - origin.1);
        let (p, q) = (dx * cos + dy * sin, -dx * sin + dy * cos);
        (p0, p1, q0, q1) = (p0.min(p), p1.max(p), q0.min(q), q1.max(q));
    }
    let (sx, sy) = tiling.steps();
    // The lattice is centred on the shape, which is not the same as putting a
    // cell centre there: a 30 mm plate cut into 15 mm squares is four squares,
    // and it is four only if the cut falls down the middle. So an even number
    // of cells across is stood half a cell over, and an odd number is not.
    let (across, along, _) = tiling.spans(bounds);
    let phase = (centring(across, sx), centring(along, sy));
    let (columns, rows) = (range(p0 - phase.0, p1 - phase.0, sx), range(q0 - phase.1, q1 - phase.1, sy));
    let layers = layers(tiling, lo_a[axis], hi_a[axis]);

    let mut cells = Vec::new();
    for j in rows.clone() {
        for i in columns.clone() {
            for (centre, outline) in cell_shapes(tiling, i, j) {
                // Out of the grid's frame and into the world's: stand the
                // lattice where the centring put it, turn by the angle, then
                // stand at the origin the grid was anchored on.
                let place = |(x, y): (f64, f64)| {
                    let (x, y) = (x + phase.0, y + phase.1);
                    (origin.0 + x * cos - y * sin, origin.1 + x * sin + y * cos)
                };
                let centre = place(centre);
                let outline: Vec<(f64, f64)> = outline.into_iter().map(place).collect();
                for &(from, to) in &layers {
                    let cell = bound(tiling, centre, outline.clone(), (from, to));
                    // The margin the ranges above carry is there so nothing at
                    // the edge is missed, not so that cells beyond the shape are
                    // planned: one that cannot touch it makes no piece, and a
                    // plan that counts it says a number nobody can find in the
                    // outliner afterwards.
                    if reaches(cell.bounds, bounds) {
                        cells.push(cell);
                    }
                }
            }
        }
    }
    cells
}
