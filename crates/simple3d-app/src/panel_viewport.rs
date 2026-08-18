//! The viewport panel: navigation, picking, the manipulator overlay and the
//! bounding-box readout (spec sections 6.1, 6.2).
//!
//! The shaded image comes from the software rasterizer as a texture; the
//! manipulator, the bounding box and the drag readout are drawn on top with the
//! toolkit's 2D painter, so they are always visible and can be hovered.

use crate::app::{App, Status};
use crate::gizmo::{self, Gizmo, Handle, Mods};
use crate::pick;
use crate::render::{self, Grid, Item, Palette, Style};
use crate::theme::{self, token};
use crate::view::View;

use simple3d_core::keymap::{MouseButton, NavMap};
use simple3d_core::scene::{Camera, NodeId};
use simple3d_geom::Vec3;
use std::hash::{Hash, Hasher};

pub fn show(app: &mut App, ctx: &egui::Context) {
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
        let rect = ui.available_rect_before_wrap();
        app.viewport_rect = rect;
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

        let dark = ui.visuals().dark_mode;
        paint_scene(app, ui, rect, dark);
        let view = app.current_view();
        // The cube gets the pointer before the viewport does, or a click on a
        // face would also orbit the camera it just turned.
        let taken = view_cube(app, ui, rect, &view);
        if !taken {
            navigate(app, ui, &response);
            place_cursor(app, ui, &response, &view);
            let view = app.current_view();
            let owned = manipulate(app, ui, &response, &view);
            // Picking is *outside* the manipulator, because it has to work when
            // there is no manipulator: with nothing selected there is no primary
            // node and no gizmo, and while this lived inside `manipulate` the
            // first click into an empty selection was thrown away. Clicking a
            // shape is how most people select one, so it cannot depend on
            // already having selected one.
            if !owned && response.clicked_by(egui::PointerButton::Primary) {
                select_under_cursor(app, ui, &view);
            }
        }
        let view = app.current_view();
        overlays(app, ui, rect, &view);
    });
}

/// Rasterize the scene into a texture, reusing the last image while nothing that
/// affects it has changed.
fn paint_scene(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect, dark: bool) {
    let pixels_per_point = ui.ctx().pixels_per_point();
    let size = [
        (rect.width() * pixels_per_point).round().max(1.0) as usize,
        (rect.height() * pixels_per_point).round().max(1.0) as usize,
    ];

    let key = image_key(app, size, dark);
    if key != app.image_key || app.texture.is_none() {
        let palette = Palette::for_dark_mode(dark);
        // The framebuffer's own coordinate space: its origin is its top-left
        // corner and its unit is the pixel, not the panel's position on screen in
        // points. Handing the rasterizer the panel rect instead would offset
        // every projected vertex by the panel's position and scale it by the
        // wrong factor -- the model would sit away from its own manipulator.
        let render_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size[0] as f32, size[1] as f32));
        let view = View::new(app.scene.camera, render_rect);
        // A hidden node is hidden: no body, and no selection outline drawn
        // around the body it does not have. Selecting it still gets a
        // manipulator, so it can be put where it belongs before being shown.
        let selected: Vec<NodeId> = app
            .top_level_selection()
            .into_iter()
            .flat_map(|id| std::iter::once(id).chain(app.scene.descendants(id)))
            .filter(|&id| app.scene.is_shown(id))
            .collect();

        let mut items: Vec<Item> = vec![Item { renderable: &app.scene_renderable, style: Style::Solid }];
        // Ghosts before the selection outline, so the outline stays readable.
        if app.settings.show_ghosts {
            for (id, renderable) in &app.node_renderables {
                if !app.scene.is_shown(*id) {
                    items.push(Item { renderable, style: Style::Ghost });
                }
            }
        }
        for id in &selected {
            if let Some(renderable) = app.node_renderables.get(id) {
                items.push(Item { renderable, style: Style::Selected });
            }
        }

        let request = render::Request {
            view,
            size,
            mode: app.settings.display_mode,
            palette,
            grid: Grid { visible: app.scene.settings.grid_visible, spacing: app.scene.settings.grid_spacing },
            items,
        };
        let frame = render::render(&request);
        let image = frame.to_color_image();
        match &mut app.texture {
            Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
            None => app.texture = Some(ui.ctx().load_texture("viewport", image, egui::TextureOptions::LINEAR)),
        }
        app.image_key = key;
    }

    if let Some(texture) = &app.texture {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
}

