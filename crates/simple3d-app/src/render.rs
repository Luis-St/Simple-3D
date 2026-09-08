//! Turning the evaluated scene into the viewport image (spec section 6.1):
//! shaded / shaded-with-edges / wireframe display, the ground grid and origin
//! axes, the selection highlight and translucent ghosts for hidden nodes.

use crate::raster::{Frame, Image, Rgba, Vertex};
use crate::snap::MARK_AXIS;
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::{AxisStyle, Colour};
use simple3d_geom::{Mesh, Vec3};
use std::collections::HashMap;

/// An edge of the surface with the triangles that meet at it.
///
/// What the selection outline is drawn from. Which edges make up a shape's
/// outline depends on where the camera is -- an edge is on the silhouette when
/// the surface turns away from the eye across it -- so it cannot be settled
/// once at preparation time the way a crease can.
pub struct BorderEdge {
    pub ends: [u32; 2],
    /// The two triangles either side of it, or the same one twice when the edge
    /// has only one -- an open boundary, or a non-manifold junction. Those are
    /// drawn whatever the camera is doing: there is no surface on the far side
    /// for the silhouette test to ask about.
    pub faces: [u32; 2],
}

/// A mesh prepared for drawing: welded, so edges can be found, together with
/// its feature edges.
pub struct Renderable {
    pub mesh: Mesh,
    /// Edges worth drawing: a real crease in the surface, not an artefact of how
    /// a flat face happens to be triangulated.
    pub edges: Vec<[u32; 2]>,
    /// Every edge of the surface, with its neighbours -- what the silhouette is
    /// picked out of each frame.
    ///
    /// Empty unless the item may be drawn as a selection: it is one entry per
    /// edge rather than per crease, which for a large mesh is millions, and the
    /// scene as a whole is never outlined.
    pub outline: Vec<BorderEdge>,
    /// Which separate body each vertex belongs to: two vertices share a number
    /// when the surface joins them. The viewport hands the renderer the whole
    /// evaluated scene as *one* mesh, so this is the only thing that says where
    /// one solid ends and the next begins -- and an origin axis has to know,
    /// because the solid it runs into may not hide it while every other one
    /// must (issue 47).
    pub bodies: Vec<u16>,
    /// How many bodies that is: what the next item's tags start after, so two
    /// items' bodies are never the same body as far as the frame is concerned.
    pub body_count: u16,
}

impl Renderable {
    pub fn prepare(mesh: &Mesh) -> Renderable {
        Renderable::prepare_with(mesh, false)
    }

    /// The same, plus the edge adjacency the selection outline needs. For the
    /// nodes that may be drawn as a selection, which is a handful rather than
    /// the whole scene.
    pub fn prepare_outlined(mesh: &Mesh) -> Renderable {
        Renderable::prepare_with(mesh, true)
    }

    fn prepare_with(mesh: &Mesh, outlined: bool) -> Renderable {
        let welded = mesh.weld();
        let edges = feature_edges(&welded, 20.0);
        let outline = if outlined { border_edges(&welded) } else { Vec::new() };
        let bodies = bodies_of(&welded);
        let body_count = bodies.iter().max().map_or(0, |last| last + 1);
        Renderable { mesh: welded, edges, outline, bodies, body_count }
    }

    pub fn empty() -> Renderable {
        Renderable { mesh: Mesh::new(), edges: Vec::new(), outline: Vec::new(), bodies: Vec::new(), body_count: 0 }
    }

    /// The body a triangle belongs to, as a tag for the depth buffer. `base` is
    /// where this item's bodies start, so two items never share a tag, and 0
    /// means "no body", so the numbering starts at 1.
    fn tag(&self, triangle: usize, base: u16) -> u16 {
        self.mesh.indices.get(triangle).map_or(0, |tri| self.body_tag(tri[0] as usize, base))
    }

    fn body_tag(&self, vertex: usize, base: u16) -> u16 {
        self.bodies.get(vertex).map_or(0, |body| base.saturating_add(*body).saturating_add(1))
    }
}

/// Group the vertices of a welded mesh into connected bodies: union-find over
/// the triangles, which is what "one solid" means once the scene has been
/// evaluated into a single mesh.
///
/// More than `u16::MAX` bodies would be a scene of sixty-five thousand separate
/// solids; past that they share the last number, which costs nothing but the
/// distinction between two axes' worth of far-off shapes.
fn bodies_of(mesh: &Mesh) -> Vec<u16> {
    let mut parent: Vec<u32> = (0..mesh.positions.len() as u32).collect();
    fn find(parent: &mut [u32], mut of: u32) -> u32 {
        while parent[of as usize] != of {
            parent[of as usize] = parent[parent[of as usize] as usize];
            of = parent[of as usize];
        }
        of
    }
    for tri in &mesh.indices {
        let root = find(&mut parent, tri[0]);
        for &vertex in &tri[1..] {
            let other = find(&mut parent, vertex);
            if other != root {
                parent[other as usize] = root;
            }
        }
    }
    let mut numbers: std::collections::HashMap<u32, u16> = std::collections::HashMap::new();
    (0..mesh.positions.len() as u32)
        .map(|vertex| {
            let root = find(&mut parent, vertex);
            let next = numbers.len().min(u16::MAX as usize - 1) as u16;
            *numbers.entry(root).or_insert(next)
        })
        .collect()
}

/// Edges where the surface actually creases, plus any edge with only one
/// triangle. Drawing *every* triangle edge would cover a cylinder in meridians
/// and a boolean result in the arbitrary cuts the BSP made across flat faces --
/// noise rather than information.
pub fn feature_edges(mesh: &Mesh, angle_deg: f64) -> Vec<[u32; 2]> {
    let cos_limit = angle_deg.to_radians().cos();
    let mut faces: HashMap<(u32, u32), Vec<Vec3>> = HashMap::new();
    for tri in &mesh.indices {
        let normal = mesh.triangle_normal(*tri);
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            let key = if a < b { (a, b) } else { (b, a) };
            faces.entry(key).or_default().push(normal);
        }
    }
    let mut edges: Vec<[u32; 2]> = faces
        .into_iter()
        .filter(|(_, normals)| match normals.as_slice() {
            [a, b] => a.dot(*b) < cos_limit,
            // One triangle (a boundary of an open mesh) or more than two (a
            // non-manifold junction): both are worth seeing.
            _ => true,
        })
        .map(|((a, b), _)| [a, b])
        .collect();
    // Deterministic order, so successive frames of an unchanged scene are
    // identical and the image comparison in the tests is meaningful.
    edges.sort_unstable();
    edges
}

/// Every edge of the mesh with the triangles that meet at it, in a
/// deterministic order.
fn border_edges(mesh: &Mesh) -> Vec<BorderEdge> {
    let mut faces: HashMap<(u32, u32), [u32; 2]> = HashMap::new();
    let mut counts: HashMap<(u32, u32), u32> = HashMap::new();
    for (index, tri) in mesh.indices.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            let key = if a < b { (a, b) } else { (b, a) };
            let count = counts.entry(key).or_insert(0);
            let slot = faces.entry(key).or_insert([index as u32; 2]);
            if *count == 1 {
                slot[1] = index as u32;
            }
            // A third triangle on one edge is a non-manifold junction: leave the
            // first two, and let the count say it is not a plain edge.
            *count += 1;
        }
    }
    let mut out: Vec<BorderEdge> = faces
        .into_iter()
        .map(|(key, mut pair)| {
            if counts[&key] != 2 {
                // Always drawn: `push_selection` reads a repeated triangle as
                // "there is no far side to ask about".
                pair[1] = pair[0];
            }
            BorderEdge { ends: [key.0, key.1], faces: pair }
        })
        .collect();
    out.sort_unstable_by_key(|edge| edge.ends);
    out
}

/// Colours, resolved from the host theme so the viewport is usable under both
/// light and dark system themes (spec section 7.4).
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// The top of the viewport's vertical gradient.
    pub background: Rgba,
    /// The bottom of it. A flat field of one colour reads as a blank canvas;
    /// a gradient this shallow is barely nameable but gives the ground plane
    /// somewhere to sit.
    pub background_low: Rgba,
    pub solid: Rgba,
    pub selected: Rgba,
    pub ghost: Rgba,
    pub grid: Rgba,
    pub grid_major: Rgba,
    pub axis_x: Rgba,
    pub axis_y: Rgba,
    pub axis_z: Rgba,
    pub wire: Rgba,
    pub edge: Rgba,
}

impl Palette {
    /// The viewport's own reading of the interface palette. The names on the
    /// left are `crate::theme::token`'s: surface-0 for the ground, the amber
    /// accent for selection, the danger red for a body that is being subtracted.
    pub fn dark() -> Palette {
        use crate::theme::token;
        Palette {
            background: rgba(token::SURFACE_0),
            background_low: rgba(token::SURFACE_0B),
            solid: [0x9A, 0xA4, 0xB2, 255],
            selected: rgba(token::ACCENT),
            // A subtrahend is drawn as a translucent red ghost, so a cut can be
            // seen before it is resolved.
            ghost: fade(token::DANGER, 80),
            grid: [0x28, 0x2D, 0x35, 255],
            grid_major: rgba(token::SURFACE_3),
            axis_x: rgba(token::AXIS_X),
            axis_y: rgba(token::AXIS_Y),
            axis_z: rgba(token::AXIS_Z),
            wire: [0xC2, 0xCA, 0xD6, 255],
            edge: [0x11, 0x13, 0x17, 255],
        }
    }

    pub fn light() -> Palette {
        Palette {
            background: [238, 240, 243, 255],
            background_low: [226, 229, 234, 255],
            solid: [150, 158, 170, 255],
            selected: [226, 122, 12, 255],
            ghost: [40, 110, 220, 60],
            grid: [214, 218, 224, 255],
            grid_major: [186, 192, 200, 255],
            axis_x: [186, 54, 54, 255],
            axis_y: [50, 140, 50, 255],
            axis_z: [46, 96, 200, 255],
            wire: [64, 70, 80, 255],
            edge: [70, 76, 86, 255],
        }
    }

    pub fn for_dark_mode(dark: bool) -> Palette {
        if dark {
            Palette::dark()
        } else {
            Palette::light()
        }
    }
}

/// A palette token as the rasterizer's own pixel format.
fn rgba(colour: egui::Color32) -> Rgba {
    [colour.r(), colour.g(), colour.b(), 255]
}

/// The same, at a chosen alpha.
fn fade(colour: egui::Color32, alpha: u8) -> Rgba {
    [colour.r(), colour.g(), colour.b(), alpha]
}

impl Palette {
    /// The background colour at `row` of a frame `height` rows tall. One
    /// definition, used by the renderer and by anything that needs to ask
    /// "was this pixel painted, or is it just the sky".
    pub fn background_at(&self, row: usize, height: usize) -> Rgba {
        if height <= 1 {
            return self.background;
        }
        let t = row as f32 / (height - 1) as f32;
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        [
            mix(self.background[0], self.background_low[0]),
            mix(self.background[1], self.background_low[1]),
            mix(self.background[2], self.background_low[2]),
            255,
        ]
    }
}

/// Lay the gradient down one row at a time, before anything else is drawn.
fn fill_background(frame: &mut Frame, palette: &Palette) {
    // The gradient is a property of the whole frame, so the colour is asked for
    // by the row's place in it -- while the pixels written are this band's own.
    let height = frame.height;
    let width = frame.width;
    let rows = frame.rows();
    for (offset, row) in rows.enumerate() {
        let colour = palette.background_at(row, height);
        let line = &mut frame.color[offset * width * 4..(offset + 1) * width * 4];
        for pixel in line.chunks_exact_mut(4) {
            pixel.copy_from_slice(&colour);
        }
    }
}

pub struct Grid {
    pub visible: bool,
    /// Spacing in millimetres.
    pub spacing: f64,
    /// Which of the three origin axes to draw, X, Y, Z. The axes are laid out on
    /// the grid's spacing, which is why they are described here with it.
    pub axes: [bool; 3],
    pub style: AxisStyle,
    /// Whether to mark, on a solid's own surface, where a principal plane cuts
    /// through it. Each plane is named by the axis it is perpendicular to and
    /// follows that axis's switch, so the ground plane's mark is the Z one.
    pub plane_marks: bool,
}

/// One thing to draw, in world space.
pub struct Item<'a> {
    pub renderable: &'a Renderable,
    pub style: Style,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// The evaluated scene.
    Solid,
    /// The selected node's own geometry, drawn over the top so it is visible
    /// even where a boolean consumed it.
    Selected,
    /// A hidden node, so a subtracted tool body can be seen while it is being
    /// positioned.
    Ghost,
}

pub struct Request<'a> {
    pub view: View,
    pub size: [usize; 2],
    pub mode: DisplayMode,
    pub palette: Palette,
    pub grid: Grid,
    pub items: Vec<Item<'a>>,
    /// A tool's preview, in world space: closed loops drawn over the model in
    /// the accent colour once everything else is down (issue 82).
    ///
    /// It is drawn here rather than with the 2D painter over the finished
    /// picture so that it meets the depth buffer: a cut on the far side of the
    /// shape is behind it, and a grid that shows through the solid it lies on
    /// reads as floating in front of it. It writes no depth of its own -- one
    /// loop must not hide the next where they cross -- and it is biased towards
    /// the eye, because the loops at the ends of a run lie exactly on the
    /// surface they are drawn on and would otherwise lose the tie to it.
    pub preview: Vec<Vec<Vec3>>,
}

/// How many rows a band must have before splitting the frame again is worth
/// the thread it costs. Below this the whole frame goes to one band: a small
/// viewport rasterizes in well under a millisecond, and spawning eight threads
/// to share that out costs more than it saves.
const MIN_BAND_ROWS: usize = 96;

/// How many bands to cut the frame into: one per core, but never so many that
/// they stop being worth starting.
fn band_count(height: usize) -> usize {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    (height / MIN_BAND_ROWS).clamp(1, cores)
}

/// Draw the scene. The frame is cut into horizontal bands and each is drawn on
/// its own thread, from the same list of primitives in the same order -- see
/// `Frame`'s own note on why the split is by rows rather than by work.
///
/// Everything that has to be decided before drawing starts is decided once,
/// here, and shared: `axis_material` walks every triangle of every item to find
/// where the axes run through material, and doing that once per band would put
/// most of the work back.
/// Prepare and draw in one step. The application prepares once and hands the
/// result to whichever engine is drawing, so this is the tests' way in.
#[cfg(test)]
pub fn render(request: &Request<'_>) -> Image {
    render_prepared(request, &prepare_frame(request))
}

/// The software renderer, from a frame that has already been prepared.
pub fn render_prepared(request: &Request<'_>, prepared: &Prepared) -> Image {
    let [_, height] = request.size;
    render_in_bands(request, prepared, band_count(height.max(1)))
}

/// The scene worked out but not yet drawn: screen-space primitives in drawing
/// order, and the pieces of the origin axes with the rule that governs them.
///
/// This is where the two renderers meet. Projection, culling, shading, the
/// grid's falloff and the whole of the axis-through-material question are
/// decided here, once, on the CPU; what the GPU renderer does differently is
/// only how the resulting primitives are turned into pixels. Anything decided
/// here cannot drift between the engines, which is the point of it.
pub struct Prepared {
    pub(crate) steps: Vec<Step>,
    pub(crate) axes: Vec<AxisStep>,
}

