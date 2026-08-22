//! Turning the evaluated scene into the viewport image (spec section 6.1):
//! shaded / shaded-with-edges / wireframe display, the ground grid and origin
//! axes, the selection highlight and translucent ghosts for hidden nodes.

use crate::raster::{Frame, Rgba, Vertex};
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::{AxisStyle, Colour};
use simple3d_geom::{Mesh, Vec3};
use std::collections::HashMap;

/// A mesh prepared for drawing: welded, so edges can be found, together with
/// its feature edges.
pub struct Renderable {
    pub mesh: Mesh,
    /// Edges worth drawing: a real crease in the surface, not an artefact of how
    /// a flat face happens to be triangulated.
    pub edges: Vec<[u32; 2]>,
}

impl Renderable {
    pub fn prepare(mesh: &Mesh) -> Renderable {
        let welded = mesh.weld();
        let edges = feature_edges(&welded, 20.0);
        Renderable { mesh: welded, edges }
    }

    pub fn empty() -> Renderable {
        Renderable { mesh: Mesh::new(), edges: Vec::new() }
    }
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
    let height = frame.height;
    for row in 0..height {
        let colour = palette.background_at(row, height);
        for column in 0..frame.width {
            let offset = (row * frame.width + column) * 4;
            frame.color[offset..offset + 4].copy_from_slice(&colour);
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
}

pub fn render(request: &Request<'_>) -> Frame {
    let [width, height] = request.size;
    let mut frame = Frame::new(width.max(1), height.max(1));
    fill_background(&mut frame, &request.palette);
    if width == 0 || height == 0 {
        return frame;
    }
    let view = request.view;

    if request.grid.visible {
        draw_grid(&mut frame, &view, &request.grid, &request.palette);
    }
    // What each axis runs through: the stretches inside material, which are cut
    // out of the line, and which solid each one goes into, which is the solid
    // that may not hide it. Every item is drawn under its own tag so the axes
    // can ask that question of the depth buffer a pixel at a time.
    let material = axis_material(&request.items, &request.grid);
    for (index, item) in request.items.iter().enumerate() {
        frame.set_tag(index as u16 + 1);
        match item.style {
            Style::Solid => match request.mode {
                DisplayMode::Wireframe => draw_wireframe(&mut frame, &view, item.renderable, request.palette.wire),
                DisplayMode::Shaded => draw_shaded(&mut frame, &view, item.renderable, request.palette.solid, 255),
                DisplayMode::ShadedWithEdges => {
                    draw_shaded(&mut frame, &view, item.renderable, request.palette.solid, 255);
                    draw_edges(&mut frame, &view, item.renderable, request.palette.edge);
                }
            },
            Style::Selected => {
                // Always outlined, in every display mode: the selection has to
                // be visible, and an outline reads clearly over a shaded body.
                // A larger bias than the solid's own edges, or the two would tie
                // at equal depth and the outline would lose.
                draw_selection(&mut frame, &view, item.renderable, request.palette.selected);
            }
            Style::Ghost => draw_ghost(&mut frame, &view, item.renderable, request.palette.ghost),
        }
    }
    frame.set_tag(0);
    if request.grid.plane_marks && request.mode != DisplayMode::Wireframe {
        // After the solids: the mark belongs on the surface, and in wireframe
        // there is no surface for it to sit on.
        draw_plane_marks(&mut frame, &view, &request.items, &request.palette, &request.grid);
    }
    // Last: an axis is hidden by two things and no others -- the material it
    // runs through, cut out of the line itself, and any solid in front of it
    // that it does not run through. The solid it *does* run through cannot hide
    // it: the arm leaving that solid is outside it from the surface onwards,
    // but its projection stays over the shape most of the way to the
    // silhouette, and the depth test alone ate exactly the stretch that says
    // where the line goes in (issue 47). The grid is untouched, drawn first and
    // covered by everything.
    draw_axes(&mut frame, &view, &request.palette, &request.grid, &material);
    frame
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

fn draw_shaded(frame: &mut Frame, view: &View, item: &Renderable, base: Rgba, alpha: u8) {
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
        let colour = shade(triangle_base(item, index, base), normal, forward, alpha);
        let in_view = [view.to_view(world[0]), view.to_view(world[1]), view.to_view(world[2])];
        frame.triangle(
            [to_vertex(view, in_view[0]), to_vertex(view, in_view[1]), to_vertex(view, in_view[2])],
            colour,
            true,
        );
    }
}

/// Ghosts are drawn without back-face culling and without writing depth, so a
/// hidden tool body reads as a translucent volume rather than a flat patch.
fn draw_ghost(frame: &mut Frame, view: &View, item: &Renderable, base: Rgba) {
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
        frame.triangle(
            [to_vertex(view, in_view[0]), to_vertex(view, in_view[1]), to_vertex(view, in_view[2])],
            colour,
            false,
        );
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

fn draw_edges(frame: &mut Frame, view: &View, item: &Renderable, colour: Rgba) {
    for edge in &item.edges {
        let a = item.mesh.positions[edge[0] as usize];
        let b = item.mesh.positions[edge[1] as usize];
        draw_world_line(frame, view, a, b, colour, EDGE_BIAS);
    }
}

fn draw_selection(frame: &mut Frame, view: &View, item: &Renderable, colour: Rgba) {
    for edge in &item.edges {
        let a = item.mesh.positions[edge[0] as usize];
        let b = item.mesh.positions[edge[1] as usize];
        draw_world_line(frame, view, a, b, colour, SELECTION_BIAS);
    }
}

fn draw_wireframe(frame: &mut Frame, view: &View, item: &Renderable, colour: Rgba) {
    for edge in &item.edges {
        let a = item.mesh.positions[edge[0] as usize];
        let b = item.mesh.positions[edge[1] as usize];
        // No depth bias and no filled faces, so the whole wireframe is visible
        // including the far side -- which is the point of wireframe.
        draw_world_line(frame, view, a, b, colour, 0.0);
    }
}

/// Draw a world-space line, clipping it against the near plane so a segment
/// crossing behind the eye does not project to nonsense. `bias` is a fraction of
/// the line's own depth key, not an absolute amount.
fn draw_world_line(frame: &mut Frame, view: &View, a: Vec3, b: Vec3, colour: Rgba, bias: f32) {
    draw_world_line_with_depth(frame, view, a, b, colour, bias, true)
}

/// A stretch of an axis: hidden by whatever is in front of it except the solids
/// in `through`, and never leaving depth of its own.
fn draw_axis_line(frame: &mut Frame, view: &View, a: Vec3, b: Vec3, colour: Rgba, through: &[bool]) {
    let (va, vb) = (to_vertex(view, view.to_view(a)), to_vertex(view, view.to_view(b)));
    frame.line_through(va, vb, colour, AXIS_BIAS, through);
}

/// As `draw_world_line`, with the depth *write* optional: the grid is
/// depth-tested against the model but leaves no depth of its own.
fn draw_world_line_with_depth(
    frame: &mut Frame,
    view: &View,
    a: Vec3,
    b: Vec3,
    colour: Rgba,
    bias: f32,
    write_depth: bool,
) {
    // Nothing is clipped: a parallel projection maps a point behind the eye to
    // its true screen position, and the depth key puts it behind everything
    // else on its own.
    let (va, vb) = (to_vertex(view, view.to_view(a)), to_vertex(view, view.to_view(b)));
    let scale = (va.key.abs() + vb.key.abs()) * 0.5;
    if write_depth {
        frame.line(va, vb, colour, bias * scale);
    } else {
        frame.line_with_depth(va, vb, colour, bias * scale, false);
    }
}

/// The world radius the ground grid and the axes cover.
///
/// It is derived from the frame rather than from the grid's spacing: half the
/// viewport's diagonal, converted to millimetres at the current zoom, with a
/// margin so the fade finishes just outside the corners. That makes it
/// continuous in the zoom -- the extent of the ground grows smoothly as the
/// camera pulls back, instead of jumping tenfold whenever the spacing steps up
/// a decade, which is what made zooming out lurch.
pub fn grid_radius(view: &View) -> f64 {
    let half_diagonal = ((view.size.x as f64).hypot(view.size.y as f64) / 2.0).max(1.0);
    half_diagonal / view.pixels_per_mm().max(1e-9) * 1.35
}

/// The narrowest a grid cell may be drawn, in pixels, and the width at which it
/// is drawn at full strength. Between the two it is faded, which is what makes a
/// decade of the grid arrive and leave without a step.
const CELL_FADE_OUT: f64 = 6.0;
const CELL_FULL: f64 = 26.0;

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
    let pixels_per_mm = view.pixels_per_mm();
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
/// The fine level at its densest would otherwise be several hundred, which is
/// pixels of work for lines that are all but invisible by then.
const MAX_LINES: i64 = 140;

fn draw_grid(frame: &mut Frame, view: &View, grid: &Grid, palette: &Palette) {
    let (fine, coarse, strength) = grid_levels(view, grid.spacing);
    let radius = grid_radius(view);
    // The fine level is drawn over a shorter reach than the coarse one: far
    // from the centre its lines are a wash rather than a measure, and drawing
    // them there costs the most.
    if strength > 0.03 {
        draw_grid_level(frame, view, fine, radius * 0.55, strength, false, palette);
    }
    draw_grid_level(frame, view, coarse, radius, 1.0, true, palette);
}

/// One decade of the ground grid, centred on the camera target so panning never
/// runs off the end of it. The centre is snapped to the level's own spacing, so
/// every line sits at a whole multiple of it -- which is what keeps the line
/// through zero *on* zero, and the X and Y axes lying along the grid rather
/// than across it.
fn draw_grid_level(
    frame: &mut Frame,
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
    let centre = Vec3::new(cx, cy, 0.0);
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
        faded_line(
            frame,
            view,
            Vec3::new(cx + offset, cy - half, 0.0),
            Vec3::new(cx + offset, cy + half, 0.0),
            centre,
            radius,
            shade(cx + offset),
            GRID_BIAS,
        );
        faded_line(
            frame,
            view,
            Vec3::new(cx - half, cy + offset, 0.0),
            Vec3::new(cx + half, cy + offset, 0.0),
            centre,
            radius,
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
fn faded_line(
    frame: &mut Frame,
    view: &View,
    from: Vec3,
    to: Vec3,
    centre: Vec3,
    radius: f64,
    colour: Rgba,
    bias: f32,
) {
    for step in 0..FADE_STEPS {
        let t0 = step as f64 / FADE_STEPS as f64;
        let t1 = (step + 1) as f64 / FADE_STEPS as f64;
        let a = from + (to - from) * t0;
        let b = from + (to - from) * t1;
        let mid = (a + b) * 0.5;
        let distance = (mid - centre).length();
        // Squared falloff: near the origin the grid is at full strength, and it
        // is gone well before the last line rather than at it.
        let fade = 1.0 - (distance / (radius * 0.8)).min(1.0).powi(2);
        if fade <= 0.03 {
            continue;
        }
        let faded = [colour[0], colour[1], colour[2], (colour[3] as f64 * fade).round() as u8];
        // Never writes depth: the grid and the axes are drawn before the model
        // and must lose every tie with it, including the exact ties a ground
        // plane makes with a plate whose side walls it cuts.
        draw_world_line_with_depth(frame, view, a, b, faded, bias, false);
    }
}

/// What the axes meet in the model: where they run inside it, which solids they
/// go through, and how far from the origin the model reaches.
struct AxisMaterial {
    /// Per axis, the stretches inside a solid, as coordinates along that axis.
    inside: [Vec<(f64, f64)>; 3],
    /// Per axis, per item, whether that axis runs through that solid. A solid
    /// pierced by Y is still an ordinary occluder for X and Z, so this cannot be
    /// collapsed to one flag per item.
    pierced: [Vec<bool>; 3],
    /// The distance from the origin to the model's furthest vertex. What an
    /// origin axis's arms have to be longer than, or a shape standing on the
    /// origin holds the whole arm and the axis is never seen at all.
    reach: f64,
}

fn axis_material(items: &[Item<'_>], grid: &Grid) -> AxisMaterial {
    let none = || vec![false; items.len()];
    let mut material =
        AxisMaterial { inside: [Vec::new(), Vec::new(), Vec::new()], pierced: [none(), none(), none()], reach: 0.0 };
    for (index, item) in items.iter().enumerate() {
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
            let spans = axis_inside_spans(item, axis);
            material.pierced[axis][index] = !spans.is_empty();
            material.inside[axis].extend(spans);
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
/// there, and picks up again where it comes out. The stretch behind the shape is
/// the same line, so it is drawn like the rest of it (issues 20, 36, 47).
#[allow(clippy::too_many_arguments)]
fn faded_axis_line(
    frame: &mut Frame,
    view: &View,
    centre: Vec3,
    to: Vec3,
    reach: f64,
    colour: Rgba,
    axis: usize,
    inside: &[(f64, f64)],
    through: &[bool],
) {
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
        for (from, to) in outside_spans(component(a, axis), component(b, axis), inside) {
            draw_axis_line(frame, view, along(axis, from), along(axis, to), faded, through);
        }
    }
}

/// The parts of `[a, b]` -- a stretch of an axis, given as the coordinate along
/// it -- that are not inside any of `inside`. Either end may be the larger; the
/// pieces come back in the order they were asked for, so a line keeps its
/// direction and its fade.
fn outside_spans(a: f64, b: f64, inside: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let (lo, hi) = (a.min(b), a.max(b));
    let mut pieces = vec![(lo, hi)];
    for &(start, end) in inside {
        let mut next = Vec::with_capacity(pieces.len() + 1);
        for (from, to) in pieces {
            if end <= from || start >= to {
                next.push((from, to));
                continue;
            }
            if start > from {
                next.push((from, start));
            }
            if end < to {
                next.push((end, to));
            }
        }
        pieces = next;
    }
    if a > b {
        pieces.reverse();
        pieces = pieces.into_iter().map(|(from, to)| (to, from)).collect();
    }
    pieces
}

/// Where along `axis` one solid is, as spans of the coordinate along it.
///
/// A closed surface is crossed an even number of times, so the crossings sorted
/// and taken in pairs are the stretches inside it. An odd count means the line
/// grazed an edge or the mesh is not closed; the odd one out is dropped rather
/// than turned into a span that runs to infinity.
fn axis_inside_spans(item: &Item<'_>, axis: usize) -> Vec<(f64, f64)> {
    // The axis passes through the origin, so a solid that does not straddle zero
    // on the other two coordinates cannot be on it -- which is most of them, and
    // this is the whole mesh not looked at.
    let Some((lo, hi)) = item.renderable.mesh.bounds() else { return Vec::new() };
    if (0..3).any(|other| other != axis && (component(lo, other) > 0.0 || component(hi, other) < 0.0)) {
        return Vec::new();
    }
    let mut crossings: Vec<f64> = Vec::new();
    for tri in &item.renderable.mesh.indices {
        let world = [
            item.renderable.mesh.positions[tri[0] as usize],
            item.renderable.mesh.positions[tri[1] as usize],
            item.renderable.mesh.positions[tri[2] as usize],
        ];
        if let Some(at) = axis_crossing(world, axis) {
            crossings.push(at);
        }
    }
    crossings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // A crossing on a shared edge is found twice, once for each triangle.
    crossings.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    crossings.chunks_exact(2).map(|pair| (pair[0], pair[1])).collect()
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

fn draw_axes(frame: &mut Frame, view: &View, palette: &Palette, grid: &Grid, material: &AxisMaterial) {
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
        // ...and the solids it runs through, which do not hide the rest of it.
        // Tags are item indices plus one, 0 being a pixel no item claimed.
        let mut through = vec![false; material.pierced[axis].len() + 1];
        for (index, pierced) in material.pierced[axis].iter().enumerate() {
            through[index + 1] = *pierced;
        }
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
                // Bounded, and always well inside the ground's own reach, so it
                // reads as a cross at the origin rather than as another pair of
                // grid lines however far the camera is pulled back.
                let length = (spacing * 12.0).min(radius * 0.45);
                // ...but never so short that the model swallows the whole arm.
                // The arms are measured from the origin, which is inside a shape
                // standing on it, so an arm that ends inside that shape is an
                // axis with nothing left to draw -- which is what zooming in on
                // a box at the origin gave: a mark on the face and no line
                // arriving at it.
                let clear = (clearance * 1.6 + 30.0 / view.pixels_per_mm().max(1e-9)).min(radius * 1.5);
                let length = length.max(clear);
                (Vec3::ZERO, length, length * 2.0)
            }
        };
        // Both halves fade outward from the centre, for the same reason the
        // grid does: an axis that ends abruptly reads as an object.
        for sign in [-1.0, 1.0] {
            faded_axis_line(
                frame,
                view,
                centre,
                centre + directions[axis] * (length * sign),
                reach,
                colours[axis],
                axis,
                inside,
                &through,
            );
        }
    }
}

/// Draw, on the surface of each solid, the line where a principal plane cuts
/// through it.
///
/// The ground plane crossing a shape is a real dimension -- how much of the
/// shape is below the build plate -- and until something marks it the only way
/// to read it is to orbit until the grid is edge-on. The mark is drawn on the
/// surface itself, where the plane meets it, in the colour of the axis the
/// plane is perpendicular to.
fn draw_plane_marks(frame: &mut Frame, view: &View, items: &[Item<'_>], palette: &Palette, grid: &Grid) {
    let colours = [palette.axis_x, palette.axis_y, palette.axis_z];
    for item in items.iter().filter(|i| i.style == Style::Solid) {
        for (axis, colour) in colours.into_iter().enumerate() {
            if !grid.axes[axis] {
                continue;
            }
            for tri in &item.renderable.mesh.indices {
                let world = [
                    item.renderable.mesh.positions[tri[0] as usize],
                    item.renderable.mesh.positions[tri[1] as usize],
                    item.renderable.mesh.positions[tri[2] as usize],
                ];
                if let Some((a, b)) = plane_crossing(world, axis) {
                    draw_world_line(frame, view, a, b, colour, MARK_BIAS);
                }
            }
        }
    }
}

/// Where the plane through the origin perpendicular to `axis` crosses one
/// triangle, as the segment it cuts. `None` when the triangle is wholly on one
/// side, which is nearly all of them, so this is the cheap case.
fn plane_crossing(world: [Vec3; 3], axis: usize) -> Option<(Vec3, Vec3)> {
    let component = |p: Vec3| match axis {
        0 => p.x,
        1 => p.y,
        _ => p.z,
    };
    let d = [component(world[0]), component(world[1]), component(world[2])];
    if (d[0] > 0.0 && d[1] > 0.0 && d[2] > 0.0) || (d[0] < 0.0 && d[1] < 0.0 && d[2] < 0.0) {
        return None;
    }
    // A triangle lying *in* the plane has no crossing line of its own -- its
    // three edges are the mark, and its neighbours draw them.
    if d[0] == 0.0 && d[1] == 0.0 && d[2] == 0.0 {
        return None;
    }
    let mut hits: Vec<Vec3> = Vec::new();
    for i in 0..3 {
        let j = (i + 1) % 3;
        if d[i] == 0.0 {
            hits.push(world[i]);
        }
        if (d[i] < 0.0 && d[j] > 0.0) || (d[i] > 0.0 && d[j] < 0.0) {
            let t = d[i] / (d[i] - d[j]);
            hits.push(world[i] + (world[j] - world[i]) * t);
        }
    }
    (hits.len() >= 2).then(|| (hits[0], hits[1]))
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
        }
    }

    fn count_non_background(frame: &Frame, palette: &Palette) -> usize {
        (0..frame.height * frame.width).filter(|&i| !is_background(frame, i, palette)).count()
    }

    /// Whether pixel `index` still holds the gradient it was cleared to.
    fn is_background(frame: &Frame, index: usize, palette: &Palette) -> bool {
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
    fn unpainted_inside(hull: &[egui::Pos2], frame: &Frame, empty: &Frame, margin: f32) -> usize {
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
    fn pixels_of(frame: &Frame, colour: Rgba) -> usize {
        (0..frame.width * frame.height)
            .filter(|&i| {
                let o = i * 4;
                frame.color[o] == colour[0] && frame.color[o + 1] == colour[1] && frame.color[o + 2] == colour[2]
            })
            .count()
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
    fn a_plane_mark_follows_the_switch_of_the_axis_that_names_it() {
        // Each plane is named by the axis it is perpendicular to, so turning
        // off the Z axis turns off the ground plane's mark with it.
        let renderable = Renderable::prepare(&primitives::box_mesh(40.0, 40.0, 40.0));
        let mut req = request(vec![Item { renderable: &renderable, style: Style::Solid }], DisplayMode::Shaded);
        req.grid = Grid {
            visible: false,
            spacing: 10.0,
            axes: [true, false, false],
            style: AxisStyle::Grid,
            plane_marks: true,
        };
        let frame = render(&req);
        assert!(pixels_of(&frame, req.palette.axis_x) > 0, "the X plane's mark should be drawn");
        assert_eq!(pixels_of(&frame, req.palette.axis_z), 0, "the Z plane's mark should be off with its axis");
    }

    #[test]
    fn a_plane_cuts_a_triangle_in_at_most_one_segment() {
        let above = [Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 2.0), Vec3::new(0.0, 1.0, 3.0)];
        assert!(plane_crossing(above, 2).is_none());
        let crossing = [Vec3::new(0.0, 0.0, -1.0), Vec3::new(2.0, 0.0, 1.0), Vec3::new(0.0, 2.0, 1.0)];
        let (a, b) = plane_crossing(crossing, 2).expect("this triangle straddles z = 0");
        assert!(a.z.abs() < 1e-9 && b.z.abs() < 1e-9, "the cut has to lie in the plane: {a:?} {b:?}");
        // A triangle lying in the plane is left to its neighbours: its own
        // edges are the mark, and it has no interior crossing.
        let flat = [Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
        assert!(plane_crossing(flat, 2).is_none());
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
    fn an_axis_stops_at_the_material_it_runs_into_and_starts_again_where_it_comes_out() {
        // Issue 47, and the rule the three earlier passes all missed. What
        // hides an axis is the material it runs *through*, not the depth
        // buffer: a shape merely standing in front of the line is no reason to
        // drop it, or the stretch arriving at that shape goes missing and only
        // the point where the line meets the surface is left.
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

            // Inside the box, which spans -15..15: nothing of the line.
            for at in [-12.0, -6.0, 0.0, 6.0, 12.0] {
                assert!(!drawn_at(at), "axis {axis} drew inside the solid, at {at}");
            }
            // Outside it: the line is there, unbroken, right up to the surface
            // it goes into -- including where it is still over the box's own
            // silhouette, which is the stretch the depth test used to eat.
            for at in [-24.0, -18.0, -16.0, 16.0, 18.0, 24.0] {
                assert!(drawn_at(at), "axis {axis} left a gap outside the solid, at {at}");
            }
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
            axis_material(&items, &req.grid).pierced.iter().all(|pierced| pierced == &vec![false]),
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

            let drawn_at = |frame: &Frame, reference: &Frame, at: f64| {
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
        // Both are run through by X, so neither may hide it...
        assert_eq!(material.pierced[0], vec![true, true]);
        // ...while the box off the Y axis is an ordinary occluder for that one,
        // which is why this cannot be one flag per solid.
        assert_eq!(material.pierced[1], vec![true, false]);
        // The furthest corner of the further box, which the arms have to clear.
        assert!(material.reach > 45.0, "reach was {}", material.reach);

        // A solid the axes miss is an ordinary occluder, and cuts nothing.
        let away = Renderable::prepare(&primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(40.0, 40.0, 0.0)));
        let items = vec![Item { renderable: &away, style: Style::Solid }];
        let material = axis_material(&items, &grid);
        assert!(material.pierced.iter().all(|axis| axis == &vec![false]), "a solid off the axes was taken as pierced");
        assert!(material.inside.iter().all(|spans| spans.is_empty()), "a solid off the axes cut one of them");

        // A ghost hides nothing: it is see-through, and so is the axis in it.
        let ghosted = vec![Item { renderable: &centred, style: Style::Ghost }];
        let material = axis_material(&ghosted, &grid);
        assert!(material.pierced.iter().all(|axis| axis == &vec![false]), "a ghost was taken as pierced");
        assert!(material.inside[0].is_empty(), "a ghost cut the axis");

        // An axis that is switched off is not looked for at all.
        let items = vec![Item { renderable: &centred, style: Style::Solid }];
        let off = Grid { axes: [false, true, true], ..grid };
        assert!(axis_material(&items, &off).inside[0].is_empty(), "a switched-off axis was cut out of the model");
    }

    #[test]
    fn a_stretch_of_axis_is_clipped_against_what_hides_it() {
        let hidden = [(-15.0, 15.0), (35.0, 45.0)];
        // Wholly clear, wholly hidden, and straddling an edge.
        assert_eq!(outside_spans(20.0, 30.0, &hidden), vec![(20.0, 30.0)]);
        assert!(outside_spans(-10.0, 10.0, &hidden).is_empty());
        assert_eq!(outside_spans(10.0, 20.0, &hidden), vec![(15.0, 20.0)]);
        // Across a whole span: two pieces, in the order they were asked for.
        assert_eq!(outside_spans(30.0, 50.0, &hidden), vec![(30.0, 35.0), (45.0, 50.0)]);
        // Backwards, for the arm that runs the other way: the pieces come back
        // in that direction too, so the fade along the arm stays put.
        assert_eq!(outside_spans(50.0, 30.0, &hidden), vec![(50.0, 45.0), (35.0, 30.0)]);
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
}