fn image_key(app: &App, size: [usize; 2], dark: bool) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    size.hash(&mut hasher);
    dark.hash(&mut hasher);
    app.evaluation_generation.hash(&mut hasher);
    app.renderable_key.hash(&mut hasher);
    (app.settings.display_mode as u8).hash(&mut hasher);
    app.scene.settings.grid_visible.hash(&mut hasher);
    app.scene.settings.grid_spacing.to_bits().hash(&mut hasher);
    let camera = app.scene.camera;
    for value in
        [camera.target.x, camera.target.y, camera.target.z, camera.distance, camera.yaw, camera.pitch, camera.fov_deg]
    {
        value.to_bits().hash(&mut hasher);
    }
    camera.orthographic.hash(&mut hasher);
    hasher.finish()
}

/// What a drag on the viewport means under the current navigation bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Orbit,
    Pan,
}

/// Which navigation gesture a drag is, under `nav`. `held` says which buttons the
/// drag is using, in `MouseButton` order.
///
/// Split out of `navigate` so acceptance criterion 29 -- remapping the orbit
/// button and having navigation follow immediately -- can be asserted. Note that
/// it takes `nav` as an argument and holds no state of its own: that is *why*
/// there is nothing to restart, and a cached copy here is what would break it.
pub fn nav_gesture(nav: &NavMap, held: [bool; 3], ctrl: bool, shift: bool, alt: bool) -> Option<Gesture> {
    for (button, down) in [MouseButton::Left, MouseButton::Middle, MouseButton::Right].into_iter().zip(held) {
        if !down {
            continue;
        }
        // Pan is tested first: it usually carries an extra modifier on top of
        // orbit's binding, and the exact-match rule keeps them apart.
        if nav.pan.matches(button, ctrl, shift, alt) {
            return Some(Gesture::Pan);
        }
        if nav.orbit.matches(button, ctrl, shift, alt) {
            return Some(Gesture::Orbit);
        }
    }
    None
}

/// Apply a navigation gesture to the camera. `view` is only consulted for a pan,
/// which has to know how many millimetres a pixel covers at the target.
pub fn apply_gesture(camera: &mut Camera, gesture: Gesture, delta: egui::Vec2, view: &View) {
    match gesture {
        Gesture::Orbit => {
            camera.yaw -= delta.x as f64 * 0.4;
            camera.pitch = (camera.pitch + delta.y as f64 * 0.4).clamp(-89.9, 89.9);
        }
        Gesture::Pan => {
            let (right, up) = view.basis();
            let scale = view.mm_per_pixel_at(camera.target);
            camera.target = camera.target - right * (delta.x as f64 * scale) + up * (delta.y as f64 * scale);
        }
    }
}

/// Apply a wheel scroll to the camera's distance, honouring the invert-zoom
/// binding.
pub fn apply_zoom(camera: &mut Camera, nav: &NavMap, scroll: f32) {
    if scroll.abs() <= 0.01 {
        return;
    }
    let direction = if nav.invert_zoom { -1.0 } else { 1.0 };
    let factor = (-scroll as f64 * direction * 0.0015).exp();
    camera.distance = (camera.distance * factor).clamp(0.05, 5.0e6);
}