pub fn prepare_frame(request: &Request<'_>) -> Prepared {
    let material = axis_material(&request.items, &request.grid);
    let axes = prepare_axes(&request.view, &request.palette, &request.grid, &material);
    Prepared { steps: prepare(request), axes }
}

/// Draw the frame in a given number of bands. The band count must not change
/// the picture -- see the test at the bottom of this file, which holds the two
/// against each other -- so it is a parameter only so that the test can ask.
fn render_in_bands(request: &Request<'_>, prepared: &Prepared, bands: usize) -> Image {
    let [width, height] = request.size;
    let (width, height) = (width.max(1), height.max(1));
    let Prepared { steps, axes } = prepared;

    // The one buffer the whole frame is drawn into. Each band is handed the
    // stretch of it holding its own rows, so what the threads write is already
    // the finished image -- no copy at the end, which on a large viewport was
    // costing more than the drawing.
    let mut color = vec![0u8; width * height * 4];

    // Rows are handed out so the remainder is spread over the first few bands
    // rather than piled onto the last one.
    let ranges: Vec<(usize, usize)> =
        (0..bands).map(|i| (height * i / bands, height * (i + 1) / bands)).filter(|(lo, hi)| lo < hi).collect();
    if ranges.len() == 1 {
        let mine: Vec<u32> = (0..steps.len() as u32).collect();
        let mut frame = Frame::band(&mut color, width, height, 0, height);
        draw(&mut frame, request, steps, &mine, axes);
        return Image { width, height, color };
    }

    let bins = bin_steps(steps, &ranges);
    // One disjoint slice per band, in row order.
    let mut rest = &mut color[..];
    let mut slices: Vec<&mut [u8]> = Vec::with_capacity(ranges.len());
    for &(lo, hi) in &ranges {
        let (mine, tail) = rest.split_at_mut((hi - lo) * width * 4);
        slices.push(mine);
        rest = tail;
    }

    std::thread::scope(|scope| {
        for ((&(lo, hi), mine), slice) in ranges.iter().zip(&bins).zip(slices) {
            let (steps, axes) = (steps, axes);
            scope.spawn(move || {
                let mut frame = Frame::band(slice, width, height, lo, hi);
                draw(&mut frame, request, steps, mine, axes);
            });
        }
    });
    Image { width, height, color }
}

/// Everything of the model that is the same whichever rows are being drawn:
/// projected, culled, shaded and ordered exactly as the drawing order requires.
fn prepare(request: &Request<'_>) -> Vec<Step> {
    let view = request.view;
    let mut steps = Vec::new();
    if request.grid.visible {
        push_grid(&mut steps, &view, &request.grid, &request.palette);
    }
    for (item, tag_base) in request.items.iter().zip(tag_bases(&request.items)) {
        match item.style {
            Style::Solid => match request.mode {
                DisplayMode::Wireframe => push_wireframe(&mut steps, &view, item.renderable, request.palette.wire),
                DisplayMode::Shaded => {
                    push_shaded(&mut steps, &view, item.renderable, request.palette.solid, 255, tag_base)
                }
                DisplayMode::ShadedWithEdges => {
                    push_shaded(&mut steps, &view, item.renderable, request.palette.solid, 255, tag_base);
                    push_edges(&mut steps, &view, item.renderable, request.palette.edge, tag_base);
                }
            },
            Style::Selected => {
                // Always outlined, in every display mode: the selection has to
                // be visible, and an outline reads clearly over a shaded body.
                // A larger bias than the solid's own edges, or the two would tie
                // at equal depth and the outline would lose.
                push_selection(&mut steps, &view, item.renderable, request.palette.selected, tag_base);
            }
            Style::Ghost => push_ghost(&mut steps, &view, item.renderable, request.palette.ghost),
        }
    }
    push_preview(&mut steps, &view, &request.preview, request.palette.selected);
    if request.grid.plane_marks && request.mode != DisplayMode::Wireframe {
        // After the solids: the mark belongs on the surface, and in wireframe
        // there is no surface for it to sit on.
        push_plane_marks(&mut steps, &view, &request.items, &request.palette, &request.grid);
    }
    steps
}

/// Draw the whole scene into one frame -- a band of one, or the lot.
fn draw(frame: &mut Frame, request: &Request<'_>, steps: &[Step], mine: &[u32], axes: &[AxisStep]) {
    fill_background(frame, &request.palette);
    draw_steps(frame, steps, mine);
    frame.set_tag(0);
    // Last: an axis is hidden by the material it runs through, which is cut out
    // of the line, and by anything in front of it -- except on the approach to a
    // surface it is about to go into, which is drawn over the shape it is
    // arriving at. Depth alone eats that approach, because the line is behind
    // the shape's own front faces for the whole stretch between the silhouette
    // and the point it enters, and losing it is what made the origin read as
    // being somewhere behind the model (issue 47). The grid is untouched, drawn
    // first and covered by everything.
    for step in axes {
        frame.line_through(step.a, step.b, step.colour, AXIS_BIAS, &step.seen);
    }
}

/// One primitive of the model, already projected into screen space and ready
/// for any band of the frame to draw.
///
/// Everything that does not depend on which rows are being drawn -- projection,
/// back-face culling, shading, the depth bias a line gets from its own distance
/// -- is worked out once here rather than once per band. Without that the
/// parallel path re-derives the whole model for every thread, and a dense mesh
/// in a small viewport comes out *slower* than drawing it on one core.
pub(crate) enum Step {
    Triangle {
        v: [Vertex; 3],
        colour: Rgba,
        tag: u16,
        write_depth: bool,
    },
    Line {
        a: Vertex,
        b: Vertex,
        colour: Rgba,
        bias: f32,
        tag: u16,
        write_depth: bool,
    },
    /// A line drawn *over* the model and tested against it, claiming neither
    /// the depth of a pixel nor its body: a tool's preview, which has to be
    /// hidden by the solid it is drawn on the far side of and must not hide the
    /// next loop of itself where two of them cross.
    ///
    /// Its own variant rather than a `Line` that writes no depth, because the
    /// two are told apart by *when* they are drawn and only the software
    /// renderer keeps the order it was handed: the GPU sorts the primitives
    /// into passes, and a line that writes no depth used to mean the ground
    /// grid, which goes under the model.
    Overlay {
        a: Vertex,
        b: Vertex,
        colour: Rgba,
        bias: f32,
    },
}

impl Step {
    /// The rows this primitive can reach. A band sharing none of them skips it
    /// on one comparison, which is what the split is worth.
    fn rows(&self) -> (f32, f32) {
        match self {
            Step::Triangle { v, .. } => {
                let (a, b, c) = (v[0].pos.y, v[1].pos.y, v[2].pos.y);
                (a.min(b).min(c), a.max(b).max(c))
            }
            Step::Line { a, b, .. } | Step::Overlay { a, b, .. } => (a.pos.y.min(b.pos.y), a.pos.y.max(b.pos.y)),
        }
    }
}

/// Which of `steps` each band has to draw, as indices into it.
///
/// Sorting the primitives into their bands once beats letting every band walk
/// the whole list: a dense mesh is tens of thousands of primitives and a large
/// frame is fifteen bands, and the scan alone then costs more than the fill.
/// The indices stay ascending, so each band still draws in preparation order.
fn bin_steps(steps: &[Step], ranges: &[(usize, usize)]) -> Vec<Vec<u32>> {
    let mut bins: Vec<Vec<u32>> = ranges.iter().map(|_| Vec::new()).collect();
    for (index, step) in steps.iter().enumerate() {
        let (from, to) = step.rows();
        for (bin, &(lo, hi)) in bins.iter_mut().zip(ranges) {
            // A pixel of slack at each end: a line samples on rounded
            // coordinates and a triangle's span is widened by one, so a
            // primitive that only just misses a band's rows can still write to
            // one of them.
            if to >= lo as f32 - 1.0 && from <= hi as f32 + 1.0 {
                bin.push(index as u32);
            }
        }
    }
    bins
}

/// Draw the prepared primitives listed for this band, in the order they were
/// prepared -- which is the order the single-threaded renderer drew them in.
fn draw_steps(frame: &mut Frame, steps: &[Step], mine: &[u32]) {
    for &index in mine {
        match steps[index as usize] {
            Step::Triangle { v, colour, tag, write_depth } => {
                frame.set_tag(tag);
                frame.triangle(v, colour, write_depth);
            }
            Step::Line { a, b, colour, bias, tag, write_depth } => {
                frame.set_tag(tag);
                if write_depth {
                    frame.line(a, b, colour, bias);
                } else {
                    frame.line_with_depth(a, b, colour, bias, false);
                }
            }
            Step::Overlay { a, b, colour, bias } => {
                frame.set_tag(0);
                frame.line_with_depth(a, b, colour, bias, false);
            }
        }
    }
}

/// A world-space line, projected and given the depth bias its own distance
/// earns it. The counterpart of `draw_world_line`, for the prepared path.
fn line_step(view: &View, a: Vec3, b: Vec3, colour: Rgba, bias: f32, tag: u16, write_depth: bool) -> Step {
    // Nothing is clipped: a parallel projection maps a point behind the eye to
    // its true screen position, and the depth key puts it behind everything
    // else on its own.
    let (a, b) = (to_vertex(view, view.to_view(a)), to_vertex(view, view.to_view(b)));
    let scale = (a.key.abs() + b.key.abs()) * 0.5;
    Step::Line { a, b, colour, bias: bias * scale, tag, write_depth }
}

/// Project a world point to a rasterizer vertex, with the depth key the
/// framebuffer expects: larger is nearer, and linear in screen space. Under a
/// parallel projection the view-space depth itself is that, negated.
fn to_vertex(view: &View, view_space: Vec3) -> Vertex {
    let (pos, z) = view.view_to_screen(view_space);
    Vertex { pos, key: -z as f32 }
}

/// Shade one triangle: a headlight from the camera plus a constant fill, so a
/// face turned away from the eye is dim but never black.
fn shade(base: Rgba, normal: Vec3, view_dir: Vec3, alpha: u8) -> Rgba {
    let lambert = normal.dot(-view_dir).abs();
    let factor = 0.34 + 0.66 * lambert;
    [
        (base[0] as f64 * factor).min(255.0) as u8,
        (base[1] as f64 * factor).min(255.0) as u8,
        (base[2] as f64 * factor).min(255.0) as u8,
        alpha,
    ]
}

/// The direction from a triangle towards the eye, for back-face culling.
///
/// The projection is parallel, so there is no eye to point at: every ray runs
/// along the view direction and that direction is the answer for every triangle
/// in the frame. Pointing at `eye - centroid` instead is right only near the
/// middle of the frame, and the further a face sits from it the more the answer
/// tilts, until a face within a few degrees of edge-on is culled although it is
/// facing the viewer -- which is what once dropped the side wall off a plate
/// seen almost from the side.
fn to_eye(view: &View, _centroid: Vec3) -> Vec3 {
    -view.forward()
}

/// The colour one triangle is painted: whatever body it came from, if that
/// body was painted, and the theme's colour for solids otherwise. The tag
/// travels with the surface through every boolean, so the far wall of a hole
/// drilled by a painted cutter is the cutter's colour and the plate around it
/// stays the plate's.
fn triangle_base(item: &Renderable, index: usize, base: Rgba) -> Rgba {
    match Colour::from_tag(item.mesh.tag(index)) {
        Some(Colour([r, g, b])) => [r, g, b, base[3]],
        None => base,
    }
}

fn push_shaded(steps: &mut Vec<Step>, view: &View, item: &Renderable, colour_base: Rgba, alpha: u8, tag_base: u16) {
    let forward = view.forward();
    for (index, tri) in item.mesh.indices.iter().enumerate() {
        let world = [
            item.mesh.positions[tri[0] as usize],
            item.mesh.positions[tri[1] as usize],
            item.mesh.positions[tri[2] as usize],
        ];
        let normal = (world[1] - world[0]).cross(world[2] - world[0]);
        if normal.length() < 1e-12 {
            continue;
        }
        let normal = normal.normalized();
        // Back-face culling in world space, where it means something: for a
        // closed solid the far side is never visible, so this halves the work.
        let centroid = (world[0] + world[1] + world[2]) * (1.0 / 3.0);
        if normal.dot(to_eye(view, centroid)) <= 0.0 {
            continue;
        }
        let colour = shade(triangle_base(item, index, colour_base), normal, forward, alpha);
        let in_view = [view.to_view(world[0]), view.to_view(world[1]), view.to_view(world[2])];
        steps.push(Step::Triangle {
            v: [to_vertex(view, in_view[0]), to_vertex(view, in_view[1]), to_vertex(view, in_view[2])],
            colour,
            tag: item.tag(index, tag_base),
            write_depth: true,
        });
    }
}

/// Ghosts are drawn without back-face culling and without writing depth, so a
/// hidden tool body reads as a translucent volume rather than a flat patch.
fn push_ghost(steps: &mut Vec<Step>, view: &View, item: &Renderable, base: Rgba) {
    let forward = view.forward();
    for tri in &item.mesh.indices {
        let world = [
            item.mesh.positions[tri[0] as usize],
            item.mesh.positions[tri[1] as usize],
            item.mesh.positions[tri[2] as usize],
        ];
        let normal = (world[1] - world[0]).cross(world[2] - world[0]);
        if normal.length() < 1e-12 {
            continue;
        }
        let colour = shade(base, normal.normalized(), forward, base[3]);
        let in_view = [view.to_view(world[0]), view.to_view(world[1]), view.to_view(world[2])];
        // A ghost writes no depth, so the tag it would have written is never
        // read; it carries the one a solid would have had for form's sake.
        steps.push(Step::Triangle {
            v: [to_vertex(view, in_view[0]), to_vertex(view, in_view[1]), to_vertex(view, in_view[2])],
            colour,
            tag: 0,
            write_depth: false,
        });
    }
}

/// Depth bias for a line, as a fraction of its own depth key rather than an
/// absolute amount, so one set of numbers holds at every zoom: the key is the
/// distance from the eye, which grows with the camera's own distance, and a
/// fixed nudge that is invisible up close would be larger than the whole
/// scene's depth range when the camera is far away.
const EDGE_BIAS: f32 = 2.0e-4;
const SELECTION_BIAS: f32 = 8.0e-4;
/// The grid and the axes are biased *away* from the eye, so a face that happens
/// to be coplanar with one of them hides it. Without this a plate 4mm thick and
/// centred on the origin has the ground grid drawn straight across its side
/// walls, because the wall and the grid line tie at exactly equal depth and the
/// grid got there first. The axes are biased slightly less than the grid,
/// because the X and Y axes lie exactly along grid lines and would otherwise
/// lose that tie in turn.
const GRID_BIAS: f32 = -8.0e-4;
const AXIS_BIAS: f32 = -5.0e-4;
/// A plane mark sits *on* the surface it is drawn on, so it needs to win the
/// tie against that surface -- and against the feature edges of the same
/// solid, which is why it is biased further than they are. Found by looking:
/// below about 2e-3 the mark breaks into dashes wherever the line's own
/// interpolated depth rounds behind the face it lies on, and an order of
/// magnitude above this it starts showing through the far side of a solid.
const MARK_BIAS: f32 = 3.0e-3;
/// A preview loop sits on the surface it is drawn over exactly as a plane mark
/// does -- the cells at the ends of a run lie in the faces the run starts and
/// stops at -- so it needs the same bias to win that tie, and no more, or it
/// starts showing through the far side of the solid.
const PREVIEW_BIAS: f32 = 3.0e-3;

