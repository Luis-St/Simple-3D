//! Cutting one solid into a tiling of smaller solids (issue 82).
//!
//! The shape is in one piece, and the pieces are made by cutting it -- into
//! squares, rectangles, triangles or hexagons, the way a plate is scored before
//! it is broken.
//!
//! The cells are prisms: a 2D tiling of the plane, extruded along one axis
//! through the whole solid, and optionally cut into layers along that axis as
//! well. Each piece is the intersection of the solid with one cell, so the
//! pieces are disjoint, they add back up to the solid, and each one keeps the
//! part of the original surface it had. Nothing here knows about scenes or
//! nodes: it takes a mesh and gives back meshes.
//!
//! Two things keep it affordable. A cell no triangle of the solid comes near is
//! either wholly inside it or wholly outside it, which one ray answers -- so the
//! inside of a big shape costs no boolean at all and only the cells along the
//! surface go through the kernel. And the cells are independent, so they are cut
//! on as many threads as the machine has.

use crate::mesh::Mesh;
use crate::revolve::extrude_frustum_polygon;
use crate::vec3::Vec3;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// The shape one cell of the tiling is.
///
/// Every one of them tiles the plane exactly -- no gaps and no overlaps -- which
/// is what makes the pieces add back up to the shape they were cut from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellKind {
    #[default]
    Squares,
    Rectangles,
    Triangles,
    Hexagons,
}

impl CellKind {
    pub const ALL: [CellKind; 4] = [CellKind::Squares, CellKind::Rectangles, CellKind::Triangles, CellKind::Hexagons];

    pub fn label(self) -> &'static str {
        match self {
            CellKind::Squares => "Squares",
            CellKind::Rectangles => "Rectangles",
            CellKind::Triangles => "Triangles",
            CellKind::Hexagons => "Hexagons",
        }
    }

    /// The singular, for naming one piece.
    pub fn singular(self) -> &'static str {
        match self {
            CellKind::Squares => "square",
            CellKind::Rectangles => "rectangle",
            CellKind::Triangles => "triangle",
            CellKind::Hexagons => "hexagon",
        }
    }

    /// Whether the second side length means anything for this kind. Only a
    /// rectangle has two; the others are defined by one number, and offering a
    /// field that changes nothing is worse than not offering it.
    pub fn has_depth(self) -> bool {
        self == CellKind::Rectangles
    }

    /// What the one size number means for this kind, for a tooltip.
    pub fn size_meaning(self) -> &'static str {
        match self {
            CellKind::Squares => "The side of one square.",
            CellKind::Rectangles => "The first side of one rectangle.",
            CellKind::Triangles => {
                "The side of one triangle. They are equilateral, and alternate point-up and \
                                    point-down along a row."
            }
            CellKind::Hexagons => "The width of one hexagon across its flats.",
        }
    }
}

/// How a solid is to be cut into pieces (issue 82).
///
/// Sizes are millimetres and the angle is degrees, as everywhere else in the
/// document. `axis` is the direction the cells are extruded along -- the tiling
/// itself lies in the plane across it -- so a plate lying flat is cut into
/// columns by the default, `Z`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tiling {
    pub kind: CellKind,
    /// The cell's size across, in millimetres. What it measures depends on the
    /// kind -- see [`CellKind::size_meaning`].
    pub size: f64,
    /// The second side of a rectangle, in millimetres. Ignored by every other
    /// kind, but kept through a change of kind so switching back and forth does
    /// not lose the number.
    pub depth: f64,
    /// The axis the cells run along: 0 = X, 1 = Y, 2 = Z.
    pub axis: u8,
    /// Degrees the whole grid is turned by within its plane.
    pub angle: f64,
    /// Where the grid sits, relative to the centre of what is being cut, in the
    /// two axes of its plane. Zero puts a cell's centre on the shape's centre,
    /// which is the symmetric answer; shifting it is how a cut is moved off a
    /// feature it would otherwise land on.
    pub offset: [f64; 2],
    /// Also cut into layers of this height along the axis. Zero -- the default
    /// -- cuts straight through, which is what "split into hexagons" means on
    /// its own.
    pub layer: f64,
}

impl Default for Tiling {
    fn default() -> Tiling {
        Tiling { kind: CellKind::Squares, size: 10.0, depth: 10.0, axis: 2, angle: 0.0, offset: [0.0, 0.0], layer: 0.0 }
    }
}

/// The smallest cell worth asking for. Below this the number of cells runs away
/// faster than the arithmetic that counts them is worth doing.
pub const MIN_SIZE: f64 = 0.01;

/// The most cells a single split may ask for.
///
/// Not a performance guess but a usability one: ten thousand pieces is ten
/// thousand rows in the outliner and ten thousand bodies in an export, and a
/// number typed with one zero too many should be refused while it can still be
/// corrected rather than after a minute of cutting.
pub const MAX_CELLS: usize = 10_000;