/// Orbit, pan and zoom, all on remappable bindings that take effect immediately
/// (spec section 8.2, acceptance criterion 29). The bindings are read from the
/// keymap on every frame, so a rebinding applies to the very next drag.
fn navigate(app: &mut App, ui: &mut egui::Ui, response: &egui::Response) {
    let nav = app.keymap.nav;
    let (ctrl, shift, alt) = ui.input(|i| (i.modifiers.command, i.modifiers.shift, i.modifiers.alt));

    let held = [
        response.dragged_by(egui::PointerButton::Primary),
        response.dragged_by(egui::PointerButton::Middle),
        response.dragged_by(egui::PointerButton::Secondary),
    ];
    // A manipulator drag owns the pointer while it is running.
    let gesture = if app.drag.is_some() { None } else { nav_gesture(&nav, held, ctrl, shift, alt) };

    if let Some(gesture) = gesture {
        let view = app.current_view();
        apply_gesture(&mut app.scene.camera, gesture, response.drag_delta(), &view);
    }

    if response.hovered() {
        apply_zoom(&mut app.scene.camera, &nav, ui.input(|i| i.smooth_scroll_delta.y));
    }
}

/// The manipulator: hover highlighting, starting and running a drag, and
/// cancelling it with Escape.
///
/// Returns whether the pointer is the manipulator's this frame, so a click that
/// grabbed a handle does not also re-select whatever is behind it.
fn manipulate(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) -> bool {
    let Some(id) = app.primary() else {
        app.drag = None;
        app.hover_handle = None;
        app.grabbed = None;
        return false;
    };
    let Some(gizmo) = app.gizmo_for(id) else {
        app.hover_handle = None;
        app.grabbed = None;
        return false;
    };
    let is_group = app.scene.node(id).is_group();
    let cursor = ui.input(|i| i.pointer.hover_pos());
    let dragging = app.drag.is_some();

    // The hover hit-test only matters while nothing is being dragged; during a
    // drag the grabbed handle is the one that counts.
    if !dragging {
        app.hover_handle = cursor.and_then(|cursor| gizmo.hit_test(view, cursor, is_group));
    }

    // **What was under the pointer when the button went down**, remembered here
    // and used to start the drag.
    //
    // A drag does not start until the pointer has moved past the toolkit's
    // threshold, and by then the pointer has left the handle it pressed: a
    // handle is grabbable within 9 px and the threshold is most of that. Asking
    // where the pointer is *now* therefore found no handle about half the time,
    // the press was reported as a plain click instead, and the object had to be
    // grabbed again -- sometimes several times over.
    let (pressed, press_origin) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.press_origin()));
    if pressed {
        app.grabbed = press_origin.and_then(|at| gizmo.hit_test(view, at, is_group));
    }

    // Everything the pointer has to say this frame, read off the response in one
    // place so the decision below needs nothing from egui.
    let pointer = gizmo::PointerState {
        escape: ui.input(|i| i.key_pressed(egui::Key::Escape)),
        released: response.drag_stopped() || ui.input(|i| i.pointer.any_released()),
        started: response.drag_started_by(egui::PointerButton::Primary),
        on_handle: app.grabbed.is_some(),
        have_cursor: cursor.is_some(),
    };
    let phase = gizmo::drag_phase(dragging, pointer);
    // The drag is measured from where the button went down, not from where the
    // pointer had already slipped to by the time the toolkit called it a drag.
    let from = if phase == gizmo::DragPhase::Begin { press_origin.or(cursor) } else { cursor };
    app.manipulate_step(&gizmo, view, id, phase, app.grabbed, from, mods_from(ui));

    let owned = app.drag.is_some() || app.grabbed.is_some();
    if pointer.released {
        app.grabbed = None;
    }
    owned
}

fn select_under_cursor(app: &mut App, ui: &mut egui::Ui, view: &View) {
    let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) else { return };
    let (origin, direction) = view.ray(cursor);
    match pick::pick(&app.scene, &app.evaluated, origin, direction) {
        Some(id) => {
            if ui.input(|i| i.modifiers.command || i.modifiers.shift) {
                app.toggle_selected(id);
            } else {
                app.select_only(id);
            }
        }
        None => {
            if !ui.input(|i| i.modifiers.command || i.modifiers.shift) {
                app.clear_selection();
            }
        }
    }
}

fn mods_from(ui: &egui::Ui) -> Mods {
    let (ctrl, shift, alt) = ui.input(|i| (i.modifiers.command, i.modifiers.shift, i.modifiers.alt));
    Mods { free: alt, coarse: shift, symmetric: ctrl }
}