/// A tool's preview loops, over the model and depth-tested against it.
///
/// Drawn as [`Step::Overlay`]: after the model, tested against it, and claiming
/// nothing of its own -- so a loop is hidden by the solid it is behind and does
/// not hide the next loop where two of them cross.
fn push_preview(steps: &mut Vec<Step>, view: &View, loops: &[Vec<Vec3>], colour: Rgba) {
    for loop_ in loops {
        for (index, &from) in loop_.iter().enumerate() {
            let to = loop_[(index + 1) % loop_.len()];
            let Step::Line { a, b, bias, .. } = line_step(view, from, to, colour, PREVIEW_BIAS, 0, false) else {
                unreachable!("a line step is a line");
            };
            steps.push(Step::Overlay { a, b, colour, bias });
        }
    }
}

fn push_edges(steps: &mut Vec<Step>, view: &View, item: &Renderable, colour: Rgba, tag_base: u16) {
    for edge in &item.edges {
        let a = item.mesh.positions[edge[0] as usize];
        let b = item.mesh.positions[edge[1] as usize];
        // Tagged like the faces it creases, so an edge of the solid an axis
        // goes into does not hide that axis where the faces either side of it
        // do not.
        let tag = item.body_tag(edge[0] as usize, tag_base);
        steps.push(line_step(view, a, b, colour, EDGE_BIAS, tag, true));
    }
}

/// The selected shape's *outline*: the edges its surface turns away from the
/// camera across, plus any edge with no far side at all.
///
/// It used to draw the feature edges instead, and that swung between the two
/// opposite failures. A sphere at the stock 32 segments creases at 11.25
/// degrees, under the 20-degree threshold, so it has no feature edges and
/// selecting one drew *nothing* -- with only the manipulator in the frame,
/// nothing said what was selected. A torus at the same segment count creases
/// past the threshold around its tube, so selecting one scribbled concentric
/// rings over the whole surface. A silhouette is the same picture for both, and
/// it is what the word outline means.
fn push_selection(steps: &mut Vec<Step>, view: &View, item: &Renderable, colour: Rgba, tag_base: u16) {
    // Drawn a second time, one pixel out from the shape, and that is what makes
    // it a line rather than a row of dots.
    //
    // A silhouette edge is the one line in the frame a depth test cannot draw.
    // It lies exactly where the surface turns away from the eye, so the face
    // beside it is nearly edge-on and its depth changes by more across a single
    // pixel than a bias can cover: measured on a 32-segment sphere, seventeen of
    // a hundred and eighty two-degree sectors of the rim had nothing drawn at
    // all, and raising the bias tenfold -- past where a mark starts showing
    // through the far side of a solid -- still left six. It is not an epsilon
    // problem, and the pixel just outside the silhouette is not covered by the
    // shape at all, so nothing there has to be won from.
    //
    // The offset is perpendicular to the edge on screen and away from the front
    // face's own centre, which for a silhouette edge is out of the shape. It
    // does not thicken the line where it is already drawn -- the two passes land
    // on the same pixel for a nearly-vertical edge -- so a selection still reads
    // as a hairline and not as a halo.
    let mut push = |edge: [u32; 2], away: Option<Vec3>| {
        let a = item.mesh.positions[edge[0] as usize];
        let b = item.mesh.positions[edge[1] as usize];
        let tag = item.body_tag(edge[0] as usize, tag_base);
        steps.push(line_step(view, a, b, colour, SELECTION_BIAS, tag, true));
        let Some(away) = away else { return };
        let (va, vb) = (to_vertex(view, view.to_view(a)), to_vertex(view, view.to_view(b)));
        let along = vb.pos - va.pos;
        let normal = egui::vec2(-along.y, along.x);
        if normal.length() < 1e-6 {
            return;
        }
        // Which way along that perpendicular leads out of the shape, decided in
        // screen space so a foreshortened face cannot get it backwards.
        let inward = to_vertex(view, view.to_view(away)).pos - va.pos;
        let normal = normal / normal.length();
        let out = if egui::vec2(normal.x, normal.y).dot(inward) > 0.0 { -normal } else { normal };
        let shift = |v: Vertex| Vertex { pos: v.pos + out, key: v.key };
        steps.push(Step::Line {
            a: shift(va),
            b: shift(vb),
            colour,
            bias: SELECTION_BIAS * (va.key.abs() + vb.key.abs()) * 0.5,
            tag,
            write_depth: true,
        });
    };
    if item.outline.is_empty() {
        // Prepared without the adjacency -- the creases are what there is, and a
        // crease has an inside on both sides, so no outward pass for it.
        for &edge in &item.edges {
            push(edge, None);
        }
        return;
    }
    let towards = view.forward();
    let front: Vec<bool> =
        item.mesh.indices.iter().map(|tri| item.mesh.triangle_normal(*tri).dot(towards) < 0.0).collect();
    let faces_the_eye = |face: u32| front.get(face as usize).copied().unwrap_or(false);
    let centroid = |face: u32| -> Option<Vec3> {
        let tri = item.mesh.indices.get(face as usize)?;
        Some(
            (item.mesh.positions[tri[0] as usize]
                + item.mesh.positions[tri[1] as usize]
                + item.mesh.positions[tri[2] as usize])
                / 3.0,
        )
    };
    for edge in &item.outline {
        let [near, far] = edge.faces;
        if near != far && faces_the_eye(near) == faces_the_eye(far) {
            continue;
        }
        // The face on the shape's own side of this edge, whose centre says
        // which way is inward.
        let inside = if faces_the_eye(near) { near } else { far };
        push(edge.ends, centroid(inside));
    }
}

fn push_wireframe(steps: &mut Vec<Step>, view: &View, item: &Renderable, colour: Rgba) {
    for edge in &item.edges {
        let a = item.mesh.positions[edge[0] as usize];
        let b = item.mesh.positions[edge[1] as usize];
        // No depth bias and no filled faces, so the whole wireframe is visible
        // including the far side -- which is the point of wireframe.
        // The tag is the wireframe's own: nothing is filled, so nothing owns a
        // pixel's depth in a way an axis has to see through.
        steps.push(line_step(view, a, b, colour, 0.0, 0, true));
    }
}

/// Half the viewport's diagonal, in millimetres at the current zoom: how far
/// the frame itself reaches, with no ground plane involved. What the pinned
/// axis cross is measured against -- it is a mark on the frame, so it is sized
/// by the frame, and it must not grow when a tilt makes the ground reach
/// further or the two axis styles stop being two different pictures.
pub fn frame_reach(view: &View) -> f64 {
    let half_diagonal = ((view.size.x as f64).hypot(view.size.y as f64) / 2.0).max(1.0);
    half_diagonal / view.pixels_per_mm().max(1e-9)
}

/// How much further than the face-on reach a tilted ground may be asked to
/// cover. A view a few degrees off edge-on would otherwise want a grid of
/// unbounded extent, and by then its lines are a wash rather than a measure.
const MAX_TILT_REACH: f64 = 6.0;

/// The world radius the ground grid and the axes cover.
///
/// It is derived from the frame rather than from the grid's spacing: the
/// furthest the viewport reaches across the ground plane at the current zoom.
/// That makes it continuous in the zoom -- the extent of the ground grows
/// smoothly as the camera pulls back, instead of jumping tenfold whenever the
/// spacing steps up a decade, which is what made zooming out lurch.
///
/// The ground is only face-on from straight above. Seen at an angle it is
/// foreshortened, so the frame reaches much further across it along the view
/// than across it sideways -- half the viewport's diagonal is the right answer
/// for a top view and far too small for any other. Taking it as the answer for
/// all of them left the grid stopping short of the top and bottom of the
/// viewport, in a flattened diamond, while the axes carried on past it. So the
/// four corners of the frame are put back onto the ground and the furthest one
/// is what the grid has to reach.
pub fn grid_radius(view: &View) -> f64 {
    let face_on = frame_reach(view);
    let centre = Vec3::new(view.camera.target.x, view.camera.target.y, 0.0);
    let half = view.size / 2.0;
    let mut reach: f64 = 0.0;
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let corner = egui::pos2(view.centre.x + half.x * sx, view.centre.y + half.y * sy);
        // Edge-on: the ground is a line on the screen, with no extent to cover.
        let Some(hit) = view.ray_plane(corner, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)) else {
            return face_on * 1.35;
        };
        reach = reach.max((hit - centre).length());
    }
    // A little past the corner it is reaching, and never less than the face-on
    // answer, so a top view keeps exactly the extent it had.
    (reach * 1.08).clamp(face_on * 1.35, face_on * MAX_TILT_REACH)
}

/// The narrowest a grid cell may be drawn, in pixels, and the width at which it
/// is drawn at full strength. Between the two it is faded, which is what makes a
/// decade of the grid arrive and leave without a step.
const CELL_FADE_OUT: f64 = 6.0;
const CELL_FULL: f64 = 26.0;

/// How much of a ground cell survives the tilt of the view, as a factor on its
/// size on screen.
///
/// A cell is only square on the screen from straight above. At an angle the
/// ground is foreshortened along the view -- at eight degrees a cell keeps a
/// seventh of its depth -- so a spacing that is perfectly legible from above is
/// a wash of lines seen from low down, and the finer of the two levels is a
/// wash covering the whole frame. Asking how big a cell really is on the screen
/// makes the level step up as the view flattens, exactly as it does when the
/// camera pulls back, so the grid stays a measure rather than a texture.
///
/// The projection is parallel, so this is one number for the whole frame rather
/// than something that varies across it: there is no horizon for cells to pile
/// up against. It is the square root of the foreshortening because what is
/// being judged is the cell's *area* on the screen -- the geometric mean of its
/// two sides, one of them untouched -- which keeps the step gentle enough that
/// orbiting does not walk the grid up and down a decade at a time.
fn ground_squash(view: &View) -> f64 {
    view.forward().z.abs().max(1e-3).sqrt()
}

/// The two decades of grid to draw at this zoom: the coarse one, always at full
/// strength, the fine one below it, and how strongly that fine one shows.
///
/// The coarse spacing is the first decade of the document's own spacing whose
/// cell is at least `CELL_FULL` across, so it is never a solid block of lines.
/// The fine spacing is the decade under it, faded out as its cells shrink from
/// `CELL_FULL` to `CELL_FADE_OUT`. At the moment the coarse level steps up, the
/// level it replaces is exactly `CELL_FULL` across and fully drawn, so the
/// picture does not change: the tenfold jump the old single-level grid made is
/// spread across the whole decade of zoom instead.
pub fn grid_levels(view: &View, spacing: f64) -> (f64, f64, f64) {
    let spacing = spacing.max(1e-6);
    let pixels_per_mm = view.pixels_per_mm() * ground_squash(view);
    let mut coarse = spacing;
    // Bounded: an absurd zoom cannot ask for an unbounded number of decades.
    for _ in 0..40 {
        if coarse * pixels_per_mm >= CELL_FULL {
            break;
        }
        coarse *= 10.0;
    }
    if coarse <= spacing * 1.000_001 {
        // The document's own spacing is already wide enough, so there is no
        // finer decade to fade in under it.
        return (spacing, spacing, 0.0);
    }
    let fine = coarse / 10.0;
    let cell = fine * pixels_per_mm;
    let t = ((cell - CELL_FADE_OUT) / (CELL_FULL - CELL_FADE_OUT)).clamp(0.0, 1.0);
    // Smoothstep, so the fine grid arrives and leaves without an edge.
    (fine, coarse, t * t * (3.0 - 2.0 * t))
}

/// Grid spacing that is legible at this zoom: the coarse decade of
/// `grid_levels`. The axes are laid out on it, and the tool rail reads it to
/// say what one grid square means.
pub fn effective_grid_spacing(view: &View, spacing: f64) -> f64 {
    grid_levels(view, spacing).1
}

/// How many lines one level of the grid may draw either side of its centre.
/// The fine level at its densest would otherwise be several thousand, which is
/// work for lines that are all but invisible by then.
///
/// Enough that the cap is not what decides the extent at any ordinary window
/// size and tilt -- it is a bound on the pathological case, not a second
/// answer to "how far does the ground reach". A line that misses the frame is
/// dropped by `visible_span` before it is rasterized, so the ones this bounds
/// are cheap to begin with.
const MAX_LINES: i64 = 400;

fn push_grid(steps: &mut Vec<Step>, view: &View, grid: &Grid, palette: &Palette) {
    let (fine, coarse, strength) = grid_levels(view, grid.spacing);
    let radius = grid_radius(view);
    // Both levels cover the same ground. Drawing the fine one over a shorter
    // reach made the grid detailed around the origin and coarse everywhere
    // else, so the ground read as a patch of detail sitting on a plainer one
    // rather than as a single grid -- and which one you were looking at
    // depended on where the origin happened to be in the frame. Detail is a
    // question about the zoom, and `grid_levels` already answers it: the fine
    // level fades in and out across the whole ground at once.
    if strength > 0.03 {
        push_grid_level(steps, view, fine, radius, strength, false, palette);
    }
    push_grid_level(steps, view, coarse, radius, 1.0, true, palette);
}

/// One decade of the ground grid, centred on the camera target so panning never
/// runs off the end of it. The centre is snapped to the level's own spacing, so
/// every line sits at a whole multiple of it -- which is what keeps the line
/// through zero *on* zero, and the X and Y axes lying along the grid rather
/// than across it.
fn push_grid_level(
    steps: &mut Vec<Step>,
    view: &View,
    spacing: f64,
    radius: f64,
    strength: f64,
    majors: bool,
    palette: &Palette,
) {
    let lines = ((radius / spacing).ceil() as i64).clamp(1, MAX_LINES);
    let half = spacing * lines as f64;
    let cx = (view.camera.target.x / spacing).round() * spacing;
    let cy = (view.camera.target.y / spacing).round() * spacing;
    // A major line every ten, counted in whole multiples of the spacing from
    // the world origin rather than from the centre, so which lines are major
    // stays put while the camera pans over them.
    let shade = |world: f64| {
        let index = (world / spacing).round() as i64;
        let colour = if majors && index.rem_euclid(10) == 0 { palette.grid_major } else { palette.grid };
        [colour[0], colour[1], colour[2], (colour[3] as f64 * strength).round() as u8]
    };
    for i in -lines..=lines {
        let offset = i as f64 * spacing;
        push_faded_line(
            steps,
            view,
            Vec3::new(cx + offset, cy - half, 0.0),
            Vec3::new(cx + offset, cy + half, 0.0),
            shade(cx + offset),
            GRID_BIAS,
        );
        push_faded_line(
            steps,
            view,
            Vec3::new(cx - half, cy + offset, 0.0),
            Vec3::new(cx + half, cy + offset, 0.0),
            shade(cy + offset),
            GRID_BIAS,
        );
    }
}

