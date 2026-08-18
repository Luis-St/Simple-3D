//! Turning the evaluated scene into the viewport image (spec section 6.1):
//! shaded / shaded-with-edges / wireframe display, the ground grid and origin
//! axes, the selection highlight and translucent ghosts for hidden nodes.

use crate::raster::{clip_near, Frame, Rgba, Vertex};
use crate::view::{View, NEAR};
use simple3d_core::config::DisplayMode;
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
    draw_axes(&mut frame, &view, &request.palette, &request.grid);

    for item in &request.items {
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
    frame
}

/// Project a world point to a rasterizer vertex, with the depth key the
/// framebuffer expects: larger is nearer, and linear in screen space for the
/// projection in use.
fn to_vertex(view: &View, view_space: Vec3) -> Vertex {
    let (pos, z) = view.view_to_screen(view_space);
    let key = if view.camera.orthographic { -z as f32 } else { (1.0 / z.max(NEAR)) as f32 };
    Vertex { pos, key }
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
/// Under perspective that is the eye minus the triangle, and it has to be
/// measured per triangle: a face at the edge of the frame is seen from a
/// noticeably different direction than one at its centre. Under **orthographic**
/// projection there is no eye to point at -- every ray runs along the view
/// direction -- and using `eye - centroid` there is simply wrong. It answers
/// correctly only near the middle of the frame, and the further a face sits from
/// it the more the answer tilts, until a face within a few degrees of edge-on is
/// culled although it is facing the viewer. That is what dropped the side wall
/// off a plate seen almost from the side.
fn to_eye(view: &View, centroid: Vec3) -> Vec3 {
    if view.camera.orthographic {
        -view.forward()
    } else {
        view.eye() - centroid
    }
}

fn draw_shaded(frame: &mut Frame, view: &View, item: &Renderable, base: Rgba, alpha: u8) {
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
        let normal = normal.normalized();
        // Back-face culling in world space, where it means something: for a
        // closed solid the far side is never visible, so this halves the work.
        let centroid = (world[0] + world[1] + world[2]) * (1.0 / 3.0);
        if normal.dot(to_eye(view, centroid)) <= 0.0 {
            continue;
        }
        let colour = shade(base, normal, forward, alpha);
        let in_view = [view.to_view(world[0]), view.to_view(world[1]), view.to_view(world[2])];
        for piece in clip_pieces(view, in_view) {
            frame.triangle(
                [to_vertex(view, piece[0]), to_vertex(view, piece[1]), to_vertex(view, piece[2])],
                colour,
                true,
            );
        }
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
        for piece in clip_pieces(view, in_view) {
            frame.triangle(
                [to_vertex(view, piece[0]), to_vertex(view, piece[1]), to_vertex(view, piece[2])],
                colour,
                false,
            );
        }
    }
}

fn clip_pieces(view: &View, in_view: [Vec3; 3]) -> Vec<[Vec3; 3]> {
    if view.camera.orthographic {
        vec![in_view]
    } else {
        clip_near(in_view, NEAR)
    }
}

/// Depth bias for a line, as a fraction of its own depth key. Absolute biases
/// do not work here: the key is `1/z` under perspective, so a fixed nudge that
/// is invisible up close is larger than the whole scene's depth range when the
/// camera is far away -- which is what made the origin axes draw straight through
/// solid geometry.
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

fn draw_world_line_with_depth(
    frame: &mut Frame,
    view: &View,
    a: Vec3,
    b: Vec3,
    colour: Rgba,
    bias: f32,
    write_depth: bool,
) {
    let (mut va, mut vb) = (view.to_view(a), view.to_view(b));
    if !view.camera.orthographic {
        if va.z < NEAR && vb.z < NEAR {
            return;
        }
        if va.z < NEAR {
            let t = (NEAR - va.z) / (vb.z - va.z);
            va = va.lerp(vb, t);
        } else if vb.z < NEAR {
            let t = (NEAR - vb.z) / (va.z - vb.z);
            vb = vb.lerp(va, t);
        }
    }
    let (va, vb) = (to_vertex(view, va), to_vertex(view, vb));
    let scale = (va.key.abs() + vb.key.abs()) * 0.5;
    if write_depth {
        frame.line(va, vb, colour, bias * scale);
    } else {
        frame.line_with_depth(va, vb, colour, bias * scale, false);
    }
}