/// Everything drawn over the rasterized image: the manipulator, the bounding
/// boxes with their numeric dimensions, the axis legend and the live drag value.
fn overlays(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect, view: &View) {
    let painter = ui.painter_at(rect);

    if app.settings.show_bounding_box {
        if let Some((lo, hi)) = app.selection_bounds() {
            draw_box(&painter, view, lo, hi, token::ACCENT, 1.5);
            // Dimensions are measurements, so they read in the measure colour,
            // never in the selection's.
            label_box(&painter, ui, view, lo, hi, app.unit(), token::MEASURE);
        }
        if let Some((lo, hi)) = app.evaluated.mesh.bounds() {
            draw_box(&painter, view, lo, hi, token::TEXT_LO.gamma_multiply(0.5), 1.0);
        }
    }

    if let Some(id) = app.primary() {
        if let Some(gizmo) = app.gizmo_for(id) {
            draw_gizmo(app, &painter, ui, &gizmo, view, app.scene.node(id).is_group());
        }
    }

    // The 3D cursor, where the next shape would land.
    draw_cursor(app, &painter, view);

    let hud = format!(
        "{} \u{00B7} {} \u{00B7} {} frame",
        app.mode.label(),
        if app.scene.camera.orthographic { "orthographic" } else { "perspective" },
        app.settings.handle_frame.label().to_lowercase()
    );
    let galley = painter.layout_no_wrap(hud, egui::FontId::proportional(theme::font::SMALL), token::TEXT_LO);
    let at = rect.left_top() + egui::vec2(10.0, 8.0);
    painter.rect_filled(
        egui::Rect::from_min_size(at, galley.size()).expand2(egui::vec2(6.0, 3.0)),
        3.0,
        token::SURFACE_1.gamma_multiply(0.72),
    );
    painter.galley(at, galley, token::TEXT_LO);

    // The live numeric value at the cursor during a drag.
    if let Some(drag) = &app.drag {
        if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
            let at = cursor + egui::vec2(14.0, -18.0);
            // Cyan, in the numeric face: a measurement, not a message.
            let galley = painter.layout_no_wrap(drag.readout.clone(), egui::FontId::monospace(13.0), token::MEASURE);
            let background = egui::Rect::from_min_size(at, galley.size()).expand(5.0);
            painter.rect_filled(background, 3.0, token::SURFACE_1.gamma_multiply(0.92));
            painter.rect_stroke(
                background,
                3.0,
                egui::Stroke::new(1.0_f32, token::MEASURE.gamma_multiply(0.5)),
                egui::StrokeKind::Inside,
            );
            painter.galley(at, galley, token::MEASURE);
        }
    }
}

/// Where a shape would land: the 3D cursor, drawn as a small set of crosshairs
/// so it reads as a position rather than as a piece of the model.
fn draw_cursor(app: &App, painter: &egui::Painter, view: &View) {
    let Some(at) = app.cursor else { return };
    let Some((screen, _)) = view.project(at) else { return };
    let r = 9.0;
    for (dx, dy) in [(1.0, 0.0), (0.0, 1.0)] {
        painter.line_segment(
            [screen - egui::vec2(dx, dy) * r, screen + egui::vec2(dx, dy) * r],
            egui::Stroke::new(1.0_f32, token::MEASURE),
        );
    }
    painter.circle_stroke(screen, r * 0.55, egui::Stroke::new(1.0_f32, token::MEASURE));
}

/// Shift and the orbit button's opposite -- the right button -- puts the cursor
/// on whatever is under the pointer, or on the ground plane when that is
/// nothing. Shift+right-click again on empty space away from the ground puts it
/// back at the origin.
fn place_cursor(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    let shift = ui.input(|i| i.modifiers.shift);
    let pressed = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary));
    if !shift || !pressed {
        return;
    }
    let Some(pointer) = response.interact_pointer_pos().or_else(|| ui.input(|i| i.pointer.hover_pos())) else {
        return;
    };
    // Prefer the surface actually under the pointer: placing a shape against
    // another shape is the reason to move the cursor at all.
    let (origin, dir) = view.ray(pointer);
    let hit = pick::ray_mesh(&app.evaluated.mesh, origin, dir).map(|t| origin + dir * t);
    let at = hit.or_else(|| view.ray_plane(pointer, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)));
    match at {
        Some(at) => {
            let snapped = snap_point(at, app.move_snap());
            app.cursor = Some(snapped);
            app.status = Status::Info(format!("3D cursor at {}", crate::ui::describe_point(snapped, app.unit())));
        }
        None => {
            app.cursor = None;
            app.status = Status::Info("3D cursor back at the origin".into());
        }
    }
}