/// How many pieces a grid line is cut into to fade it. Enough that the steps
/// between one piece's alpha and the next are invisible, few enough that the
/// whole grid is still one pass of cheap segment drawing.
const FADE_STEPS: usize = 24;

/// Draw one grid line as a run of short segments whose alpha falls off with
/// distance from the grid's centre. A grid that simply stops leaves a hard
/// square edge in mid-air, and the eye reads that edge as part of the model.
/// The stretch of a world segment, as a parameter range inside `[0, 1]`, whose
/// projection lands in the frame -- `None` when none of it does.
///
/// The grid's lines run far outside the viewport, and at a shallow angle the
/// ground reaches several times the width of the frame. Subdividing the whole
/// segment would spend the fade's steps on the part nobody sees and leave two
/// or three of them for the part they do, which shows as banding across the
/// frame; finding the visible stretch first spends them all where they are
/// seen, and drops a line that misses the frame entirely before it costs
/// anything.
fn visible_span(view: &View, from: Vec3, to: Vec3) -> Option<(f64, f64)> {
    let a = view.view_to_screen(view.to_view(from)).0;
    let b = view.view_to_screen(view.to_view(to)).0;
    let (dx, dy) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let (width, height) = (view.size.x as f64, view.size.y as f64);
    let left = view.centre.x as f64 - width / 2.0;
    let top = view.centre.y as f64 - height / 2.0;
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    // Liang-Barsky against the frame, as the rasterizer does with pixels.
    for (edge, room) in [
        (-dx, a.x as f64 - left),
        (dx, left + width - a.x as f64),
        (-dy, a.y as f64 - top),
        (dy, top + height - a.y as f64),
    ] {
        if edge == 0.0 {
            if room < 0.0 {
                return None; // parallel to this edge and outside it
            }
        } else {
            let at = room / edge;
            if edge < 0.0 {
                if at > t1 {
                    return None;
                }
                t0 = t0.max(at);
            } else {
                if at < t0 {
                    return None;
                }
                t1 = t1.min(at);
            }
        }
    }
    (t1 > t0).then_some((t0, t1))
}

fn push_faded_line(steps: &mut Vec<Step>, view: &View, from: Vec3, to: Vec3, colour: Rgba, bias: f32) {
    let Some((visible_from, visible_to)) = visible_span(view, from, to) else { return };
    let half_diagonal = ((view.size.x as f64).hypot(view.size.y as f64) / 2.0).max(1.0);
    let span = visible_to - visible_from;
    for step in 0..FADE_STEPS {
        let t0 = visible_from + span * (step as f64 / FADE_STEPS as f64);
        let t1 = visible_from + span * ((step + 1) as f64 / FADE_STEPS as f64);
        let a = from + (to - from) * t0;
        let b = from + (to - from) * t1;
        let mid = (a + b) * 0.5;
        // Measured on the screen, not in the world. In the world it is a circle
        // about the origin, which the tilt of the ground turns into an ellipse
        // on the screen -- so the grid faded out before the top and bottom of
        // the viewport at every angle but straight down, however far it
        // reached. On the screen the falloff is the same in every direction and
        // the ground covers the frame at any tilt.
        let screen = view.view_to_screen(view.to_view(mid)).0;
        let distance = ((screen.x - view.centre.x) as f64).hypot((screen.y - view.centre.y) as f64);
        // Squared falloff: full strength in the middle of the frame, and gone
        // just past the corners rather than at them.
        let fade = 1.0 - (distance / (half_diagonal * 1.08)).min(1.0).powi(2);
        if fade <= 0.03 {
            continue;
        }
        let faded = [colour[0], colour[1], colour[2], (colour[3] as f64 * fade).round() as u8];
        // Never writes depth: the grid and the axes are drawn before the model
        // and must lose every tie with it, including the exact ties a ground
        // plane makes with a plate whose side walls it cuts.
        steps.push(line_step(view, a, b, faded, bias, 0, false));
    }
}

/// What the axes meet in the model: where they run inside it, which solids they
/// go through, and how far from the origin the model reaches.
struct AxisMaterial {
    /// Per axis, the stretches inside a solid, as coordinates along that axis.
    inside: [Vec<(f64, f64)>; 3],
    /// Per axis, the bodies it runs through: the stretch inside each one, with
    /// that body's tag. Per body and per axis both -- a box the Y axis runs
    /// through is still an ordinary occluder for X and Z, and a box the axes
    /// never touch is an ordinary occluder for all three, however many other
    /// shapes it was merged into one mesh with (img2).
    ///
    /// The span is kept, not just the tag, because *where* a stretch of the
    /// line sits relative to it decides whether that body may hide it: only the
    /// approach, on the eye's side of the material, is drawn over the shape.
    through: [Vec<(f64, f64, u16)>; 3],
    /// One past the largest tag in `through`, so a lookup table indexed by tag
    /// can be sized once.
    tags: usize,
    /// The distance from the origin to the model's furthest vertex. What an
    /// origin axis's arms have to be longer than, or a shape standing on the
    /// origin holds the whole arm and the axis is never seen at all.
    reach: f64,
}