impl Tiling {
    /// The axes of the plane the cells tile, as a right-handed pair with the
    /// extrusion axis: Z gives (X, Y), X gives (Y, Z), Y gives (Z, X). Keeping
    /// the cycle is what lets a cell built in the plane be carried into the
    /// world by a permutation of its coordinates, which cannot turn a solid
    /// inside out the way a reflection would.
    fn plane_axes(&self) -> (usize, usize) {
        match self.axis {
            0 => (1, 2),
            1 => (2, 0),
            _ => (0, 1),
        }
    }

    /// A world point in the two directions of the plane the cells tile, which
    /// is what a picture of the tiling is drawn in.
    pub fn flatten(&self, p: Vec3) -> (f64, f64) {
        let (ua, va) = self.plane_axes();
        let p = [p.x, p.y, p.z];
        (p[ua], p[va])
    }

    /// A point of the cell frame -- across, along, through -- in world terms.
    ///
    /// The inverse of [`Tiling::flatten`] once a distance along the axis is
    /// named: `flatten` throws that distance away, because a plan of the cells
    /// does not need it, and lifting an outline back into the shape does.
    pub fn to_world(self, u: f64, v: f64, w: f64) -> Vec3 {
        let (ua, va) = self.plane_axes();
        let mut out = [0.0; 3];
        out[ua] = u;
        out[va] = v;
        out[self.axis.min(2) as usize] = w;
        Vec3::new(out[0], out[1], out[2])
    }

    /// The two side lengths of the grid's repeat, for the kinds laid out on a
    /// rectangular lattice.
    fn steps(&self) -> (f64, f64) {
        let size = self.size.max(MIN_SIZE);
        match self.kind {
            CellKind::Squares => (size, size),
            CellKind::Rectangles => (size, self.depth.max(MIN_SIZE)),
            // A row of triangles repeats every side length across and every
            // triangle height along.
            CellKind::Triangles => (size, size * f64::sqrt(3.0) / 2.0),
            // Pointy-top hexagons: one width across, three quarters of the
            // point-to-point height along.
            CellKind::Hexagons => (size, size * f64::sqrt(3.0) / 2.0),
        }
    }

    /// How many cells a lattice row holds per step. Triangles are two per step
    /// -- one pointing each way -- and everything else is one.
    fn per_step(&self) -> usize {
        if self.kind == CellKind::Triangles {
            2
        } else {
            1
        }
    }

    /// How many cells would be laid over a shape of these bounds, before any of
    /// them is found to be empty.
    ///
    /// Counted arithmetically rather than by building them, because the whole
    /// point is to be able to refuse a number too large to build.
    pub fn cell_count(&self, bounds: (Vec3, Vec3)) -> usize {
        let (across, along, through) = self.spans(bounds);
        let (sx, sy) = self.steps();
        let columns = (across / sx).ceil() as usize + 2;
        let rows = (along / sy).ceil() as usize + 2;
        columns.saturating_mul(rows).saturating_mul(self.per_step()).saturating_mul(self.layer_count(through))
    }

    /// How far the shape reaches across the grid's own two directions, and
    /// along the axis.
    ///
    /// The grid may be turned within its plane, so what has to be covered is
    /// the box of the *turned* corners, not the box itself.
    fn spans(&self, bounds: (Vec3, Vec3)) -> (f64, f64, f64) {
        let (lo, hi) = bounds;
        let (ua, va) = self.plane_axes();
        let (lo, hi) = ([lo.x, lo.y, lo.z], [hi.x, hi.y, hi.z]);
        let (w, d) = (hi[ua] - lo[ua], hi[va] - lo[va]);
        let (sin, cos) = (self.angle.to_radians().sin().abs(), self.angle.to_radians().cos().abs());
        (w * cos + d * sin, w * sin + d * cos, hi[self.axis.min(2) as usize] - lo[self.axis.min(2) as usize])
    }

    /// How many layers the cells are cut into along the axis.
    fn layer_count(&self, through: f64) -> usize {
        if self.layer < MIN_SIZE {
            1
        } else {
            ((through / self.layer).ceil() as usize).max(1)
        }
    }

    /// Whether this is a tiling that can be built at all: a size that is a size,
    /// and a cell count something can be done with.
    pub fn refusal(&self, bounds: (Vec3, Vec3)) -> Option<String> {
        if self.size < MIN_SIZE || (self.kind.has_depth() && self.depth < MIN_SIZE) {
            return Some("A cell needs a size bigger than zero.".to_string());
        }
        let cells = self.cell_count(bounds);
        if cells > MAX_CELLS {
            return Some(format!(
                "That is {cells} cells, and {MAX_CELLS} is as many as one split may make. \
                 Use a bigger cell, or split a part of the shape."
            ));
        }
        None
    }
}

/// Several tilings, cut one after another, so a shape can be cut on more than
/// one axis at once and with a different cell shape on each (issue 82).
///
/// One tiling answers "squares through Z". Two answer "squares through Z, then
/// slabs through X", which is the pattern a plate is scored into a grid of
/// blocks by, and what neither a single tiling nor two separate splits can say:
/// splitting a split cuts the *pieces* of one into a second collection, and the
/// way back is then two joins deep.
///
/// The passes are applied in order, each to the pieces the last one left, so
/// the pieces are the cells of every tiling intersected. That is also why the
/// order does not change the answer -- intersection does not care -- and why
/// nothing here has to reason about how two lattices meet.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SplitPlan {
    pub passes: Vec<Tiling>,
}