/// The cursor snaps to the same grid a move does, so a shape placed with it
/// lands on the same numbers a shape moved with the handles does.
pub fn snap_point(p: Vec3, step: f64) -> Vec3 {
    if step <= 0.0 {
        return p;
    }
    Vec3::new((p.x / step).round() * step, (p.y / step).round() * step, (p.z / step).round() * step)
}

/// The orientation cube's id. Fixed rather than derived from the viewport's Ui,
/// so it is the same cube whatever else the panel contains -- and so a test can
/// click a face of it.
pub fn cube_id() -> egui::Id {
    egui::Id::new("view-cube")
}

/// The orientation cube in the bottom-right corner.
///
/// It answers which way the model faces, and it is also the fastest way to
/// change that: a face turns the camera to look at it straight on, the dot at
/// its centre switches between perspective and orthographic. Returns true when
/// it took the pointer, so a click on it does not also orbit.
fn view_cube(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect, view: &View) -> bool {
    let side = theme::metric::VIEW_CUBE;
    let box_rect =
        egui::Rect::from_min_size(rect.right_bottom() - egui::vec2(side + 12.0, side + 12.0), egui::Vec2::splat(side));
    let response = ui.interact(box_rect, cube_id(), egui::Sense::click());
    let painter = ui.painter_at(box_rect.expand(2.0));
    painter.rect_filled(box_rect, 3.0, token::SURFACE_1.gamma_multiply(0.80));
    painter.rect_stroke(box_rect, 3.0, egui::Stroke::new(1.0_f32, token::SURFACE_3), egui::StrokeKind::Inside);

    let centre = box_rect.center();
    let reach = side * 0.30;
    let (yaw, pitch) = (app.scene.camera.yaw, app.scene.camera.pitch);
    let project = |v: Vec3| crate::view::cube_project(yaw, pitch, v, reach);
    let corner = |i: usize| {
        Vec3::new(
            if i & 1 == 0 { -1.0 } else { 1.0 },
            if i & 2 == 0 { -1.0 } else { 1.0 },
            if i & 4 == 0 { -1.0 } else { 1.0 },
        )
    };

    let hover = response.hover_pos();
    let centre_radius = side * 0.11;
    let over_centre = hover.is_some_and(|p| (p - centre).length() < centre_radius);
    // Which face the pointer is over: the nearest one that is turned towards
    // the eye, so a click never asks for the side of the cube you cannot see.
    let hovered_face = match (over_centre, hover) {
        (false, Some(p)) => crate::view::cube_face_at(yaw, pitch, p - centre, reach),
        _ => None,
    };
    let faces: Vec<(usize, egui::Pos2, f64)> = crate::view::CUBE_FACES
        .iter()
        .enumerate()
        .map(|(index, (normal, _, _))| {
            let n = Vec3::new(normal[0] as f64, normal[1] as f64, normal[2] as f64);
            let (offset, depth) = project(n);
            (index, centre + offset, depth)
        })
        .collect();

    // Far faces first, so a near one draws over them.
    let mut order: Vec<usize> = (0..crate::view::CUBE_FACES.len()).collect();
    order.sort_by(|a, b| faces[*b].2.partial_cmp(&faces[*a].2).unwrap_or(std::cmp::Ordering::Equal));
    for index in order {
        let (normal, _, label) = crate::view::CUBE_FACES[index];
        let (_, at, depth) = faces[index];
        if depth >= 0.0 {
            continue;
        }
        // The face as a quad: the four cube corners that share this normal.
        let axis = normal.iter().position(|c| *c != 0).unwrap_or(0);
        let sign = normal[axis] as f64;
        let quad: Vec<egui::Pos2> = (0..8)
            .filter(|i| crate::gizmo::get_axis(corner(*i), axis) * sign > 0.0)
            .map(|i| centre + project(corner(i)).0)
            .collect();
        let quad = sort_ring(quad, at);
        let tint = crate::theme::axis_colour(axis);
        let fill = if hovered_face == Some(index) {
            token::ACCENT.gamma_multiply(0.55)
        } else {
            tint.gamma_multiply(0.16).blend(token::SURFACE_2)
        };
        painter.add(egui::Shape::convex_polygon(
            quad,
            fill,
            egui::Stroke::new(1.0_f32, token::SURFACE_3.gamma_multiply(0.9)),
        ));
        let text_colour = if hovered_face == Some(index) { token::SURFACE_0 } else { token::TEXT_LO };
        // Pushed a little away from the cube's centre: in an isometric view the
        // three visible face centres meet at the near corner, and that is where
        // the projection dot lives.
        let text_at = centre + (at - centre) * 1.2;
        painter.text(text_at, egui::Align2::CENTER_CENTER, label, egui::FontId::monospace(9.0), text_colour);
    }

    // The centre dot: projection. It sits where no face label does, so it never
    // covers one.
    let dot = if over_centre { token::ACCENT } else { token::TEXT_LO };
    painter.circle_filled(centre, centre_radius * 0.45, dot);
    if !app.scene.camera.orthographic {
        painter.circle_stroke(centre, centre_radius * 0.8, egui::Stroke::new(1.0_f32, dot.gamma_multiply(0.6)));
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        if over_centre {
            app.run(simple3d_core::keymap::Command::ToggleProjection);
        } else if let Some(index) = hovered_face {
            app.set_view(crate::view::CUBE_FACES[index].1);
        }
    }
    let _ = view;
    response.hovered() || response.clicked()
}