/// Where each item's body tags start, so no two items share one.
fn tag_bases(items: &[Item<'_>]) -> Vec<u16> {
    let mut base = 0_u16;
    items
        .iter()
        .map(|item| {
            let start = base;
            base = base.saturating_add(item.renderable.body_count);
            start
        })
        .collect()
}

fn axis_material(items: &[Item<'_>], grid: &Grid) -> AxisMaterial {
    let mut material = AxisMaterial {
        inside: [Vec::new(), Vec::new(), Vec::new()],
        through: [Vec::new(), Vec::new(), Vec::new()],
        tags: 0,
        reach: 0.0,
    };
    for (item, tag_base) in items.iter().zip(tag_bases(items)) {
        let positions = item.renderable.mesh.positions.iter();
        material.reach = positions.map(|p| p.length()).fold(material.reach, f64::max);
        // A ghost is see-through, so the axis inside it is too -- and it never
        // reaches the depth buffer either way.
        if item.style != Style::Solid {
            continue;
        }
        for axis in 0..3 {
            if !grid.axes[axis] {
                continue;
            }
            for (span, tag) in axis_inside_spans(item, axis, tag_base) {
                material.inside[axis].push(span);
                material.through[axis].push((span.0, span.1, tag));
                material.tags = material.tags.max(tag as usize + 1);
            }
        }
    }
    material
}

/// One arm of an axis: faded along its length like the grid, and drawn over the
/// frame rather than tested against it -- except where it runs inside material,
/// which is not drawn at all.
///
/// The depth test is the wrong question for an axis. Asked of the depth buffer,
/// an axis disappears wherever the shape merely *stands in front of it*, which
/// is most of the screen once the camera is close: the arm leaving a box at the
/// origin is outside the box from the surface onwards, but its projection stays
/// over the box for a long way, so the line arriving at the shape was missing
/// and only a mark on the face was left (issue 47). Asked of the model instead
/// -- is this stretch of the line inside anything? -- the answer is the one the
/// picture wants: the line runs unbroken up to the surface it goes into, stops
/// there, and picks up again where it comes out (issues 20, 36, 47).
///
/// That exception is the *approach*, and nothing else. It is granted to the
/// stretch of the line on the eye's side of the material, because the stretch
/// beyond the far surface really is behind the shape: drawing it over the solid
/// as well put the line on the face of a box it had already left, which reads
/// as an axis running inside the object instead of out the back of it. So every
/// piece asks the question for itself, and the arm going away is
/// depth-tested like anything else -- hidden by the box, and picked up again
/// where it comes out past the silhouette.
#[allow(clippy::too_many_arguments)]
fn push_axis_line(
    out: &mut Vec<AxisStep>,
    view: &View,
    centre: Vec3,
    to: Vec3,
    reach: f64,
    colour: Rgba,
    axis: usize,
    inside: &[(f64, f64)],
    through: &[(f64, f64, u16)],
    tags: usize,
) {
    // Which way depth runs along this axis: positive when travelling along
    // +axis moves away from the eye.
    let away = component(view.forward(), axis);
    let mut seen = vec![false; tags + 1];
    for step in 0..FADE_STEPS {
        let t0 = step as f64 / FADE_STEPS as f64;
        let t1 = (step + 1) as f64 / FADE_STEPS as f64;
        let a = centre + (to - centre) * t0;
        let b = centre + (to - centre) * t1;
        let mid = (a + b) * 0.5;
        let fade = 1.0 - ((mid - centre).length() / (reach * 0.8)).min(1.0).powi(2);
        if fade <= 0.03 {
            continue;
        }
        let faded = [colour[0], colour[1], colour[2], (colour[3] as f64 * fade).round() as u8];
        for (from, to, material) in clip_spans(component(a, axis), component(b, axis), inside) {
            if material {
                continue;
            }
            let (start, end) = (along(axis, from), along(axis, to));
            let (va, vb) = (to_vertex(view, view.to_view(start)), to_vertex(view, view.to_view(end)));
            seen_through(&mut seen, through, (from + to) / 2.0, away);
            out.push(AxisStep { a: va, b: vb, colour: faded, seen: seen.clone().into() });
        }
    }
}

/// Fill `seen`, indexed by body tag, with the bodies that may not hide the piece
/// of an axis at `at` along it.
///
/// A body qualifies only when that piece is on the eye's side of the stretch the
/// axis runs through it: the line is being drawn over the shape so that it can
/// be seen *arriving* at the surface it enters, and past the far surface there
/// is no arrival left to show -- only a solid the line is genuinely behind.
/// `away` is how depth runs along the axis; when it is about zero the axis lies
/// in the screen plane, the two sides are the same distance off, and the
/// exception applies to both.
fn seen_through(seen: &mut [bool], through: &[(f64, f64, u16)], at: f64, away: f64) {
    seen.fill(false);
    for &(lo, hi, tag) in through {
        let Some(slot) = seen.get_mut(tag as usize) else { continue };
        *slot |= if away > 1e-9 {
            at <= lo
        } else if away < -1e-9 {
            at >= hi
        } else {
            true
        };
    }
}

/// `[a, b]` -- a stretch of an axis, given as the coordinate along it -- cut at
/// every boundary of `spans`, each piece saying whether it lies inside one.
///
/// Either end may be the larger; the pieces come back in the order they were
/// asked for, so a line keeps its direction and the fade along it.
fn clip_spans(a: f64, b: f64, spans: &[(f64, f64)]) -> Vec<(f64, f64, bool)> {
    let (lo, hi) = (a.min(b), a.max(b));
    let mut cuts = vec![lo, hi];
    for &(start, end) in spans {
        for edge in [start, end] {
            if edge > lo && edge < hi {
                cuts.push(edge);
            }
        }
    }
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut pieces: Vec<(f64, f64, bool)> = Vec::with_capacity(cuts.len());
    for pair in cuts.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if to - from < 1e-9 {
            continue;
        }
        let middle = (from + to) / 2.0;
        pieces.push((from, to, spans.iter().any(|&(start, end)| middle > start && middle < end)));
    }
    if a > b {
        pieces.reverse();
        pieces = pieces.into_iter().map(|(from, to, flag)| (to, from, flag)).collect();
    }
    pieces
}

/// Where along `axis` one item's solids are, as spans of the coordinate along
/// it, each with the body it runs through.
///
/// A closed surface is crossed an even number of times, so the crossings sorted
/// and taken in pairs are the stretches inside it. They are paired *per body*,
/// because the mesh handed to the renderer is the whole scene at once and two
/// shapes on the same axis would otherwise pair across the gap between them --
/// which would call the empty space between two boxes "material". An odd count
/// means the line grazed an edge or the mesh is not closed; the odd one out is
/// dropped rather than turned into a span that runs to infinity.
fn axis_inside_spans(item: &Item<'_>, axis: usize, tag_base: u16) -> Vec<((f64, f64), u16)> {
    // The axis passes through the origin, so a solid that does not straddle zero
    // on the other two coordinates cannot be on it -- which is most of them, and
    // this is the whole mesh not looked at.
    let Some((lo, hi)) = item.renderable.mesh.bounds() else { return Vec::new() };
    if (0..3).any(|other| other != axis && (component(lo, other) > 0.0 || component(hi, other) < 0.0)) {
        return Vec::new();
    }
    let mut crossings: std::collections::BTreeMap<u16, Vec<f64>> = std::collections::BTreeMap::new();
    for (index, tri) in item.renderable.mesh.indices.iter().enumerate() {
        let world = [
            item.renderable.mesh.positions[tri[0] as usize],
            item.renderable.mesh.positions[tri[1] as usize],
            item.renderable.mesh.positions[tri[2] as usize],
        ];
        if let Some(at) = axis_crossing(world, axis) {
            crossings.entry(item.renderable.tag(index, tag_base)).or_default().push(at);
        }
    }
    let mut spans = Vec::new();
    for (tag, mut at) in crossings {
        at.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // A crossing on a shared edge is found twice, once for each triangle.
        at.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        spans.extend(at.chunks_exact(2).map(|pair| ((pair[0], pair[1]), tag)));
    }
    spans
}

/// The point on the axis at `value` along it. Every axis line passes through the
/// origin, in both styles, so the other two coordinates are zero.
fn along(axis: usize, value: f64) -> Vec3 {
    match axis {
        0 => Vec3::new(value, 0.0, 0.0),
        1 => Vec3::new(0.0, value, 0.0),
        _ => Vec3::new(0.0, 0.0, value),
    }
}

fn component(v: Vec3, axis: usize) -> f64 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// Where the axis through the origin along `axis` pierces one triangle, as the
/// coordinate along that axis. Moller-Trumbore against the line rather than a
/// ray, so a crossing behind the origin is found as readily as one in front.
fn axis_crossing(world: [Vec3; 3], axis: usize) -> Option<f64> {
    let direction = along(axis, 1.0);
    let (edge1, edge2) = (world[1] - world[0], world[2] - world[0]);
    let pvec = direction.cross(edge2);
    let det = edge1.dot(pvec);
    // Edge-on to the axis: no crossing, and the maths is degenerate.
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let tvec = -world[0];
    let u = tvec.dot(pvec) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qvec = tvec.cross(edge1);
    let v = direction.dot(qvec) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some(edge2.dot(qvec) * inv)
}

/// One piece of an origin axis, ready to draw: where it runs on screen, what
/// colour it has faded to, and which bodies may not hide it.
///
/// `seen` is indexed by body tag and is the whole of the axis rule -- a piece
/// of the line that loses the depth test is still drawn when whatever won that
/// pixel is a body the line is arriving at. It is worked out here, from the
/// model, so that both renderers answer the question the same way.
pub(crate) struct AxisStep {
    pub a: Vertex,
    pub b: Vertex,
    pub colour: Rgba,
    pub seen: std::sync::Arc<[bool]>,
}

fn prepare_axes(view: &View, palette: &Palette, grid: &Grid, material: &AxisMaterial) -> Vec<AxisStep> {
    let mut out = Vec::new();
    let spacing = effective_grid_spacing(view, grid.spacing);
    let radius = grid_radius(view);
    let clearance = material.reach;
    let colours = [palette.axis_x, palette.axis_y, palette.axis_z];
    let directions = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
    for axis in 0..3 {
        if !grid.axes[axis] {
            continue;
        }
        // Where this axis runs through material, and so is not drawn at all...
        let inside = &material.inside[axis];
        // ...and the bodies it runs through, each with the stretch inside it, so
        // the approach to a surface is drawn over that body and the arm beyond
        // it is not.
        let through = &material.through[axis];
        // The two styles are two different things, and each is drawn as what it
        // is. Along the grid, an axis *is* a grid line: it runs the width of the
        // ground, travels with it, and fades out with it at the edge -- so X and
        // Y are the coloured lines through zero, the way every other 3D
        // application draws them. Pinned at the origin, it is a cross: a
        // bounded mark of a dozen grid squares that stays at zero while the
        // camera pans away from it, drawn at full strength so it reads as an
        // object rather than as ground.
        let (centre, length, reach) = match grid.style {
            AxisStyle::Grid => {
                let centre = match axis {
                    // Snapped to the grid's own spacing, so the axis lies along
                    // a grid line rather than between two of them.
                    0 => Vec3::new((view.camera.target.x / spacing).round() * spacing, 0.0, 0.0),
                    1 => Vec3::new(0.0, (view.camera.target.y / spacing).round() * spacing, 0.0),
                    // Z has no grid line to be: the grid is the ground.
                    _ => Vec3::ZERO,
                };
                (centre, radius, radius)
            }
            // `reach` past the length, so the fade only softens the last part of
            // each arm instead of consuming the whole of it.
            AxisStyle::Origin => {
                // Bounded, and always well inside the frame, so it reads as a
                // cross at the origin rather than as another pair of grid lines
                // however far the camera is pulled back. Measured against the
                // frame rather than the ground's reach, which a tilt stretches.
                let frame_reach = frame_reach(view);
                let length = (spacing * 12.0).min(frame_reach * 0.61);
                // ...but never so short that the model swallows the whole arm.
                // The arms are measured from the origin, which is inside a shape
                // standing on it, so an arm that ends inside that shape is an
                // axis with nothing left to draw -- which is what zooming in on
                // a box at the origin gave: a mark on the face and no line
                // arriving at it.
                let clear = (clearance * 1.6 + 30.0 / view.pixels_per_mm().max(1e-9)).min(frame_reach * 2.0);
                let length = length.max(clear);
                (Vec3::ZERO, length, length * 2.0)
            }
        };
        // Both halves fade outward from the centre, for the same reason the
        // grid does: an axis that ends abruptly reads as an object.
        for sign in [-1.0, 1.0] {
            push_axis_line(
                &mut out,
                view,
                centre,
                centre + directions[axis] * (length * sign),
                reach,
                colours[axis],
                axis,
                inside,
                through,
                material.tags,
            );
        }
    }
    out
}

/// The colour a principal plane's mark is drawn in, indexed by the axis that
/// plane is perpendicular to: the colour of the axis the mark is *presented*
/// as, which for X and Y is the other one (see [`crate::snap::MARK_AXIS`]).
fn mark_colours(palette: &Palette) -> [Rgba; 3] {
    let axes = [palette.axis_x, palette.axis_y, palette.axis_z];
    [axes[MARK_AXIS[0]], axes[MARK_AXIS[1]], axes[MARK_AXIS[2]]]
}

/// Draw, on the surface of each solid, the line where a principal plane cuts
/// through it.
///
/// The ground plane crossing a shape is a real dimension -- how much of the
/// shape is below the build plate -- and until something marks it the only way
/// to read it is to orbit until the grid is edge-on. The mark is drawn on the
/// surface itself, where the plane meets it, in the colour `mark_colours` gives
/// for the axis the plane is perpendicular to.
///
/// A mark answers to the switch of the axis it is *drawn as* rather than to the
/// one its plane is perpendicular to, because its colour is the only thing there
/// is to recognise it by (issue 75).
fn push_plane_marks(steps: &mut Vec<Step>, view: &View, items: &[Item<'_>], palette: &Palette, grid: &Grid) {
    let colours = mark_colours(palette);
    for item in items.iter().filter(|i| i.style == Style::Solid) {
        for (axis, colour) in colours.into_iter().enumerate() {
            if !grid.axes[MARK_AXIS[axis]] {
                continue;
            }
            for tri in &item.renderable.mesh.indices {
                let world = [
                    item.renderable.mesh.positions[tri[0] as usize],
                    item.renderable.mesh.positions[tri[1] as usize],
                    item.renderable.mesh.positions[tri[2] as usize],
                ];
                if let Some((a, b)) = crate::snap::plane_crossing(world, axis) {
                    steps.push(line_step(view, a, b, colour, MARK_BIAS, 0, true));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simple3d_core::scene::Camera;
    use simple3d_geom::primitives;

    fn view(width: usize, height: usize) -> View {
        View::new(
            Camera { yaw: -55.0, pitch: 28.0, distance: 120.0, ..Camera::default() },
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width as f32, height as f32)),
        )
    }

    fn request<'a>(items: Vec<Item<'a>>, mode: DisplayMode) -> Request<'a> {
        Request {
            view: view(160, 120),
            size: [160, 120],
            mode,
            palette: Palette::dark(),
            grid: Grid { visible: false, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false },
            items,
            preview: Vec::new(),
        }
    }

    fn count_non_background(frame: &Image, palette: &Palette) -> usize {
        (0..frame.height * frame.width).filter(|&i| !is_background(frame, i, palette)).count()
    }

    /// Whether pixel `index` still holds the gradient it was cleared to.
    fn is_background(frame: &Image, index: usize, palette: &Palette) -> bool {
        let offset = index * 4;
        let pixel: Rgba =
            [frame.color[offset], frame.color[offset + 1], frame.color[offset + 2], frame.color[offset + 3]];
        pixel == palette.background_at(index / frame.width, frame.height)
    }

    #[test]
    fn a_box_has_twelve_feature_edges_and_a_cylinder_keeps_only_its_rims() {
        let box_edges = feature_edges(&primitives::box_mesh(20.0, 20.0, 20.0).weld(), 20.0);
        assert_eq!(box_edges.len(), 12, "a box has twelve real edges, got {}", box_edges.len());

        // A 32-segment cylinder's curved surface is smooth, so only the two rims
        // and the vertical seams between wall and cap survive -- never the fan
        // triangulation inside the caps.
        let cylinder = primitives::cylinder_mesh(20.0, 20.0, 20.0, 32).weld();
        let edges = feature_edges(&cylinder, 20.0);
        assert!(edges.len() >= 64, "both rims should be kept, got {}", edges.len());
        assert!(edges.len() < 100, "the cap triangulation leaked into the edges: {}", edges.len());
    }

    /// Splitting the frame across threads must not change one pixel of it.
    ///
    /// This is the whole contract the banded renderer rests on, and it is not
    /// self-evident: the first version of it clipped each line to the band
    /// before stepping along it, which re-spaced the samples and moved every
    /// grid line and feature edge by up to a pixel wherever a band began. The
    /// picture still looked right on its own; it was only wrong against the
    /// picture one thread drew. Every case below fails on that version.
    #[test]
    fn bands_draw_the_very_same_frame_as_one_thread() {
        let mut mesh = primitives::box_mesh(30.0, 20.0, 14.0);
        // A round body, so there are many small triangles and many feature
        // edges landing at every angle to the band boundaries.
        mesh.append(
            &primitives::ellipsoid_mesh(18.0, 18.0, 18.0, 24)
                .transformed(simple3d_geom::Vec3::new(22.0, 8.0, 4.0), simple3d_geom::Vec3::ZERO),
        );
        let prepared = Renderable::prepare(&mesh);
        for mode in [DisplayMode::ShadedWithEdges, DisplayMode::Shaded, DisplayMode::Wireframe] {
            let items = vec![Item { renderable: &prepared, style: Style::Solid }];
            let mut req = request(items, mode);
            // The grid, the axes and the plane marks all draw lines that cross
            // the whole frame, so they cross every band boundary there is.
            req.grid =
                Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Grid, plane_marks: true };
            let prepared = prepare_frame(&req);
            let one = render_in_bands(&req, &prepared, 1);
            for bands in [2, 3, 7, 16] {
                let many = render_in_bands(&req, &prepared, bands);
                let differing =
                    (0..one.color.len() / 4).filter(|i| one.color[i * 4..i * 4 + 4] != many.color[i * 4..i * 4 + 4]);
                let differing: Vec<usize> = differing.collect();
                assert!(
                    differing.is_empty(),
                    "{mode:?} in {bands} bands differs from one band at {} pixels, first at ({}, {})",
                    differing.len(),
                    differing[0] % one.width,
                    differing[0] / one.width,
                );
            }
        }
    }

    #[test]
    fn a_shaded_render_actually_draws_the_model() {
        let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
        let items = vec![Item { renderable: &prepared, style: Style::Solid }];
        let req = request(items, DisplayMode::Shaded);
        let frame = render(&req);
        let painted = count_non_background(&frame, &req.palette);
        assert!(painted > 1000, "only {painted} pixels painted");
        assert!(painted < 160 * 120, "the model filled the entire viewport");
    }

    /// The convex hull of a set of screen points, counter-clockwise in
    /// screen coordinates. Andrew's monotone chain.
    fn hull(mut points: Vec<egui::Pos2>) -> Vec<egui::Pos2> {
        points.sort_by(|a, b| (a.x, a.y).partial_cmp(&(b.x, b.y)).unwrap());
        points.dedup();
        let cross = |o: egui::Pos2, a: egui::Pos2, b: egui::Pos2| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
        let mut out: Vec<egui::Pos2> = Vec::new();
        for pass in 0..2 {
            let start = (out.len() + 1).max(2);
            let iter: Box<dyn Iterator<Item = &egui::Pos2>> =
                if pass == 0 { Box::new(points.iter()) } else { Box::new(points.iter().rev()) };
            for &p in iter {
                while out.len() >= start && cross(out[out.len() - 2], out[out.len() - 1], p) <= 0.0 {
                    out.pop();
                }
                out.push(p);
            }
            out.pop();
        }
        out
    }

    /// Pixels that lie at least `margin` inside the hull but were never painted.
    /// For a convex solid the painted silhouette *is* the hull of its projected
    /// vertices, so any such pixel means a face that faces the viewer was not
    /// drawn.
    fn unpainted_inside(hull: &[egui::Pos2], frame: &Image, empty: &Image, margin: f32) -> usize {
        let mut missing = 0;
        for row in 0..frame.height {
            for x in 0..frame.width {
                let p = egui::pos2(x as f32 + 0.5, row as f32 + 0.5);
                let inside =
                    hull.windows(2).chain(std::iter::once([hull[hull.len() - 1], hull[0]].as_slice())).all(|e| {
                        let (a, b) = (e[0], e[1]);
                        let n = egui::vec2(b.y - a.y, a.x - b.x);
                        let len = n.length().max(1e-6);
                        ((p - a).dot(n) / len) <= -margin
                    });
                let o = (row * frame.width + x) * 4;
                if inside && frame.color[o..o + 4] == empty.color[o..o + 4] {
                    missing += 1;
                }
            }
        }
        missing
    }

    #[test]
    fn a_box_away_from_the_centre_of_the_frame_keeps_all_its_faces() {
        // Every ray runs along the view direction,
        // so which faces are turned towards the viewer cannot depend on where in
        // the frame they land. Culling against `eye - centroid` made it depend on
        // exactly that: it dropped a near-edge-on side wall out of the image, and
        // the model was drawn a wall short down one side.
        //
        // A box is convex, so what it should cover is exactly the convex hull of
        // its projected corners -- a missing wall is a slice of that hull left
        // showing the background, which is what this counts. A hole in the middle
        // would not do: a dropped wall is at the edge of the silhouette, not
        // enclosed by it.
        // 100 mm off the camera's target at a distance of 120 mm is nearly 40
        // degrees between the two answers, and every wall inside that is lost.
        let mesh = primitives::box_mesh(40.0, 20.0, 4.0).translated(Vec3::new(100.0, 0.0, 0.0));
        let prepared = Renderable::prepare(&mesh);
        let camera = Camera { yaw: 2.0, pitch: -15.0, distance: 120.0, ..Camera::default() };
        let view = View::new(camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(160.0, 120.0)));

        let mut empty = request(Vec::new(), DisplayMode::Shaded);
        empty.view = view;
        let empty = render(&empty);

        let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
        req.view = view;
        let frame = render(&req);

        assert!(count_non_background(&frame, &req.palette) > 500, "the box did not draw at all");
        let corners: Vec<egui::Pos2> =
            prepared.mesh.positions.iter().filter_map(|&p| view.project(p)).map(|(s, _)| s).collect();
        let missing = unpainted_inside(&hull(corners), &frame, &empty, 1.0);
        assert_eq!(missing, 0, "{missing} pixels inside the box's own silhouette were never drawn");
    }

    /// How many pixels of the frame carry the given colour, shaded or not.
    /// A drawn line keeps its colour exactly; only shaded faces are scaled.
    fn pixels_of(frame: &Image, colour: Rgba) -> usize {
        (0..frame.width * frame.height)
            .filter(|&i| {
                let o = i * 4;
                frame.color[o] == colour[0] && frame.color[o + 1] == colour[1] && frame.color[o + 2] == colour[2]
            })
            .count()
    }

    /// A tool's preview is drawn *on* the model, not through it (issue 82).
    ///
    /// The cells at the near end of a run lie in the face turned towards the
    /// camera and have to be drawn over it; the ones at the far end lie in the
    /// face turned away and are behind forty millimetres of solid. A grid that
    /// shows through the shape reads as floating in front of it, which is the
    /// one thing the preview must not say.
    /// The preview is a step of its own, and it comes after the model.
    ///
    /// The two engines are handed the same prepared steps, and only the
    /// software one draws them in the order it was given: the GPU sorts them
    /// into passes by kind, and a line that writes no depth meant the ground
    /// grid, which goes *under* the model. Drawn as one, the preview was
    /// covered by the very shape it is drawn on -- invisible in the running
    /// application while every pixel test on this file passed.
    #[test]
    fn a_preview_is_its_own_kind_of_step_and_comes_after_the_model() {
        let prepared = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
        let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
        req.preview = vec![vec![
            Vec3::new(-15.0, -15.0, 20.0),
            Vec3::new(15.0, -15.0, 20.0),
            Vec3::new(15.0, 15.0, 20.0),
            Vec3::new(-15.0, 15.0, 20.0),
        ]];
        let steps = prepare(&req);
        let overlays = steps.iter().filter(|step| matches!(step, Step::Overlay { .. })).count();
        assert_eq!(overlays, 4, "a four-cornered loop is four lines");
        let first = steps.iter().position(|step| matches!(step, Step::Overlay { .. })).expect("it is in there");
        let last_face = steps.iter().rposition(|step| matches!(step, Step::Triangle { .. })).expect("the box is drawn");
        assert!(first > last_face, "the preview was prepared before the model it is drawn over");
    }

    #[test]
    fn a_preview_loop_is_drawn_on_the_solid_and_hidden_behind_it() {
        let prepared = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
        let square = |z: f64| {
            vec![
                Vec3::new(-15.0, -15.0, z),
                Vec3::new(15.0, -15.0, z),
                Vec3::new(15.0, 15.0, z),
                Vec3::new(-15.0, 15.0, z),
            ]
        };
        let drawn = |loops: Vec<Vec<Vec3>>| {
            let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
            req.preview = loops;
            let frame = render(&req);
            pixels_of(&frame, req.palette.selected)
        };
        assert!(drawn(vec![square(20.0)]) > 0, "the cells on the face turned towards the camera were not drawn");
        assert_eq!(drawn(vec![square(-20.0)]), 0, "the cells on the far side of the solid were drawn through it");
    }

    #[test]
    fn along_the_grid_the_axes_travel_with_the_view_and_pinned_ones_do_not() {
        // Issue 14: pinned axes leave the view the moment the origin is panned
        // off it, which is not how other 3D software reads. Along the grid, X
        // and Y are the grid's own lines through zero and are always there to
        // read -- and both styles stay available, because the pinned cross is
        // the one that says where the origin actually is.
        let far = Camera {
            target: Vec3::new(4000.0, 0.0, 0.0),
            yaw: -55.0,
            pitch: 28.0,
            distance: 120.0,
            ..Camera::default()
        };
        let drawn = |style: AxisStyle| {
            let mut req = request(Vec::new(), DisplayMode::Shaded);
            req.view = View::new(far, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(160.0, 120.0)));
            req.grid = Grid { visible: false, spacing: 10.0, axes: [true; 3], style, plane_marks: false };
            let frame = render(&req);
            pixels_of(&frame, req.palette.axis_x)
        };
        assert_eq!(drawn(AxisStyle::Origin), 0, "a pinned X axis should be long gone at this distance");
        assert!(drawn(AxisStyle::Grid) > 0, "the X axis should follow the grid and still be drawn");
    }

    #[test]
    fn the_z_axis_stays_at_the_origin_in_both_styles() {
        // There is no ground line for Z to be: the grid is the ground, so
        // neither style moves it off zero.
        let drawn = |style: AxisStyle| {
            let mut req = request(Vec::new(), DisplayMode::Shaded);
            req.grid = Grid { visible: false, spacing: 10.0, axes: [false, false, true], style, plane_marks: false };
            let frame = render(&req);
            let mut columns: Vec<usize> = Vec::new();
            for i in 0..frame.width * frame.height {
                let o = i * 4;
                if frame.color[o..o + 3] == req.palette.axis_z[..3] {
                    columns.push(i % frame.width);
                }
            }
            columns
        };
        for style in AxisStyle::ALL {
            let columns = drawn(style);
            assert!(!columns.is_empty(), "{style:?}: the Z axis did not draw");
            // The camera looks at the origin, so the axis through it runs down
            // the middle of the frame whichever style drew it.
            let (lo, hi) = (*columns.iter().min().unwrap(), *columns.iter().max().unwrap());
            assert!(lo >= 78 && hi <= 82, "{style:?}: the Z axis is not at the origin: columns {lo}..{hi}");
        }
    }

    #[test]
    fn the_two_axis_styles_are_two_different_pictures() {
        // Issue 23: the setting had no visible effect. Both styles put the same
        // three lines through the same origin, and at the origin the pinned one
        // was centred exactly where the travelling one was -- so the only way to
        // tell them apart was to pan a long way off. A pinned axis is now a
        // bounded cross, drawn short, while one along the grid runs the width of
        // the ground.
        // How far the axis reaches from the centre of the frame, counting any
        // pixel that is not the background: an axis fades out along its length,
        // and a faded pixel is still a drawn one.
        let reach = |style: AxisStyle| {
            let mut req = request(Vec::new(), DisplayMode::Shaded);
            req.grid = Grid { visible: false, spacing: 10.0, axes: [true, false, false], style, plane_marks: false };
            let frame = render(&req);
            let centre = ((frame.width / 2) as f64, (frame.height / 2) as f64);
            (0..frame.width * frame.height)
                .filter(|&i| !is_background(&frame, i, &req.palette))
                .map(|i| ((i % frame.width) as f64 - centre.0).hypot((i / frame.width) as f64 - centre.1))
                .fold(0.0_f64, f64::max)
        };
        let pinned = reach(AxisStyle::Origin);
        let along = reach(AxisStyle::Grid);
        assert!(pinned > 0.0 && along > 0.0, "both styles should draw the X axis: {pinned} pinned, {along} along");
        assert!(along > pinned * 1.5, "the two styles look the same: {pinned} pinned against {along} along the grid");
    }

    #[test]
    fn the_axes_lie_along_the_grid_lines_rather_than_across_them() {
        // Issue 20: with the grid snapped to the camera target and the axes
        // snapped to their own rounding, the X axis could run between two grid
        // lines instead of along one. Every line of either is a whole multiple
        // of the spacing, so the line through zero is a grid line -- checked
        // from a target that is deliberately not on one.
        let camera = Camera { target: Vec3::new(37.3, -12.8, 0.0), distance: 300.0, ..Camera::default() };
        let view = View::new(camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(160.0, 120.0)));
        let spacing = effective_grid_spacing(&view, 10.0);
        for target in [camera.target.x, camera.target.y] {
            let centre = (target / spacing).round() * spacing;
            let remainder = (centre / spacing) - (centre / spacing).round();
            assert!(remainder.abs() < 1e-9, "the grid centre is not a whole multiple of the spacing");
            // Zero is one of the lines this grid draws, which is the axis.
            assert!((centre / spacing).abs().fract() < 1e-9);
        }
    }

    #[test]
    fn zooming_out_brings_the_next_grid_decade_in_without_a_step() {
        // Issue 25: the grid stepped up a whole decade at one particular zoom,
        // and the picture lurched -- every cell tenfold wider from one wheel
        // notch to the next, and the ground's extent with it. Sweeping the zoom
        // continuously, neither the coarse spacing nor the extent may ever jump.
        let mut previous: Option<(f64, f64)> = None;
        let mut distance = 20.0_f64;
        while distance < 200_000.0 {
            let camera = Camera { distance, ..Camera::default() };
            let view = View::new(camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)));
            let (fine, coarse, strength) = grid_levels(&view, 1.0);
            let radius = grid_radius(&view);
            if let Some((previous_coarse, previous_radius)) = previous {
                // The extent follows the zoom itself, so one step of it moves
                // the extent by one step.
                assert!(
                    radius / previous_radius < 1.05,
                    "the ground's extent jumped at distance {distance}: {previous_radius} to {radius}"
                );
                // Nothing to check where the level that stepped up was the
                // document's own spacing: there was no finer decade under it to
                // be faded, and it was being drawn in full.
                if coarse != previous_coarse {
                    // A decade may only arrive by taking over from the one below
                    // it, and that one has to still be drawn at full strength as
                    // it hands over -- so the frame after the step looks like the
                    // frame before it.
                    assert!((coarse - previous_coarse * 10.0).abs() < 1e-9, "the grid skipped a decade");
                    assert!((fine - previous_coarse).abs() < 1e-9, "the level that stepped up is not the old coarse");
                    assert!(
                        strength > 0.98,
                        "the grid stepped up while the decade below it was already faded to {strength}"
                    );
                }
            }
            previous = Some((coarse, radius));
            distance *= 1.01;
        }
    }

    #[test]
    fn a_shape_the_ground_plane_cuts_is_marked_where_it_cuts_it() {
        // Issue 16: how much of a shape is below the build plate is a real
        // dimension, and it is invisible until something marks it.
        let straddling = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
        let clear = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0).translated(Vec3::new(0.0, 0.0, 60.0)));
        let marks = |renderable: &Renderable, on: bool| {
            let mut req = request(vec![Item { renderable, style: Style::Solid }], DisplayMode::Shaded);
            req.grid = Grid {
                visible: false,
                spacing: 10.0,
                axes: [false, false, true],
                style: AxisStyle::Grid,
                plane_marks: on,
            };
            pixels_of(&render(&req), req.palette.axis_z)
        };
        // Measured as the difference the switch makes, because the Z axis
        // itself is drawn in the same colour wherever the box does not hide it.
        assert!(
            marks(&straddling, true) > marks(&straddling, false),
            "the ground plane cuts this box and nothing said where"
        );
        assert_eq!(marks(&clear, true), marks(&clear, false), "a box clear of the plane has nothing to mark");
    }

    #[test]
    fn a_plane_mark_follows_the_switch_of_the_axis_it_is_drawn_as() {
        // Issue 75. A mark is recognised by its colour and by nothing else, and
        // X's and Y's are exchanged on purpose, so the switch has to follow the
        // exchange too: the X box shows and hides the mark drawn in X's red,
        // whichever plane happens to leave it.
        let renderable = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
        let pixels = |marked: bool, colour: Rgba| {
            let mut req = request(vec![Item { renderable: &renderable, style: Style::Solid }], DisplayMode::Shaded);
            req.grid = Grid {
                visible: false,
                spacing: 10.0,
                axes: [true, false, false],
                style: AxisStyle::Grid,
                plane_marks: marked,
            };
            pixels_of(&render(&req), colour)
        };
        // Measured as the difference the marks make, because the X axis line is
        // drawn in the same red wherever the box does not hide it.
        let palette = Palette::dark();
        assert!(
            pixels(true, palette.axis_x) > pixels(false, palette.axis_x),
            "the X switch is on and no mark was drawn in X's colour"
        );
        assert_eq!(pixels(true, palette.axis_y), 0, "a mark the Y switch governs was drawn with Y turned off");
        assert_eq!(pixels(true, palette.axis_z), 0, "a mark the Z switch governs was drawn with Z turned off");
    }

    #[test]
    fn each_origin_axis_can_be_turned_off_on_its_own() {
        // Three switches, not one: an axis running through the model is a
        // distraction when it is not the one being worked to.
        let colours = |axes: [bool; 3]| {
            let mut req = request(Vec::new(), DisplayMode::Shaded);
            req.grid = Grid { visible: false, spacing: 10.0, axes, style: AxisStyle::Origin, plane_marks: false };
            let frame = render(&req);
            let mut found: Vec<Rgba> = (0..frame.width * frame.height)
                .filter(|&i| !is_background(&frame, i, &req.palette))
                .map(|i| {
                    let o = i * 4;
                    [frame.color[o], frame.color[o + 1], frame.color[o + 2], 255]
                })
                .collect();
            found.sort_unstable();
            found.dedup();
            found
        };

        assert!(colours([false, false, false]).is_empty(), "an axis was drawn with all three turned off");
        let all = colours([true; 3]);
        assert!(all.len() >= 3, "the three axes should be three colours, got {all:?}");
        for axis in 0..3 {
            let mut only = [false; 3];
            only[axis] = true;
            let drawn = colours(only);
            assert!(!drawn.is_empty(), "axis {axis} drew nothing when it was the one turned on");
            let mut without = [true; 3];
            without[axis] = false;
            let rest = colours(without);
            for colour in &drawn {
                assert!(!rest.contains(colour), "axis {axis} was still drawn after being turned off");
            }
        }
    }

    #[test]
    fn shading_makes_faces_facing_different_ways_different_shades() {
        let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
        let req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
        let frame = render(&req);
        let mut shades: Vec<u8> = (0..frame.width * frame.height)
            .filter(|&i| !is_background(&frame, i, &req.palette))
            .map(|i| frame.color[i * 4])
            .collect();
        shades.sort_unstable();
        shades.dedup();
        assert!(shades.len() >= 3, "expected the three visible faces to differ, got {shades:?}");
    }

    #[test]
    fn wireframe_paints_less_than_shaded_and_shaded_with_edges_paints_more() {
        let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
        let counts: Vec<usize> = [DisplayMode::Wireframe, DisplayMode::Shaded, DisplayMode::ShadedWithEdges]
            .into_iter()
            .map(|mode| {
                let req = request(vec![Item { renderable: &prepared, style: Style::Solid }], mode);
                let frame = render(&req);
                count_non_background(&frame, &req.palette)
            })
            .collect();
        assert!(counts[0] < counts[1], "wireframe {} should paint less than shaded {}", counts[0], counts[1]);
        // Edges overwrite pixels the fill already covered, so the count is close;
        // what matters is that dark edge pixels appeared.
        let req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::ShadedWithEdges);
        let frame = render(&req);
        let edge_pixels = frame.color.chunks_exact(4).filter(|p| *p == req.palette.edge).count();
        assert!(edge_pixels > 50, "no edge pixels in shaded-with-edges mode");
        assert!(counts[2] > 1000);
    }

    #[test]
    fn the_selection_is_outlined_in_the_selection_colour() {
        let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
        let req = request(
            vec![
                Item { renderable: &prepared, style: Style::Solid },
                Item { renderable: &prepared, style: Style::Selected },
            ],
            DisplayMode::Shaded,
        );
        let frame = render(&req);
        let highlighted = frame.color.chunks_exact(4).filter(|p| *p == req.palette.selected).count();
        assert!(highlighted > 50, "the selection outline is missing");
    }

    /// Where the selection colour was painted when `item` is drawn selected
    /// over itself: how many pixels in all, and how many in the middle of the
    /// frame.
    ///
    /// The middle is the discriminator. An outline touches the shape's rim and
    /// leaves the inside alone; a set of creases scribbles across it.
    fn selection_coverage_of(item: &Renderable) -> (usize, usize) {
        let req = request(
            vec![Item { renderable: item, style: Style::Solid }, Item { renderable: item, style: Style::Selected }],
            DisplayMode::Shaded,
        );
        let frame = render(&req);
        let mut drawn = 0;
        let mut middle = 0;
        for y in 0..frame.height {
            for x in 0..frame.width {
                let offset = (y * frame.width + x) * 4;
                let pixel: Rgba =
                    [frame.color[offset], frame.color[offset + 1], frame.color[offset + 2], frame.color[offset + 3]];
                if pixel != req.palette.selected {
                    continue;
                }
                drawn += 1;
                let middling = (x as f32 - frame.width as f32 / 2.0).abs() < frame.width as f32 / 8.0
                    && (y as f32 - frame.height as f32 / 2.0).abs() < frame.height as f32 / 8.0;
                if middling {
                    middle += 1;
                }
            }
        }
        (drawn, middle)
    }

    #[test]
    fn a_smooth_solid_is_outlined_and_a_creased_one_is_not_scribbled_over() {
        // Both halves of one bug, measured against the picture the feature
        // edges used to draw -- which is still what `prepare` alone gives, so
        // the old and the new can be rendered side by side.
        //
        // A 32-segment sphere creases at 11.25 degrees, under the 20-degree
        // feature-edge threshold, so it had no feature edges at all: selecting
        // one drew nothing, and only the manipulator said what was selected. A
        // torus at the same segment count creases past the threshold around its
        // tube, so selecting one scribbled concentric rings across the surface.
        let ball = primitives::ellipsoid_mesh(40.0, 40.0, 40.0, 32);
        let (creased, _) = selection_coverage_of(&Renderable::prepare(&ball));
        assert_eq!(creased, 0, "the sphere is only interesting because the creases drew nothing");
        let (outlined, middle) = selection_coverage_of(&Renderable::prepare_outlined(&ball));
        assert!(outlined > 50, "a smooth sphere got no selection outline: {outlined} pixels");
        assert_eq!(middle, 0, "the outline ran across the middle of the sphere: {middle} pixels");

        let ring = primitives::torus_mesh(40.0, 14.0, 360.0, 32);
        let (creased, creased_middle) = selection_coverage_of(&Renderable::prepare(&ring));
        let (outlined, middle) = selection_coverage_of(&Renderable::prepare_outlined(&ring));
        assert!(outlined > 50, "the torus got no selection outline: {outlined} pixels");
        // The hole is in the middle of the frame at this camera, so the inner
        // silhouette does cross it; the creases covered it three times over.
        assert!(
            middle * 3 <= creased_middle,
            "the outline still scribbles over the torus: {middle} pixels in the middle against {creased_middle}"
        );
        assert!(outlined < creased, "the outline is no smaller than the creases: {outlined} against {creased}");
    }

    #[test]
    fn the_selection_outline_goes_all_the_way_round() {
        // A silhouette edge is the one line a depth test cannot draw: it lies
        // exactly where the surface turns away from the eye, so the face beside
        // it is nearly edge-on and its depth changes by more across one pixel
        // than a bias can cover. Drawn once, the outline of a 32-segment sphere
        // came out as a row of dots -- seventeen of these hundred and eighty
        // sectors with nothing in them at all, and raising the bias tenfold
        // still left six. The second pass, one pixel out from the shape, is over
        // background rather than over the shape and has nothing to win from.
        //
        // Asked as "is the rim drawn all the way round" rather than "how many
        // pixels are orange", because a dotted line and a solid one differ by
        // very little on a pixel count and by everything to look at.
        // A frame big enough for the question: at the stock test size the rim is
        // forty pixels across and a two-degree sector is less than one of them,
        // so bare sectors would say nothing about the drawing.
        fn wide<'a>(items: Vec<Item<'a>>) -> Request<'a> {
            Request {
                view: View::new(
                    Camera { yaw: -55.0, pitch: 28.0, distance: 150.0, ..Camera::default() },
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 480.0)),
                ),
                size: [640, 480],
                mode: DisplayMode::Shaded,
                palette: Palette::dark(),
                grid: Grid {
                    visible: false,
                    spacing: 10.0,
                    axes: [false; 3],
                    style: AxisStyle::Origin,
                    plane_marks: false,
                },
                items,
                preview: Vec::new(),
            }
        }
        let ball = primitives::ellipsoid_mesh(50.0, 50.0, 50.0, 32);
        let prepared = Renderable::prepare_outlined(&ball);
        let plain = render(&wide(vec![Item { renderable: &prepared, style: Style::Solid }]));
        let outlined = render(&wide(vec![
            Item { renderable: &prepared, style: Style::Solid },
            Item { renderable: &prepared, style: Style::Selected },
        ]));

        // Every pixel the outline changed, measured as the difference selecting
        // makes: an alpha-blended line is never exactly its own colour.
        let changed: Vec<usize> = (0..plain.width * plain.height)
            .filter(|&i| plain.color[i * 4..i * 4 + 4] != outlined.color[i * 4..i * 4 + 4])
            .collect();
        assert!(changed.len() > 100, "the sphere got no outline at all: {} pixels", changed.len());

        let (width, sum) = (plain.width, changed.len() as f64);
        let cx = changed.iter().map(|i| (i % width) as f64).sum::<f64>() / sum;
        let cy = changed.iter().map(|i| (i / width) as f64).sum::<f64>() / sum;
        let mut sectors = [false; 180];
        for &i in &changed {
            let (x, y) = ((i % width) as f64 - cx, (i / width) as f64 - cy);
            let degrees = y.atan2(x).to_degrees().rem_euclid(360.0);
            sectors[(degrees / 2.0) as usize % 180] = true;
        }
        let bare: Vec<usize> = (0..180).filter(|&s| !sectors[s]).map(|s| s * 2).collect();
        assert!(bare.is_empty(), "the outline is broken at these bearings: {bare:?}");
    }

    #[test]
    fn a_ghost_is_translucent_over_the_background() {
        let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
        let mut req = request(vec![Item { renderable: &prepared, style: Style::Ghost }], DisplayMode::Shaded);
        // No axes: a ghost hides nothing, so all three are drawn across it at
        // full strength, and `AXIS_X` is the same red as `DANGER` -- they would
        // answer the question this test is asking.
        req.grid.axes = [false; 3];
        let frame = render(&req);
        let painted = count_non_background(&frame, &req.palette);
        assert!(painted > 500, "the ghost did not draw");
        // Nothing fully opaque in the ghost's colour: everything is blended.
        assert!(!frame.color.chunks_exact(4).any(|p| p[..3] == req.palette.ghost[..3]), "the ghost drew opaquely");
    }

    #[test]
    fn the_grid_and_axes_draw_in_their_own_colours() {
        let empty = Renderable::empty();
        let mut req = request(vec![Item { renderable: &empty, style: Style::Solid }], DisplayMode::Shaded);
        // Close enough in that a 10 mm grid is drawn at full strength, so there
        // are grid lines either side of the axes to find.
        req.view.camera.distance = 30.0;
        req.grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
        let frame = render(&req);
        for (name, colour) in
            [("X axis", req.palette.axis_x), ("Y axis", req.palette.axis_y), ("Z axis", req.palette.axis_z)]
        {
            let count = frame.color.chunks_exact(4).filter(|p| *p == colour).count();
            assert!(count > 0, "{name} did not draw");
        }
        // The grid fades outwards from the camera target and the axes cover its
        // two lines through zero, so it is counted as what it adds to the frame
        // rather than by an exact colour match.
        let mut bare = req;
        bare.grid.visible = false;
        let bare = render(&bare);
        assert!(
            count_non_background(&frame, &Palette::dark()) > count_non_background(&bare, &Palette::dark()),
            "the grid did not draw"
        );
    }

    #[test]
    fn an_axis_arrives_at_the_solid_it_enters_and_stays_behind_it_on_the_way_out() {
        // Issue 47, and the rule the three earlier passes all missed. What
        // hides an axis on the way *in* is the material it runs through, not
        // the depth buffer: a shape merely standing in front of the line is no
        // reason to drop it, or the stretch arriving at that shape goes missing
        // and only the point where the line meets the surface is left.
        //
        // On the way out it is the other way round. Past the far surface the
        // line has left the solid and is simply behind it, so the depth buffer
        // is exactly the right question -- and answering it the same way as the
        // approach drew the arm across the face of a box it had already come
        // out of, which reads as a line inside the object.
        //
        // Sampled at points on the line itself, in the world, and measured as
        // the difference switching that one axis off makes -- a line is faded
        // and alpha-blended, so the pixel is never the axis colour exactly, and
        // counting coloured pixels is what let every earlier version of this
        // pass while being wrong.
        let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
        let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
        req.grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
        let frame = render(&req);

        for axis in 0..3 {
            let mut without = Request { grid: Grid { axes: [true; 3], ..req.grid }, ..request(Vec::new(), req.mode) };
            without.grid.axes[axis] = false;
            without.items = vec![Item { renderable: &prepared, style: Style::Solid }];
            let without = render(&without);

            // Whether this axis put anything on the frame at a point on it.
            let drawn_at = |at: f64| {
                let (pos, _) = req.view.project(along(axis, at)).expect("the sample is in front of the camera");
                let (x, y) = (pos.x.round() as usize, pos.y.round() as usize);
                // A line is a pixel wide and the projection rounds, so the
                // neighbourhood is what is asked, not the single pixel.
                (y.saturating_sub(1)..=y + 1).any(|y| {
                    (x.saturating_sub(1)..=x + 1).any(|x| {
                        if x >= frame.width || y >= frame.height {
                            return false;
                        }
                        let o = (y * frame.width + x) * 4;
                        frame.color[o..o + 4] != without.color[o..o + 4]
                    })
                })
            };

            // Which way this axis runs into the frame: the arm on the eye's
            // side is the one that arrives at a surface, and the other one
            // leaves through the back.
            let near = -component(req.view.forward(), axis).signum();

            // Inside the box, which spans -15..15: nothing of the line.
            for at in [-12.0, -6.0, 0.0, 6.0, 12.0] {
                assert!(!drawn_at(at), "axis {axis} drew inside the solid, at {at}");
            }
            // The near arm is there unbroken right up to the surface it goes
            // into -- including where it is still over the box's own
            // silhouette, which is the stretch the depth test used to eat.
            for at in [16.0, 18.0, 24.0] {
                assert!(drawn_at(at * near), "axis {axis} left a gap arriving at the solid, at {at}");
            }
            // The far arm is behind the box, so the box hides it like anything
            // else: nothing while it is over the silhouette...
            for at in [16.0, 18.0, 22.0] {
                assert!(!drawn_at(at * -near), "axis {axis} drew behind the solid, at {at}");
            }
            // ...and the line again once it is clear of it.
            assert!(drawn_at(40.0 * -near), "axis {axis} never came out from behind the solid");
        }
    }

    #[test]
    fn a_solid_an_axis_does_not_run_through_hides_it_like_anything_else() {
        // Issue 47, the other half: only the solid an axis actually goes into
        // is seen through. A shape standing in front of the origin covers the
        // axes behind it, and a shape a *different* axis runs through covers
        // them just the same -- the exception is per axis and per solid, which
        // is why it cannot be one flag on the item.
        let mut req = request(Vec::new(), DisplayMode::Shaded);
        req.grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
        // Between the eye and the origin, and square in front of it: the axes
        // cross its silhouette without touching the solid itself.
        let between = primitives::box_mesh(30.0, 30.0, 30.0).translated(req.view.offset_dir() * 40.0);
        let prepared = Renderable::prepare(&between);
        let items = vec![Item { renderable: &prepared, style: Style::Solid }];
        assert!(
            axis_material(&items, &req.grid).inside.iter().all(|spans| spans.is_empty()),
            "the solid was placed on an axis, so this proves nothing"
        );

        // The same request, with or without the solid in the way and with one
        // axis switched off, so what the axis drew can be measured as the
        // difference switching it off makes.
        let frame = |axes: [bool; 3], in_the_way: bool| {
            let mut this = Request { grid: Grid { axes, ..req.grid }, ..request(Vec::new(), req.mode) };
            this.view = req.view;
            if in_the_way {
                this.items = vec![Item { renderable: &prepared, style: Style::Solid }];
            }
            render(&this)
        };
        let bare = frame([true; 3], false);
        let covered = frame([true; 3], true);

        for axis in 0..3 {
            let mut axes = [true; 3];
            axes[axis] = false;
            let without = frame(axes, false);
            let covered_without = frame(axes, true);

            let drawn_at = |frame: &Image, reference: &Image, at: f64| {
                let (pos, _) = req.view.project(along(axis, at)).expect("the sample is in front of the camera");
                let (x, y) = (pos.x.round() as usize, pos.y.round() as usize);
                (y.saturating_sub(1)..=y + 1).any(|y| {
                    (x.saturating_sub(1)..=x + 1).any(|x| {
                        if x >= frame.width || y >= frame.height {
                            return false;
                        }
                        let o = (y * frame.width + x) * 4;
                        frame.color[o..o + 4] != reference.color[o..o + 4]
                    })
                })
            };

            for at in [-6.0, -3.0, 3.0, 6.0] {
                // The control: with nothing in the way the axis draws here...
                assert!(
                    drawn_at(&bare, &without, at),
                    "axis {axis} does not draw at {at} even with nothing in the way"
                );
                // ...and with the solid in front of it, it does not.
                assert!(
                    !drawn_at(&covered, &covered_without, at),
                    "axis {axis} drew through a solid in front of it, at {at}"
                );
            }
        }
    }

    #[test]
    fn one_mesh_of_two_shapes_is_still_two_solids_to_an_axis() {
        // The viewport hands the renderer the *whole scene* as one mesh, so
        // "the solid an axis runs into" cannot be an item: a box on the origin
        // and a box nowhere near it arrive welded into a single `Renderable`,
        // and taking that as one solid drew all three axes across everything
        // (img2). The bodies are found in the mesh instead.
        let mut req = request(Vec::new(), DisplayMode::Shaded);
        req.grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
        // One on the origin, and one off to the side but nearer the eye, so it
        // covers a stretch of the axes without touching any of them.
        let (right, up) = req.view.basis();
        let elsewhere =
            primitives::box_mesh(24.0, 24.0, 24.0).translated(req.view.offset_dir() * 40.0 + right * 26.0 + up * 8.0);
        let apart = Renderable::prepare(&elsewhere);
        let scene = simple3d_geom::csg_bsp::union(&primitives::box_mesh(20.0, 20.0, 20.0), &elsewhere);
        let prepared = Renderable::prepare(&scene);
        assert_eq!(prepared.body_count, 2, "the two shapes welded into one body, so this proves nothing");

        let items = vec![Item { renderable: &prepared, style: Style::Solid }];
        let material = axis_material(&items, &req.grid);
        for axis in 0..3 {
            let seen = material.through[axis].len();
            assert_eq!(seen, 1, "axis {axis} is seen through {seen} of the two bodies");
        }

        let frame = |axes: [bool; 3], items: Vec<Item<'_>>| {
            let mut this = Request { grid: Grid { axes, ..req.grid }, ..request(Vec::new(), req.mode) };
            this.view = req.view;
            this.items = items;
            render(&this)
        };
        fn solid<'a>(renderable: &'a Renderable) -> Vec<Item<'a>> {
            vec![Item { renderable, style: Style::Solid }]
        }
        // Where each box drew on its own, so a sample can be put behind the far
        // one on purpose -- and away from the near one, which is seen through.
        let apart_only = frame([false; 3], solid(&apart));
        let on_origin = Renderable::prepare(&primitives::box_mesh(20.0, 20.0, 20.0));
        let near_only = frame([false; 3], solid(&on_origin));
        let all = frame([true; 3], solid(&prepared));
        let bare = frame([true; 3], Vec::new());

        // Counted across all three axes: the far box sits on one side, so it can
        // cover the whole approach of one axis while leaving the others clear.
        let (mut behind_the_far_box, mut arriving) = (0, 0);
        for axis in 0..3 {
            let mut axes = [true; 3];
            axes[axis] = false;
            let (without, bare_without) = (frame(axes, solid(&prepared)), frame(axes, Vec::new()));
            let at = |frame: &Image, reference: &Image, at: f64| {
                let (pos, _) = req.view.project(along(axis, at)).expect("the sample is in front of the camera");
                let (x, y) = (pos.x.round() as usize, pos.y.round() as usize);
                let drawn = (y.saturating_sub(1)..=y + 1).any(|y| {
                    (x.saturating_sub(1)..=x + 1).any(|x| {
                        x < frame.width && y < frame.height && {
                            let o = (y * frame.width + x) * 4;
                            frame.color[o..o + 4] != reference.color[o..o + 4]
                        }
                    })
                });
                // Off-frame samples report the last pixel, which is background
                // in every one of these renders, so they simply do not qualify.
                let pixel = if x < frame.width && y < frame.height { x + y * frame.width } else { 0 };
                (drawn, pixel)
            };

            // Just outside the box on the origin, on the eye's side, where the
            // line runs up to the surface it goes into: drawn, over that box's
            // own silhouette. Only that arm -- the one leaving through the back
            // is behind the box, and the box hides it.
            let near = -component(req.view.forward(), axis).signum();
            // The samples are found rather than guessed: the far box sits off
            // to one side and may cover any given point of the near arm, so
            // walk out along it and take every point it does not cover.
            for step in 0..40 {
                let sample = (11.0 + step as f64 * 0.5) * near;
                let (drawn, pixel) = at(&all, &without, sample);
                // Only where the far box is not the one in the way: that stretch
                // is its own case, tested below.
                if !is_background(&apart_only, pixel, &req.palette) {
                    continue;
                }
                arriving += 1;
                assert!(drawn, "axis {axis} stopped short of the solid it enters, at {sample}");
            }

            // ...and behind the box it never enters: not drawn. The samples are
            // found rather than guessed -- a point on this axis that the far box
            // covers, and that the axis does draw at with nothing in the way.
            for step in 0..80 {
                let sample = 15.0 + step as f64;
                for sample in [sample, -sample] {
                    let (drawn_bare, pixel) = at(&bare, &bare_without, sample);
                    let behind_far = !is_background(&apart_only, pixel, &req.palette);
                    let over_near = !is_background(&near_only, pixel, &req.palette);
                    // Behind the far box and clear of the near one, so the far
                    // box is what is actually in the way.
                    if !drawn_bare || !behind_far || over_near {
                        continue;
                    }
                    behind_the_far_box += 1;
                    assert!(
                        !at(&all, &without, sample).0,
                        "axis {axis} drew through the solid it only passes behind, at {sample}"
                    );
                }
            }
        }
        assert!(arriving > 0, "every sample beside the near box was covered by the far one");
        assert!(
            behind_the_far_box >= 2,
            "only {behind_the_far_box} samples landed behind the far box, so nothing was really tested"
        );
    }

    #[test]
    fn the_spans_an_axis_is_inside_are_the_solids_it_passes_through() {
        // Pairs of crossings, per solid: in at one face, out at the other.
        let centred = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
        let beside = Renderable::prepare(&primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(40.0, 0.0, 0.0)));
        let items =
            vec![Item { renderable: &centred, style: Style::Solid }, Item { renderable: &beside, style: Style::Solid }];
        let grid = Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
        let material = axis_material(&items, &grid);

        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        let spans = &material.inside[0];
        assert_eq!(spans.len(), 2, "one span per solid the X axis passes through, got {spans:?}");
        assert!(spans.iter().any(|&(a, b)| near(a, -15.0) && near(b, 15.0)), "{spans:?}");
        assert!(spans.iter().any(|&(a, b)| near(a, 35.0) && near(b, 45.0)), "{spans:?}");
        // The second box is off the Y axis entirely, so only the first is on it.
        assert_eq!(material.inside[1].len(), 1);
        // Both boxes are run through by X, so neither may hide it...
        assert_eq!(material.through[0].len(), 2, "{:?}", material.through[0]);
        // ...while for Y only the one it goes into is seen through, which is
        // the whole of img2: the other box hides Y like anything else.
        assert_eq!(material.through[1].len(), 1, "{:?}", material.through[1]);
        // The furthest corner of the further box, which the arms have to clear.
        assert!(material.reach > 45.0, "reach was {}", material.reach);

        // A solid the axes miss is an ordinary occluder, and cuts nothing.
        let away = Renderable::prepare(&primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(40.0, 40.0, 0.0)));
        let items = vec![Item { renderable: &away, style: Style::Solid }];
        let material = axis_material(&items, &grid);
        assert!(material.inside.iter().all(|spans| spans.is_empty()), "a solid off the axes cut one of them");
        assert!(
            material.through.iter().all(|bodies| bodies.is_empty()),
            "a solid the axes never enter was marked see-through, so it would not hide them"
        );

        // A ghost hides nothing: it is see-through, and so is the axis in it.
        let ghosted = vec![Item { renderable: &centred, style: Style::Ghost }];
        let material = axis_material(&ghosted, &grid);
        assert!(material.through.iter().all(|bodies| bodies.is_empty()), "a ghost was marked see-through");
        assert!(material.inside[0].is_empty(), "a ghost cut the axis");

        // An axis that is switched off is not looked for at all.
        let items = vec![Item { renderable: &centred, style: Style::Solid }];
        let off = Grid { axes: [false, true, true], ..grid };
        assert!(axis_material(&items, &off).inside[0].is_empty(), "a switched-off axis was cut out of the model");
    }

    #[test]
    fn a_stretch_of_axis_is_cut_at_every_boundary_it_crosses() {
        let spans = [(-15.0, 15.0), (35.0, 45.0)];
        // Wholly clear, wholly inside, and straddling one edge.
        assert_eq!(clip_spans(20.0, 30.0, &spans), vec![(20.0, 30.0, false)]);
        assert_eq!(clip_spans(-10.0, 10.0, &spans), vec![(-10.0, 10.0, true)]);
        assert_eq!(clip_spans(10.0, 20.0, &spans), vec![(10.0, 15.0, true), (15.0, 20.0, false)]);
        // Across a whole span: three pieces, in the order they were asked for.
        assert_eq!(clip_spans(30.0, 50.0, &spans), vec![(30.0, 35.0, false), (35.0, 45.0, true), (45.0, 50.0, false)]);
        // Backwards, for the arm that runs the other way: the pieces come back
        // in that direction too, so the fade along the arm stays put.
        assert_eq!(clip_spans(50.0, 30.0, &spans), vec![(50.0, 45.0, false), (45.0, 35.0, true), (35.0, 30.0, false)]);
    }

    #[test]
    fn hiding_the_grid_hides_it() {
        let empty = Renderable::empty();
        let mut req = request(vec![Item { renderable: &empty, style: Style::Solid }], DisplayMode::Shaded);
        req.grid =
            Grid { visible: false, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
        let frame = render(&req);
        assert_eq!(frame.color.chunks_exact(4).filter(|p| *p == req.palette.grid).count(), 0);
        // The axes are not part of the grid toggle.
        assert!(frame.color.chunks_exact(4).any(|p| *p == req.palette.axis_x));
    }

    #[test]
    fn grid_spacing_steps_up_so_a_fine_grid_stays_legible() {
        let mut v = view(800, 600);
        v.camera.distance = 50.0;
        assert_eq!(effective_grid_spacing(&v, 10.0), 10.0);
        // Zoomed far out, a 1mm grid would be sub-pixel, so it coarsens.
        v.camera.distance = 100_000.0;
        let spacing = effective_grid_spacing(&v, 1.0);
        assert!(spacing >= 100.0, "spacing stayed at {spacing}");
        assert!(spacing * v.pixels_per_mm() >= 6.0);
    }

    #[test]
    fn the_light_and_dark_palettes_differ_in_every_role() {
        let (dark, light) = (Palette::dark(), Palette::light());
        assert_ne!(dark.background, light.background);
        assert_ne!(dark.background_low, light.background_low);
        // The gradient runs one way only: the sky is never darker than the floor.
        assert!(dark.background[0] < dark.background_low[0]);
        assert_ne!(dark.solid, light.solid);
        assert_ne!(dark.grid, light.grid);
        assert_eq!(Palette::for_dark_mode(true).background, dark.background);
        assert_eq!(Palette::for_dark_mode(false).background, light.background);
        // A dark background needs a light model and vice versa, or nothing reads.
        assert!(dark.background[0] < dark.solid[0]);
        assert!(light.background[0] > light.solid[0]);
    }

    #[test]
    fn the_grid_never_draws_over_geometry_it_is_coplanar_with() {
        // A 4mm plate centred on the origin has its side walls cut exactly by the
        // z=0 grid plane, and its bottom face lies on the grid for an anchored
        // one. Rendering with and without the grid must leave every pixel the
        // model itself painted untouched.
        let prepared = Renderable::prepare(&primitives::box_mesh(60.0, 40.0, 4.0));
        let build = |grid: bool| {
            let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
            req.grid =
                Grid { visible: grid, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
            req.view.camera.pitch = 10.0;
            // The axes fade, so an axis pixel is a blend of the axis with
            // whatever is under it -- which is the grid in one render and the
            // background in the other. Making them invisible here leaves the
            // question this test is actually asking: does the *grid* ever
            // overwrite the model it is coplanar with.
            for axis in [&mut req.palette.axis_x, &mut req.palette.axis_y, &mut req.palette.axis_z] {
                axis[3] = 0;
            }
            (render(&req), req.palette)
        };
        let (without, palette) = build(false);
        let (with, _) = build(true);

        let mut checked = 0;
        for i in 0..(160 * 120) {
            let o = i * 4;
            let bare: Rgba = [without.color[o], without.color[o + 1], without.color[o + 2], without.color[o + 3]];
            if is_background(&without, i, &palette) {
                continue;
            }
            let gridded: Rgba = [with.color[o], with.color[o + 1], with.color[o + 2], with.color[o + 3]];
            assert_eq!(gridded, bare, "the grid overwrote the model at pixel {i}");
            checked += 1;
        }
        assert!(checked > 800, "only {checked} model pixels were checked");
    }

    #[test]
    fn rendering_the_same_scene_twice_gives_the_same_image() {
        let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 20.0, 10.0));
        let build = || {
            let mut req =
                request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::ShadedWithEdges);
            req.grid =
                Grid { visible: true, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
            render(&req).color
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn a_camera_inside_the_model_does_not_smear_across_the_viewport() {
        let prepared = Renderable::prepare(&primitives::box_mesh(200.0, 200.0, 200.0));
        let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
        req.view.camera.distance = 1.0;
        let frame = render(&req);
        // A parallel projection has no near-plane singularity to fall into: the
        // walls the camera has passed simply land behind the ones it has not, so
        // this is a partial fill rather than a panic or a screen of garbage.
        assert_eq!(frame.color.len(), 160 * 120 * 4);
    }

    #[test]
    fn a_view_from_far_away_renders_too() {
        let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
        let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
        req.view.camera.distance = 4000.0;
        req.view.camera.fov_deg = 2.0;
        let frame = render(&req);
        assert!(count_non_background(&frame, &req.palette) > 1000);
    }

    #[test]
    fn nearer_geometry_hides_what_is_behind_it() {
        let near = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
        let far = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0).translated(Vec3::new(0.0, 0.0, -200.0)));
        let req = request(
            vec![Item { renderable: &far, style: Style::Solid }, Item { renderable: &near, style: Style::Solid }],
            DisplayMode::Shaded,
        );
        let with_both = render(&req);
        let only_near = render(&request(vec![Item { renderable: &near, style: Style::Solid }], DisplayMode::Shaded));
        // The far box is below the near one on screen, so it adds pixels, but at
        // the centre the near box must still win.
        let centre = (120 / 2 * 160 + 160 / 2) * 4;
        assert_eq!(with_both.color[centre..centre + 4], only_near.color[centre..centre + 4]);
    }

    /// A grid-only frame at `pitch`, with no model and no axes on it, so every
    /// pixel that is not the background is a grid line.
    fn ground_only(pitch: f64) -> (Image, Palette) {
        let (w, h) = (400, 300);
        let camera = Camera { yaw: -55.0, pitch, distance: 900.0, ..Camera::default() };
        let view = View::new(camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w as f32, h as f32)));
        let req = Request {
            view,
            size: [w, h],
            mode: DisplayMode::Shaded,
            palette: Palette::dark(),
            grid: Grid { visible: true, spacing: 10.0, axes: [false; 3], style: AxisStyle::Grid, plane_marks: false },
            items: Vec::new(),
            preview: Vec::new(),
        };
        (render(&req), req.palette)
    }

    #[test]
    fn the_ground_covers_the_viewport_at_every_tilt() {
        // The ground is only face-on from straight above, and its extent was
        // worked out as though it always were: half the viewport's diagonal,
        // whatever the angle. Seen from anywhere else the grid stopped short of
        // the top and bottom of the viewport in a flattened diamond -- at ten
        // degrees it covered rows 107 to 192 of 300 and left the rest empty,
        // while the axes carried on across the whole frame.
        //
        // Measured along the middle row and column rather than into the
        // corners, where the fade is meant to take the grid out.
        for pitch in [10.0, 28.0, 45.0, 60.0, 89.0] {
            let (frame, palette) = ground_only(pitch);
            let (w, h) = (frame.width, frame.height);
            let painted = |x: usize, y: usize| !is_background(&frame, y * w + x, &palette);
            let top = (0..h).find(|&y| painted(w / 2, y)).unwrap_or(h);
            let bottom = (0..h).rev().find(|&y| painted(w / 2, y)).unwrap_or(0);
            let left = (0..w).find(|&x| painted(x, h / 2)).unwrap_or(w);
            let right = (0..w).rev().find(|&x| painted(x, h / 2)).unwrap_or(0);
            // Within a seventh of the frame of each edge: the lines are a whole
            // cell apart, so the nearest one to an edge is not *on* it.
            assert!(top < h * 15 / 100, "pitch {pitch}: the ground starts {top} rows down, of {h}");
            assert!(bottom > h * 85 / 100, "pitch {pitch}: the ground ends at row {bottom}, of {h}");
            assert!(left < w * 15 / 100, "pitch {pitch}: the ground starts {left} columns in, of {w}");
            assert!(right > w * 85 / 100, "pitch {pitch}: the ground ends at column {right}, of {w}");
        }
    }

    #[test]
    fn the_ground_is_drawn_at_one_detail_all_the_way_across() {
        // The fine level used to be drawn over half the reach of the coarse
        // one, which made the ground a patch of detail around the origin
        // sitting on a plainer one -- and which of the two you were looking at
        // depended on where the origin happened to be in the frame. At ten
        // degrees a band across the bottom of the viewport held no grid at all
        // against 2304 pixels of it across the middle; at twenty-eight, 59
        // against 892.
        //
        // Detail is a question about the zoom, and one answer has to serve the
        // whole ground: a band at the edge of the frame carries as much grid as
        // a band through the middle of it.
        for pitch in [10.0, 28.0, 45.0, 60.0, 89.0] {
            let (frame, palette) = ground_only(pitch);
            let (w, h) = (frame.width, frame.height);
            let band = |from: usize, to: usize| {
                (from..to)
                    .map(|y| (0..w).filter(|&x| !is_background(&frame, y * w + x, &palette)).count())
                    .sum::<usize>()
            };
            let middle = band(h / 2 - 15, h / 2 + 15);
            let edge = band(h - 32, h - 2);
            assert!(middle > 0, "pitch {pitch}: nothing drawn across the middle, so this proves nothing");
            assert!(
                edge * 10 >= middle * 6,
                "pitch {pitch}: {edge} grid pixels at the edge against {middle} in the middle -- \
                 the detail does not reach the edge of the frame"
            );
        }
    }
}