impl Default for SplitPlan {
    fn default() -> SplitPlan {
        SplitPlan::of(Tiling::default())
    }
}

/// Read either a list of tilings or a single one.
///
/// A split used to be cut by exactly one tiling and wrote it as an object. A
/// file or a settings file written then still says what was done, and there is
/// no reason to lose it over a pair of brackets.
impl<'de> Deserialize<'de> for SplitPlan {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<SplitPlan, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Read {
            Many(Vec<Tiling>),
            One(Box<Tiling>),
        }
        Ok(match Read::deserialize(deserializer)? {
            Read::Many(passes) => SplitPlan { passes },
            Read::One(tiling) => SplitPlan::of(*tiling),
        })
    }
}

/// The most passes one split may be cut by.
///
/// Three is not a technical limit -- the pieces of any pass can be cut again --
/// but each pass multiplies the pieces the last one made, so a fourth is a
/// number nobody typed and a wait nobody asked for. Two is what "on more than
/// one axis" means; the third is there for the plate that is also cut into
/// layers a different way.
pub const MAX_PASSES: usize = 3;

impl SplitPlan {
    pub fn of(tiling: Tiling) -> SplitPlan {
        SplitPlan { passes: vec![tiling] }
    }

    /// The first pass, which is the whole plan for the ordinary one-pass split
    /// and the one a summary is named after.
    pub fn first(&self) -> Tiling {
        self.passes.first().copied().unwrap_or_default()
    }

    /// Whether this is a plan that can be cut at all: every pass buildable on
    /// its own, and the pieces they come to between them a number a person can
    /// still find in the outliner.
    pub fn refusal(&self, bounds: (Vec3, Vec3)) -> Option<String> {
        if self.passes.is_empty() {
            return Some("There is nothing to cut with: add a cut.".to_string());
        }
        for tiling in &self.passes {
            // A single pass is refused by its own count first, so the message
            // names the cell that is too small rather than the total.
            if let Some(why) = tiling.refusal(bounds) {
                return Some(why);
            }
        }
        let cells = self.cell_count(bounds);
        if cells > MAX_CELLS {
            return Some(format!(
                "The cuts come to {cells} cells between them, and {MAX_CELLS} is as many as one split may make. \
                 Use a bigger cell, or one cut fewer."
            ));
        }
        None
    }

    /// How many cells the passes lay over a shape of these bounds between them,
    /// as arithmetic -- the product, because every cell of one pass is cut by
    /// every cell of the next.
    pub fn cell_count(&self, bounds: (Vec3, Vec3)) -> usize {
        self.passes.iter().fold(1usize, |total, tiling| total.saturating_mul(tiling.cell_count(bounds)))
    }

    /// How many cells the passes actually lay over the shape -- the ones that
    /// could hold a piece of it, which is the number worth showing.
    pub fn planned(&self, bounds: (Vec3, Vec3)) -> usize {
        if self.refusal(bounds).is_some() {
            return 0;
        }
        self.passes.iter().fold(1usize, |total, tiling| total.saturating_mul(planned(tiling, bounds)))
    }

    /// How many cells will be *tried*, which is what progress is measured
    /// against: the first pass over the shape, then the second over each piece
    /// the first left, and so on. Every pass but the first is counted against
    /// the pieces before it, which is why this is a running product rather than
    /// the last one.
    pub fn work(&self, bounds: (Vec3, Vec3)) -> usize {
        let mut total = 0usize;
        let mut running = 1usize;
        for tiling in &self.passes {
            running = running.saturating_mul(planned(tiling, bounds));
            total = total.saturating_add(running);
        }
        total
    }

    /// Where every pass's cuts will fall, for drawing them over the model. The
    /// cap is shared out between the passes, so a second cut cannot be squeezed
    /// off the picture by the first one filling it.
    pub fn preview_loops(&self, bounds: (Vec3, Vec3), limit: usize) -> Vec<Vec<Vec3>> {
        let each = limit / self.passes.len().max(1);
        self.passes.iter().flat_map(|tiling| preview_loops(tiling, bounds, each)).collect()
    }
}

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
struct Cell {
    /// The outline in world terms already, one point per corner.
    outline: Vec<(f64, f64)>,
    centre: (f64, f64),
    /// Where the prism starts and ends along the axis.
    span: (f64, f64),
    bounds: (Vec3, Vec3),
}