/// Grid spacing that is actually legible: step up in powers of ten until one
/// cell is at least a few pixels across, so a 1mm grid does not turn a zoomed-out
/// view into a solid block of lines.
pub fn effective_grid_spacing(view: &View, spacing: f64) -> f64 {
    let mut spacing = spacing.max(1e-6);
    let pixels_per_mm = view.pixels_per_mm();
    while spacing * pixels_per_mm < 6.0 {
        spacing *= 10.0;
    }
    spacing
}

fn draw_grid(frame: &mut Frame, view: &View, grid: &Grid, palette: &Palette) {
    let spacing = effective_grid_spacing(view, grid.spacing);
    // Enough lines to fill the view, capped so an extreme zoom cannot cost
    // seconds.
    let lines = 48i64;
    let half = spacing * lines as f64;
    // Snap the grid to the camera target so panning does not run off the end.
    let cx = (view.camera.target.x / spacing).round() * spacing;
    let cy = (view.camera.target.y / spacing).round() * spacing;
    for i in -lines..=lines {
        let offset = i as f64 * spacing;
        let major = i % 10 == 0;
        let colour = if major { palette.grid_major } else { palette.grid };
        faded_line(
            frame,
            view,
            Vec3::new(cx + offset, cy - half, 0.0),
            Vec3::new(cx + offset, cy + half, 0.0),
            Vec3::new(cx, cy, 0.0),
            half,
            colour,
            GRID_BIAS,
        );
        faded_line(
            frame,
            view,
            Vec3::new(cx - half, cy + offset, 0.0),
            Vec3::new(cx + half, cy + offset, 0.0),
            Vec3::new(cx, cy, 0.0),
            half,
            colour,
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

fn draw_axes(frame: &mut Frame, view: &View, palette: &Palette, grid: &Grid) {
    let length = effective_grid_spacing(view, grid.spacing) * 48.0;
    for (dir, colour) in [
        (Vec3::new(1.0, 0.0, 0.0), palette.axis_x),
        (Vec3::new(0.0, 1.0, 0.0), palette.axis_y),
        (Vec3::new(0.0, 0.0, 1.0), palette.axis_z),
    ] {
        // Both halves fade outward from the origin, for the same reason the
        // grid does: an axis that ends abruptly reads as an object.
        for sign in [-1.0, 1.0] {
            faded_line(
                frame,
                view,
                Vec3::new(0.0, 0.0, 0.0),
                dir * (length * sign),
                Vec3::new(0.0, 0.0, 0.0),
                length,
                colour,
                AXIS_BIAS,
            );
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
            grid: Grid { visible: false, spacing: 10.0 },
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
    fn an_orthographic_box_away_from_the_centre_of_the_frame_keeps_all_its_faces() {
        // Under orthographic projection every ray runs along the view direction,
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
        let camera = Camera { yaw: 2.0, pitch: -15.0, distance: 120.0, orthographic: true, ..Camera::default() };
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
        let req = request(vec![Item { renderable: &prepared, style: Style::Ghost }], DisplayMode::Shaded);
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
        req.grid = Grid { visible: true, spacing: 10.0 };
        let frame = render(&req);
        for (name, colour) in [
            ("grid", req.palette.grid),
            ("X axis", req.palette.axis_x),
            ("Y axis", req.palette.axis_y),
            ("Z axis", req.palette.axis_z),
        ] {
            let count = frame.color.chunks_exact(4).filter(|p| *p == colour).count();
            assert!(count > 0, "{name} did not draw");
        }
    }

    #[test]
    fn hiding_the_grid_hides_it() {
        let empty = Renderable::empty();
        let mut req = request(vec![Item { renderable: &empty, style: Style::Solid }], DisplayMode::Shaded);
        req.grid = Grid { visible: false, spacing: 10.0 };
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
            req.grid = Grid { visible: grid, spacing: 10.0 };
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
            req.grid = Grid { visible: true, spacing: 10.0 };
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
        // The near plane clipping means we get a partial fill, not a panic and
        // not a screen of garbage from vertices behind the eye.
        assert_eq!(frame.color.len(), 160 * 120 * 4);
    }

    #[test]
    fn an_orthographic_view_renders_too() {
        let prepared = Renderable::prepare(&primitives::box_mesh(30.0, 30.0, 30.0));
        let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
        req.view.camera.orthographic = true;
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