/// Put the corners of a face in ring order around its centre, so the quad drawn
/// from them is the face and not a bow tie.
fn sort_ring(mut points: Vec<egui::Pos2>, centre: egui::Pos2) -> Vec<egui::Pos2> {
    points.sort_by(|a, b| {
        let angle = |p: &egui::Pos2| (p.y - centre.y).atan2(p.x - centre.x);
        angle(a).partial_cmp(&angle(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    points
}

fn draw_box(painter: &egui::Painter, view: &View, lo: Vec3, hi: Vec3, colour: egui::Color32, width: f32) {
    let corner = |i: usize| {
        Vec3::new(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        )
    };
    const EDGES: [(usize, usize); 12] =
        [(0, 1), (1, 3), (3, 2), (2, 0), (4, 5), (5, 7), (7, 6), (6, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
    for (a, b) in EDGES {
        let (Some((pa, _)), Some((pb, _))) = (view.project(corner(a)), view.project(corner(b))) else {
            continue;
        };
        painter.line_segment([pa, pb], egui::Stroke::new(width, colour));
    }
}

/// The bounding box's dimensions, numerically -- the fastest way to answer "will
/// this fit" (spec section 6.1).
fn label_box(
    painter: &egui::Painter,
    ui: &egui::Ui,
    view: &View,
    lo: Vec3,
    hi: Vec3,
    unit: simple3d_core::unit::Unit,
    colour: egui::Color32,
) {
    let size = hi - lo;
    let centre = (lo + hi) * 0.5;
    let labels = [
        (Vec3::new(centre.x, lo.y, lo.z), simple3d_core::unit::format_length(size.x, unit)),
        (Vec3::new(hi.x, centre.y, lo.z), simple3d_core::unit::format_length(size.y, unit)),
        (Vec3::new(hi.x, lo.y, centre.z), simple3d_core::unit::format_length(size.z, unit)),
    ];
    for (world, text) in labels {
        let Some((screen, _)) = view.project(world) else { continue };
        let galley = painter.layout_no_wrap(format!("{text}{}", unit.suffix()), egui::FontId::monospace(11.0), colour);
        let at = screen + egui::vec2(6.0, -6.0);
        painter.rect_filled(
            egui::Rect::from_min_size(at, galley.size()).expand(2.0),
            2.0,
            ui.visuals().extreme_bg_color.gamma_multiply(0.75),
        );
        painter.galley(at, galley, colour);
    }
}

fn draw_gizmo(app: &App, painter: &egui::Painter, ui: &egui::Ui, gizmo: &Gizmo, view: &View, is_group: bool) {
    let handles = gizmo.handles(is_group);
    if handles.is_empty() {
        if app.mode == gizmo::Mode::Resize {
            // Say why, rather than leaving the user wondering.
            let Some((origin, _)) = view.project(gizmo.origin) else { return };
            painter.text(
                origin + egui::vec2(12.0, 12.0),
                egui::Align2::LEFT_TOP,
                if is_group { "Groups have no resize handles" } else { "This shape has no resizable axis" },
                egui::FontId::proportional(11.0),
                ui.visuals().weak_text_color(),
            );
        }
        return;
    }
    let Some((origin, _)) = view.project(gizmo.origin) else { return };
    let active = app.drag.as_ref().map(|d| d.handle);
    let highlight = |handle: Handle| active == Some(handle) || (active.is_none() && app.hover_handle == Some(handle));

    for handle in handles {
        let axes = handle.axes();
        let colour = if axes.len() == 1 { gizmo::axis_colour(axes[0]) } else { egui::Color32::from_rgb(200, 200, 210) };
        let colour = if highlight(handle) { egui::Color32::from_rgb(255, 214, 96) } else { colour };
        match handle {
            Handle::MoveAxis(_) => {
                let Some((tip, _)) = view.project(gizmo.handle_point(handle, view)) else { continue };
                painter.line_segment([origin, tip], egui::Stroke::new(2.0_f32, colour));
                painter.circle_filled(tip, if highlight(handle) { 6.0 } else { 4.5 }, colour);
            }
            Handle::MovePlane(axis) => {
                let Some((corner, _)) = view.project(gizmo.handle_point(handle, view)) else { continue };
                let (u, v) = (axes[0], axes[1]);
                let arm = gizmo.arm(view) * crate::gizmo::PLANE_FRACTION;
                let Some((pu, _)) = view.project(gizmo.origin + gizmo.axes[u] * arm) else { continue };
                let Some((pv, _)) = view.project(gizmo.origin + gizmo.axes[v] * arm) else { continue };
                let fill = gizmo::axis_colour(axis).gamma_multiply(if highlight(handle) { 0.55 } else { 0.25 });
                painter.add(egui::Shape::convex_polygon(
                    vec![origin, pu, corner, pv],
                    fill,
                    egui::Stroke::new(1.0_f32, colour),
                ));
            }
            Handle::RotateRing(axis) => {
                let points: Vec<egui::Pos2> = gizmo
                    .ring_points(axis, view, 64)
                    .into_iter()
                    .filter_map(|p| view.project(p).map(|(screen, _)| screen))
                    .collect();
                if points.len() > 2 {
                    painter.add(egui::Shape::closed_line(points, egui::Stroke::new(2.0_f32, colour)));
                }
            }
            Handle::ResizeFace(_, _) => {
                let Some((at, _)) = view.project(gizmo.handle_point(handle, view)) else { continue };
                let size = if highlight(handle) { 5.5 } else { 4.0 };
                painter.rect_filled(egui::Rect::from_center_size(at, egui::Vec2::splat(size * 2.0)), 1.0, colour);
            }
            Handle::ResizeCorner(_) => {
                let Some((at, _)) = view.project(gizmo.handle_point(handle, view)) else { continue };
                let size = if highlight(handle) { 5.0 } else { 3.5 };
                painter.circle(at, size, egui::Color32::TRANSPARENT, egui::Stroke::new(2.0_f32, colour));
            }
        }
    }
    painter.circle_filled(origin, 3.0, ui.visuals().strong_text_color());
}

#[cfg(test)]
mod tests {
    use super::*;
    use simple3d_core::keymap::{Drag as NavDrag, Keymap};

    fn only(button: MouseButton) -> [bool; 3] {
        [button == MouseButton::Left, button == MouseButton::Middle, button == MouseButton::Right]
    }

    fn view_of(camera: Camera) -> View {
        View::new(camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0)))
    }

    /// Spec acceptance criterion 29: remap the orbit mouse button and navigation
    /// follows the new binding immediately, without a restart.
    ///
    /// "Without a restart" is the load-bearing half, and it is a property of
    /// where the binding is read from: `navigate` passes the live `NavMap` in on
    /// every frame rather than caching one. The test mutates the same keymap it
    /// already queried and asserts the answer changes.
    #[test]
    fn remapping_the_orbit_button_takes_effect_without_a_restart() {
        let mut keymap = Keymap::default();
        let original = keymap.nav.orbit;
        assert_eq!(nav_gesture(&keymap.nav, only(original.button), false, false, false), Some(Gesture::Orbit));

        // Pick a button the default map does not already use for orbit.
        let remapped = [MouseButton::Left, MouseButton::Middle, MouseButton::Right]
            .into_iter()
            .find(|&b| b != original.button && !keymap.nav.pan.matches(b, false, false, false))
            .expect("a free button");
        keymap.nav.orbit = NavDrag::new(remapped);

        // The very next drag follows the new binding -- no reload of anything.
        assert_eq!(nav_gesture(&keymap.nav, only(remapped), false, false, false), Some(Gesture::Orbit));
        assert_ne!(
            nav_gesture(&keymap.nav, only(original.button), false, false, false),
            Some(Gesture::Orbit),
            "the old button still orbits"
        );

        // And the camera really moves on the new binding, through the same call
        // `navigate` makes.
        let mut camera = Camera::default();
        let before = camera.yaw;
        let gesture = nav_gesture(&keymap.nav, only(remapped), false, false, false).unwrap();
        let view = view_of(camera);
        apply_gesture(&mut camera, gesture, egui::vec2(30.0, 0.0), &view);
        assert_ne!(camera.yaw, before, "orbiting on the new binding did not turn the camera");
    }

    /// The same immediacy for pan and for the invert-zoom switch, and the rule
    /// that keeps pan and orbit apart when pan is orbit's chord plus a modifier.
    #[test]
    fn pan_wins_over_orbit_on_the_same_button_with_a_modifier() {
        let mut keymap = Keymap::default();
        keymap.nav.orbit = NavDrag::new(MouseButton::Right);
        keymap.nav.pan = NavDrag::with_shift(MouseButton::Right);

        assert_eq!(nav_gesture(&keymap.nav, only(MouseButton::Right), false, false, false), Some(Gesture::Orbit));
        assert_eq!(nav_gesture(&keymap.nav, only(MouseButton::Right), false, true, false), Some(Gesture::Pan));
        // A modifier neither binding asks for matches nothing, rather than
        // falling back to orbit.
        assert_eq!(nav_gesture(&keymap.nav, only(MouseButton::Right), true, false, false), None);
        assert_eq!(nav_gesture(&keymap.nav, only(MouseButton::Left), false, false, false), None);
        assert_eq!(nav_gesture(&keymap.nav, [false; 3], false, false, false), None);
    }

    #[test]
    fn inverting_the_zoom_reverses_which_way_the_wheel_goes() {
        let mut keymap = Keymap::default();
        keymap.nav.invert_zoom = false;
        let mut normal = Camera::default();
        apply_zoom(&mut normal, &keymap.nav, 10.0);

        keymap.nav.invert_zoom = true;
        let mut inverted = Camera::default();
        apply_zoom(&mut inverted, &keymap.nav, 10.0);

        let start = Camera::default().distance;
        assert_ne!(normal.distance, start);
        assert!(
            (normal.distance - start).signum() != (inverted.distance - start).signum(),
            "inverting the zoom did not reverse it: {} vs {}",
            normal.distance,
            inverted.distance
        );
        // Wheel noise below the threshold does nothing at all.
        let mut still = Camera::default();
        apply_zoom(&mut still, &keymap.nav, 0.001);
        assert_eq!(still.distance, start);
    }
}