impl Cell {
    /// The prism itself. Built when the cell is cut rather than when it is
    /// planned: most cells of a large split are thrown away without ever being
    /// intersected with anything, and a mesh each for those is pure allocation.
    fn prism(&self, tiling: &Tiling) -> Mesh {
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
const OVERSHOOT: f64 = 1.0;

/// Lay the tiling over a shape of these bounds, giving every cell that could
/// hold a piece of it.
fn plan(tiling: &Tiling, bounds: (Vec3, Vec3)) -> Vec<Cell> {
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

/// How far the lattice is stood over so that the cells across a span are
/// centred on it: half a cell where an even number of them covers it, nothing
/// where an odd number does and the middle cell already sits in the middle.
fn centring(span: f64, step: f64) -> f64 {
    let count = (span / step).ceil().max(1.0) as i64;
    if count % 2 == 0 {
        step / 2.0
    } else {
        0.0
    }
}

/// The indices of the lattice rows or columns that can reach a span, with a
/// cell of margin on each side so nothing at the edge is missed.
fn range(from: f64, to: f64, step: f64) -> std::ops::RangeInclusive<i64> {
    let first = (from / step).floor() as i64 - 1;
    let last = (to / step).ceil() as i64 + 1;
    first..=last
}

/// Where each layer of cells starts and ends along the axis. The first and the
/// last overshoot the shape, so the pieces there are capped by the shape's own
/// surface rather than by the cell.
fn layers(tiling: &Tiling, lo: f64, hi: f64) -> Vec<(f64, f64)> {
    if tiling.layer < MIN_SIZE {
        return vec![(lo - OVERSHOOT, hi + OVERSHOOT)];
    }
    let count = tiling.layer_count(hi - lo);
    (0..count)
        .map(|k| {
            let from = if k == 0 { lo - OVERSHOOT } else { lo + k as f64 * tiling.layer };
            let to = if k == count - 1 { hi + OVERSHOOT } else { lo + (k + 1) as f64 * tiling.layer };
            (from, to)
        })
        .collect()
}

/// One cell as it is laid out: where its centre is, and its outline
/// counter-clockwise about it.
type CellShape = ((f64, f64), Vec<(f64, f64)>);

/// The cell or cells at one place in the lattice, in the grid's own frame.
fn cell_shapes(tiling: &Tiling, i: i64, j: i64) -> Vec<CellShape> {
    let (sx, sy) = tiling.steps();
    let (i, j) = (i as f64, j as f64);
    match tiling.kind {
        CellKind::Squares | CellKind::Rectangles => {
            let centre = (i * sx, j * sy);
            let (hw, hd) = (sx / 2.0, sy / 2.0);
            let outline = vec![
                (centre.0 - hw, centre.1 - hd),
                (centre.0 + hw, centre.1 - hd),
                (centre.0 + hw, centre.1 + hd),
                (centre.0 - hw, centre.1 + hd),
            ];
            vec![(centre, outline)]
        }
        // Two per step: one pointing up whose base is the bottom of the row,
        // and one pointing down between it and the next, whose base is the top.
        // Together they fill the row exactly.
        CellKind::Triangles => {
            let radius = sx / f64::sqrt(3.0);
            let up = (i * sx + sx / 2.0, j * sy + sy / 3.0);
            let down = (i * sx + sx, j * sy + 2.0 * sy / 3.0);
            vec![(up, triangle(up, radius, false)), (down, triangle(down, radius, true))]
        }
        // Rows interlock, so every other one is shifted half a cell across.
        CellKind::Hexagons => {
            let shift = if j.rem_euclid(2.0) == 0.0 { 0.0 } else { sx / 2.0 };
            let centre = (i * sx + shift, j * sy);
            vec![(centre, hexagon(centre, sx / f64::sqrt(3.0)))]
        }
    }
}

/// An equilateral triangle about its own centre, point up or point down.
fn triangle(centre: (f64, f64), radius: f64, down: bool) -> Vec<(f64, f64)> {
    let turn = if down { PI } else { 0.0 };
    (0..3)
        .map(|k| {
            let angle = turn + PI / 2.0 + k as f64 * 2.0 * PI / 3.0;
            (centre.0 + radius * angle.cos(), centre.1 + radius * angle.sin())
        })
        .collect()
}

/// A pointy-top hexagon about its own centre. `radius` is corner to centre,
/// which is the width across the flats over the square root of three.
fn hexagon(centre: (f64, f64), radius: f64) -> Vec<(f64, f64)> {
    (0..6)
        .map(|k| {
            let angle = PI / 2.0 + k as f64 * PI / 3.0;
            (centre.0 + radius * angle.cos(), centre.1 + radius * angle.sin())
        })
        .collect()
}

/// Whether a cell can hold any of a shape, which is a stricter question than
/// whether their boxes overlap.
///
/// [`crate::boxes_overlap`] answers yes for boxes that merely *touch*, because
/// two coincident faces are exactly what the boolean kernel must be given a
/// chance to handle. Here the opposite is wanted: a cell sitting exactly against
/// the side of the shape holds a slice of it nothing thick, which is not a
/// piece, and counting it says a number nobody can find in the outliner.
fn reaches(cell: (Vec3, Vec3), shape: (Vec3, Vec3)) -> bool {
    const EPS: f64 = 1e-6;
    let ((clo, chi), (slo, shi)) = (cell, shape);
    clo.x + EPS < shi.x
        && slo.x + EPS < chi.x
        && clo.y + EPS < shi.y
        && slo.y + EPS < chi.y
        && clo.z + EPS < shi.z
        && slo.z + EPS < chi.z
}

fn bound(tiling: &Tiling, centre: (f64, f64), outline: Vec<(f64, f64)>, span: (f64, f64)) -> Cell {
    let mut lo = tiling.to_world(outline[0].0, outline[0].1, span.0);
    let mut hi = lo;
    for &(x, y) in &outline {
        for w in [span.0, span.1] {
            let p = tiling.to_world(x, y, w);
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    Cell { outline, centre, span, bounds: (lo, hi) }
}

/// How many cells the tiling actually lays over a shape of these bounds -- the
/// ones that could hold a piece of it, which is what a count worth showing
/// means and what the progress of a split is measured against.
///
/// [`Tiling::cell_count`] is the arithmetic upper bound and is what refuses a
/// number too large to build; this one plans the cells to find out, so it is
/// only asked once the count is known to be sane.
pub fn planned(tiling: &Tiling, bounds: (Vec3, Vec3)) -> usize {
    if tiling.refusal(bounds).is_some() {
        return 0;
    }
    plan(tiling, bounds).len()
}

/// The outline of every cell the tiling lays over a shape of these bounds, in
/// the plane the cells tile -- for drawing where the cuts will fall, and for
/// nothing else.
///
/// The layers do not come into it: they cut across the plane rather than within
/// it, so every layer's cells have the one outline.
pub fn cell_outlines(tiling: &Tiling, bounds: (Vec3, Vec3)) -> Vec<Vec<(f64, f64)>> {
    let flat = Tiling { layer: 0.0, ..*tiling };
    if flat.refusal(bounds).is_some() {
        return Vec::new();
    }
    plan(&flat, bounds).into_iter().map(|cell| cell.outline).collect()
}

/// Where the cuts will fall, as loops in the frame the shape's own bounds are
/// given in -- for drawing the tiling over the model itself rather than as a
/// plan beside it (issue 82).
///
/// A cut is a surface, not a line, and drawing every one of them would be a
/// cage nobody can see the shape through. What is drawn instead is the tiling
/// where it meets the shape: at each end of the run along the axis, and at
/// every layer boundary in between. Seen down the axis those coincide and read
/// as one grid, which is the plan; seen from anywhere else they separate, and
/// the separation is what says how deep the cuts go.
///
/// `limit` caps how many loops come back, because ten thousand cells at three
/// depths is thirty thousand outlines and a preview is not worth a frame rate.
/// The ends are laid down before the layers between them, so what survives the
/// cap is the part of the picture that says the most.
pub fn preview_loops(tiling: &Tiling, bounds: (Vec3, Vec3), limit: usize) -> Vec<Vec<Vec3>> {
    let outlines = cell_outlines(tiling, bounds);
    if outlines.is_empty() || limit == 0 {
        return Vec::new();
    }
    let axis = tiling.axis.min(2) as usize;
    let (lo, hi) = ([bounds.0.x, bounds.0.y, bounds.0.z], [bounds.1.x, bounds.1.y, bounds.1.z]);
    let (lo, hi) = (lo[axis], hi[axis]);
    // The far end first, then the near one, then the layers between them: the
    // order the cap eats from the back of.
    let mut depths = vec![hi, lo];
    if tiling.layer >= MIN_SIZE {
        for k in 1..tiling.layer_count(hi - lo) {
            let at = lo + k as f64 * tiling.layer;
            if at > lo && at < hi {
                depths.push(at);
            }
        }
    }
    let mut loops = Vec::new();
    for depth in depths {
        if loops.len() >= limit {
            break;
        }
        for outline in &outlines {
            if loops.len() >= limit {
                break;
            }
            loops.push(outline.iter().map(|&(u, v)| tiling.to_world(u, v, depth)).collect());
        }
    }
    loops
}

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
fn cut_within(
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
fn cut_cell(
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
fn inside(mesh: &Mesh, point: Vec3) -> bool {
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
fn volume(mesh: &Mesh) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::box_mesh;

    fn never() -> bool {
        false
    }

    fn cut_box(tiling: &Tiling) -> Vec<Mesh> {
        cut(&box_mesh(30.0, 30.0, 10.0), tiling, &|| {}, &never).expect("nothing abandoned it")
    }

    /// The whole point of the feature: the pieces are the shape, cut up. Their
    /// volumes must add back up to what they were cut from, whatever shape the
    /// cells are.
    #[test]
    fn every_kind_of_cell_adds_back_up_to_the_shape() {
        for kind in CellKind::ALL {
            let tiling = Tiling { kind, size: 9.0, depth: 7.0, ..Tiling::default() };
            let pieces = cut_box(&tiling);
            let total: f64 = pieces.iter().map(volume).sum();
            assert!(pieces.len() > 1, "{kind:?} made {} piece(s)", pieces.len());
            assert!((total - 9000.0).abs() < 1.0, "{kind:?} pieces hold {total} mm3 of the 9000 they were cut from");
        }
    }

    /// A cell in the middle of a solid never goes through the kernel, so its
    /// piece is the cell itself. That is a shortcut, and a shortcut is only
    /// worth having if it gives the same answer: a 30 mm cube in 10 mm squares
    /// is 9 columns of exactly 100 mm2 each.
    #[test]
    fn cells_inside_the_solid_come_out_whole() {
        let pieces = cut_box(&Tiling { size: 10.0, ..Tiling::default() });
        assert_eq!(pieces.len(), 9);
        for piece in &pieces {
            assert!((volume(piece) - 1000.0).abs() < 1e-6, "a column came out at {} mm3", volume(piece));
        }
    }

    #[test]
    fn layers_cut_along_the_axis_as_well() {
        let plain = cut_box(&Tiling { size: 10.0, ..Tiling::default() });
        let layered = cut_box(&Tiling { size: 10.0, layer: 5.0, ..Tiling::default() });
        assert_eq!(plain.len(), 9);
        assert_eq!(layered.len(), 18);
        let total: f64 = layered.iter().map(volume).sum();
        assert!((total - 9000.0).abs() < 1.0);
    }

    /// The axis is the direction the cells run in, so cutting a 30 x 30 x 10
    /// plate along X gives cells across its 30 x 10 side.
    #[test]
    fn the_axis_chooses_which_way_the_cells_run() {
        let along_z = cut_box(&Tiling { size: 10.0, axis: 2, ..Tiling::default() });
        let along_x = cut_box(&Tiling { size: 10.0, axis: 0, ..Tiling::default() });
        assert_eq!(along_z.len(), 9);
        assert_eq!(along_x.len(), 3);
        for piece in &along_x {
            assert!((volume(piece) - 3000.0).abs() < 1e-6, "a slab came out at {} mm3", volume(piece));
        }
    }

    #[test]
    fn a_cell_bigger_than_the_shape_leaves_it_in_one_piece() {
        let pieces = cut_box(&Tiling { size: 100.0, ..Tiling::default() });
        assert_eq!(pieces.len(), 1);
        assert!((volume(&pieces[0]) - 9000.0).abs() < 1e-6);
    }

    #[test]
    fn a_count_too_large_is_refused_before_anything_is_built() {
        let bounds = (Vec3::new(-100.0, -100.0, -1.0), Vec3::new(100.0, 100.0, 1.0));
        let tiling = Tiling { size: 0.5, ..Tiling::default() };
        assert!(tiling.cell_count(bounds) > MAX_CELLS);
        assert!(tiling.refusal(bounds).is_some());
        assert!(Tiling { size: 20.0, ..Tiling::default() }.refusal(bounds).is_none());
    }

    /// Every piece is exported and printed on its own, so every piece has to be
    /// a solid in its own right -- including the awkward ones, cut out of a
    /// shape that already had a hole through it.
    #[test]
    fn every_piece_is_a_solid_of_its_own() {
        let plate = crate::csg_bsp::subtract(
            &box_mesh(40.0, 40.0, 6.0),
            &crate::primitives::cylinder_mesh(14.0, 14.0, 20.0, 48),
        );
        for kind in CellKind::ALL {
            let tiling = Tiling { kind, size: 12.0, depth: 9.0, ..Tiling::default() };
            let pieces = cut(&plate, &tiling, &|| {}, &never).expect("nothing abandoned it");
            assert!(pieces.len() > 3, "{kind:?} made {} piece(s)", pieces.len());
            for (index, piece) in pieces.iter().enumerate() {
                assert!(piece.manifold_issue().is_none(), "{kind:?} piece {index}: {:?}", piece.manifold_issue());
            }
            let total: f64 = pieces.iter().map(volume).sum();
            let expected = 40.0 * 40.0 * 6.0 - PI * 7.0 * 7.0 * 6.0;
            assert!((total - expected).abs() < expected * 0.01, "{kind:?} holds {total} mm3 of {expected}");
        }
    }

    /// The plan carries a cell of margin around the shape so nothing at the edge
    /// is missed, and those cells are not pieces: a 60 x 40 plate in 10 mm
    /// squares is 24 cells, not the 48 the margin makes.
    #[test]
    fn the_plan_counts_the_cells_that_can_hold_a_piece_and_no_others() {
        let bounds = (Vec3::new(-30.0, -20.0, -4.0), Vec3::new(30.0, 20.0, 4.0));
        let tiling = Tiling { size: 10.0, ..Tiling::default() };
        assert_eq!(planned(&tiling, bounds), 24);
        assert_eq!(cell_outlines(&tiling, bounds).len(), 24);
        // And the cells it does plan are the ones the shape is cut into.
        let pieces = cut(&box_mesh(60.0, 40.0, 8.0), &tiling, &|| {}, &never).expect("nothing abandoned it");
        assert_eq!(pieces.len(), 24);
    }

    #[test]
    fn giving_up_hands_back_nothing() {
        let tiling = Tiling { size: 2.0, ..Tiling::default() };
        assert!(cut(&box_mesh(30.0, 30.0, 10.0), &tiling, &|| {}, &|| true).is_none());
    }

    /// Turning the grid must not lose or gain material, only move where the
    /// cuts fall.
    #[test]
    fn a_turned_grid_still_covers_the_shape() {
        let pieces = cut_box(&Tiling { size: 9.0, angle: 30.0, ..Tiling::default() });
        let total: f64 = pieces.iter().map(volume).sum();
        assert!((total - 9000.0).abs() < 1.0, "a turned grid holds {total} mm3 of 9000");
    }

    /// A grid is centred on what it cuts, so an even division comes out even:
    /// a 30 mm square in 15 mm cells is four of them and not nine.
    #[test]
    fn an_even_division_falls_evenly() {
        let pieces = cut_box(&Tiling { size: 15.0, ..Tiling::default() });
        assert_eq!(pieces.len(), 4);
        for piece in &pieces {
            assert!((volume(piece) - 2250.0).abs() < 1e-6, "a quarter came out at {} mm3", volume(piece));
        }
    }

    /// The bounds of the 30 x 30 x 10 box every test here cuts.
    fn box_bounds() -> (Vec3, Vec3) {
        box_mesh(30.0, 30.0, 10.0).bounds().expect("a box has bounds")
    }

    #[test]
    fn the_preview_draws_the_grid_at_both_ends_of_the_run() {
        // The loops are what the tool draws over the model, so they have to be
        // *on* the model: at the top face and the bottom one, in the shape's own
        // frame, and nowhere in between when there are no layers.
        let tiling = Tiling { size: 15.0, ..Tiling::default() };
        let (lo, hi) = box_bounds();
        let loops = preview_loops(&tiling, (lo, hi), 1000);
        // Four cells across a 30 mm square at 15 mm, at each of two depths.
        assert_eq!(loops.len(), 8, "the grid was not drawn at both ends");
        let depths: Vec<f64> = loops.iter().map(|l| l[0].z).collect();
        assert!(depths.iter().any(|z| (z - hi.z).abs() < 1e-9), "nothing was drawn on the top face");
        assert!(depths.iter().any(|z| (z - lo.z).abs() < 1e-9), "nothing was drawn on the bottom face");
        assert!(depths.iter().all(|z| (z - hi.z).abs() < 1e-9 || (z - lo.z).abs() < 1e-9), "{depths:?}");
        // And every loop is closed, four-cornered and the size it says.
        for outline in &loops {
            assert_eq!(outline.len(), 4);
            assert!((outline[0] - outline[1]).length() - 15.0 < 1e-9);
        }
    }

    #[test]
    fn layers_put_a_grid_at_every_cut_between_the_ends() {
        // A 10 mm run in 4 mm layers is cut at 4 and at 8 -- two planes between
        // the two faces, so four grids in all.
        let tiling = Tiling { size: 15.0, layer: 4.0, ..Tiling::default() };
        let (lo, hi) = box_bounds();
        let loops = preview_loops(&tiling, (lo, hi), 1000);
        let mut depths: Vec<f64> = loops.iter().map(|l| l[0].z).collect();
        depths.sort_by(|a, b| a.partial_cmp(b).unwrap());
        depths.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        assert_eq!(depths.len(), 4, "the layer cuts were not drawn: {depths:?}");
        assert!((depths[1] - (lo.z + 4.0)).abs() < 1e-9, "{depths:?}");
        assert!((depths[2] - (lo.z + 8.0)).abs() < 1e-9, "{depths:?}");
    }

    #[test]
    fn the_preview_follows_the_axis_it_is_cut_through() {
        // Cells running along X put their grids on the two X faces, not the Z
        // ones: a preview that ignored the axis would draw the grid flat on the
        // ground whichever way the cut runs.
        let tiling = Tiling { size: 15.0, axis: 0, ..Tiling::default() };
        let (lo, hi) = box_bounds();
        let loops = preview_loops(&tiling, (lo, hi), 1000);
        assert!(!loops.is_empty());
        for outline in &loops {
            let x = outline[0].x;
            assert!((x - lo.x).abs() < 1e-9 || (x - hi.x).abs() < 1e-9, "a loop was drawn at x = {x}");
            // And the loop lies in the plane, so every corner shares that x.
            assert!(outline.iter().all(|p| (p.x - x).abs() < 1e-9));
        }
    }

    #[test]
    fn the_preview_is_capped_rather_than_drawing_ten_thousand_loops() {
        // A split may ask for cells by the thousand, and a preview is not worth
        // a frame rate. What survives the cap is the end laid down first.
        let tiling = Tiling { size: 0.5, ..Tiling::default() };
        let (lo, hi) = box_bounds();
        let loops = preview_loops(&tiling, (lo, hi), 100);
        assert_eq!(loops.len(), 100);
        assert!(loops.iter().all(|l| (l[0].z - hi.z).abs() < 1e-9), "the cap ate the wrong end");
        assert!(preview_loops(&tiling, (lo, hi), 0).is_empty());
    }

    #[test]
    fn a_tiling_too_fine_to_cut_previews_nothing() {
        // The window has to say why rather than drawing a grid for a split that
        // will be refused; `refusal` is what says it, and the preview keeps out
        // of the way.
        let (lo, hi) = box_bounds();
        assert!(preview_loops(&Tiling { size: 0.0, ..Tiling::default() }, (lo, hi), 1000).is_empty());
    }

    #[test]
    fn the_offset_moves_where_the_cuts_fall() {
        let centred = cut_box(&Tiling { size: 15.0, ..Tiling::default() });
        let shifted = cut_box(&Tiling { size: 15.0, offset: [7.5, 7.5], ..Tiling::default() });
        // Centred on a 30 mm square a 15 mm grid falls exactly into four; half
        // a cell over, the same grid cuts nine unequal ones.
        assert_eq!(centred.len(), 4);
        assert_eq!(shifted.len(), 9);
        for pieces in [&centred, &shifted] {
            let total: f64 = pieces.iter().map(volume).sum();
            assert!((total - 9000.0).abs() < 1.0);
        }
    }

    /// The whole of the second half of the feature: two cuts, on two axes,
    /// leave the blocks their grids come to between them -- and they still add
    /// back up to the shape they were cut from.
    #[test]
    fn two_cuts_leave_the_pieces_both_of_them_make() {
        // A 30 x 30 x 10 plate in 10 mm squares through Z is nine columns of
        // 10 x 10 x 10. The second cut runs across the first -- through X, so
        // its cells lie in the Y-Z plane -- and cuts each of those columns into
        // the four a 5 mm grid makes of a 10 x 10 face.
        let plan = SplitPlan {
            passes: vec![
                Tiling { size: 10.0, axis: 2, ..Tiling::default() },
                Tiling { size: 5.0, axis: 0, ..Tiling::default() },
            ],
        };
        let pieces = cut_plan(&box_mesh(30.0, 30.0, 10.0), &plan, &|| {}, &never).expect("nothing abandoned it");
        assert_eq!(pieces.len(), 36, "nine columns cut four ways each are thirty-six blocks");
        let total: f64 = pieces.iter().map(volume).sum();
        assert!((total - 9000.0).abs() < 1.0, "the blocks hold {total} mm3 of the 9000 they were cut from");
        for piece in &pieces {
            assert!((volume(piece) - 250.0).abs() < 1e-6, "a block came out at {} mm3", volume(piece));
        }
    }

    /// A pass that cannot cut what it is given must not lose it: the pieces of
    /// the cut before it come through whole.
    #[test]
    fn a_cut_that_misses_leaves_the_pieces_it_was_given() {
        let plan = SplitPlan {
            passes: vec![
                Tiling { size: 10.0, axis: 2, ..Tiling::default() },
                Tiling { size: 100.0, axis: 0, ..Tiling::default() },
            ],
        };
        let pieces = cut_plan(&box_mesh(30.0, 30.0, 10.0), &plan, &|| {}, &never).expect("nothing abandoned it");
        assert_eq!(pieces.len(), 9);
        let total: f64 = pieces.iter().map(volume).sum();
        assert!((total - 9000.0).abs() < 1.0);
    }

    /// What the two cuts come to between them is what is counted and what is
    /// refused -- one cut that is fine on its own and a second that multiplies
    /// it past the limit is a split nobody can find the pieces of.
    #[test]
    fn a_plan_is_counted_and_refused_by_what_its_cuts_come_to_together() {
        let bounds = (Vec3::new(-50.0, -50.0, -5.0), Vec3::new(50.0, 50.0, 5.0));
        let one = Tiling { size: 5.0, axis: 2, ..Tiling::default() };
        let plan = SplitPlan { passes: vec![one, Tiling { axis: 0, ..one }] };
        assert!(one.refusal(bounds).is_none(), "one cut this size is fine on its own");
        assert!(plan.refusal(bounds).is_some(), "two of them are far past the limit and were let through");
        // Progress is measured against every cell that will be tried, which is
        // the first cut over the shape plus the second over each piece it left.
        let pair = SplitPlan {
            passes: vec![
                Tiling { size: 25.0, axis: 2, ..Tiling::default() },
                Tiling { size: 25.0, axis: 0, ..Tiling::default() },
            ],
        };
        let (first, both) = (planned(&pair.passes[0], bounds), pair.planned(bounds));
        assert_eq!(pair.work(bounds), first + both, "the bar would run at two speeds");
    }

    /// A split written before a split could be cut more than once says its one
    /// tiling as an object, and there is no reason to lose it over a pair of
    /// brackets.
    #[test]
    fn a_plan_reads_both_a_list_of_cuts_and_the_single_one_that_came_before_it() {
        let plan = SplitPlan { passes: vec![Tiling::default(), Tiling { axis: 0, ..Tiling::default() }] };
        let text = serde_json::to_string(&plan).expect("it writes");
        assert!(text.starts_with('['), "a plan is written as the list of cuts it is: {text}");
        assert_eq!(serde_json::from_str::<SplitPlan>(&text).expect("it reads"), plan);

        let one = Tiling { size: 12.0, ..Tiling::default() };
        let older = serde_json::to_string(&one).expect("it writes");
        assert_eq!(serde_json::from_str::<SplitPlan>(&older).expect("it reads"), SplitPlan::of(one));
    }
}
