//! The viewport panel: navigation, picking, the manipulator overlay and the
//! bounding-box readout (spec sections 6.1, 6.2).
//!
//! The shaded image comes from the software rasterizer as a texture; the
//! manipulator, the bounding box and the drag readout are drawn on top with the
//! toolkit's 2D painter, so they are always visible and can be hovered.

use crate::app::{App, Measurement, Status};
use crate::gizmo::{self, Gizmo, Handle, Mods};
use crate::pick;
use crate::render::{self, Grid, Item, Palette, Style};
use crate::theme::{self, token};
use crate::view::View;

use simple3d_core::keymap::{Command, MouseButton, NavMap};
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
            // The measure tool owns the pointer while it is out: clicks pick
            // features to measure between rather than selecting or manipulating.
            if app.measure.active {
                measure_interact(app, ui, &response, &view);
            } else {
                place_cursor(app, ui, &response, &view);
                let view = app.current_view();
                // A pattern's lay-out grips take the pointer before the
                // manipulator, so dragging one lays the copies out rather than
                // moving the whole pattern (issue 67).
                let grips_owned = pattern_grips_interact(app, ui, &view);
                let owned = grips_owned || manipulate(app, ui, &response, &view);
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
        }
        let view = app.current_view();
        overlays(app, ui, rect, &view);
        // The tool's own preview, over everything else the viewport draws: the
        // cells are what the window is asking about, and a manipulator handle
        // across them is a handle the eye reads as part of the grid (issue 82).
        crate::split_tool::preview(app, &ui.painter_at(rect), &view);
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
        // Ticked pieces are outlined like a selection: they are what Extract is
        // about to act on, and a list of two thousand names says nothing about
        // which part of the shape each one is (issue 82).
        let selected: Vec<NodeId> = app
            .top_level_selection()
            .into_iter()
            .chain(app.piece_ticks.iter().copied())
            .filter(|&id| app.scene.is_shown(id))
            .collect();

        // While a tool draws a preview, the document says what the viewport
        // does under it: nothing, drop the axes, drop the grid, or drop
        // everything but the object being previewed (issue 82).
        let preview = app.preview_subject();
        let mode = app.scene.settings.preview_viewport;
        let solid = match preview.filter(|_| !mode.keeps_other_bodies()).and_then(|id| app.node_renderables.get(&id)) {
            // The previewed object alone, drawn as the model rather than as an
            // outline: everything else in the scene is out of the picture, so
            // what is left has to be the picture.
            Some(only) => only,
            None => &app.scene_renderable,
        };
        let mut items: Vec<Item> = vec![Item { renderable: solid, style: Style::Solid }];
        // Ghosts before the selection outline, so the outline stays readable.
        let ghosts = app.ghosts();
        for (id, renderable) in &app.node_renderables {
            // A ghost group shows its own children as ghosts, so a whole
            // assembly can be positioned before it is subtracted.
            if ghosts.iter().any(|&g| g == *id || app.scene.is_ancestor_of(g, *id)) {
                items.push(Item { renderable, style: Style::Ghost });
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
            grid: Grid {
                visible: app.scene.settings.grid_visible && (preview.is_none() || mode.keeps_grid()),
                spacing: app.scene.settings.grid_spacing,
                axes: match preview.is_none() || mode.keeps_axes() {
                    true => app.scene.settings.axes_visible,
                    false => [false; 3],
                },
                style: app.scene.settings.axis_style,
                plane_marks: app.scene.settings.plane_marks,
            },
            items,
        };
        // One preparation, whichever engine draws it: the projection, the
        // shading, the grid's falloff and the axis rule are settled here and
        // the engine only turns the result into pixels.
        let prepared = render::prepare_frame(&request);
        match app.gpu.as_mut() {
            Some(gpu) => match gpu.render(&request, &prepared) {
                Ok(id) => app.gpu_texture = Some(id),
                Err(why) => {
                    // The driver said no. Say so once, and go on drawing in
                    // software rather than showing nothing.
                    app.gpu_error = Some(why);
                    app.gpu = None;
                    app.gpu_texture = None;
                }
            },
            None => app.gpu_texture = None,
        }
        if app.gpu_texture.is_none() {
            let image = render::render_prepared(&request, &prepared).to_color_image();
            match &mut app.texture {
                Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
                None => app.texture = Some(ui.ctx().load_texture("viewport", image, egui::TextureOptions::LINEAR)),
            }
        }
        app.image_key = key;
    }

    // The GPU renderer draws into an OpenGL texture egui was handed once; the
    // software one uploads a fresh image. From here on they are the same thing:
    // a texture painted over the panel.
    let drawn = app.gpu_texture.or_else(|| app.texture.as_ref().map(|texture| texture.id()));
    if let Some(id) = drawn {
        ui.painter().image(
            id,
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
    // A preview opening or closing changes what is drawn under it, so the frame
    // has to be redrawn for it -- and so does a change to the setting that says
    // what (issue 82).
    app.preview_subject().hash(&mut hasher);
    app.scene.settings.preview_viewport.hash(&mut hasher);
    app.scene.settings.grid_visible.hash(&mut hasher);
    app.scene.settings.grid_spacing.to_bits().hash(&mut hasher);
    app.scene.settings.axes_visible.hash(&mut hasher);
    app.scene.settings.axis_style.hash(&mut hasher);
    app.scene.settings.plane_marks.hash(&mut hasher);
    let camera = app.scene.camera;
    for value in
        [camera.target.x, camera.target.y, camera.target.z, camera.distance, camera.yaw, camera.pitch, camera.fov_deg]
    {
        value.to_bits().hash(&mut hasher);
    }
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
///
/// `anchor` is the panel the wheel turned over and where the pointer was in it.
/// Given one, the zoom is about the pointer: whatever is under it stays under
/// it, so zooming in on a corner of the model walks the view towards that
/// corner instead of pulling the frame centre in and leaving the corner off the
/// side. The other half of moving freely about the grid (issue 72), pan being
/// the first. Pass `None` to zoom about the frame centre.
///
/// The projection is parallel, which is what makes this a two-line move rather
/// than a raycast: every world point along the pointer's ray sits at the same
/// offset from the target across the screen, so there is no depth to resolve
/// and no reference plane to choose. Zooming with the pointer over empty sky
/// works the same as zooming with it over the model.
pub fn apply_zoom(camera: &mut Camera, nav: &NavMap, scroll: f32, anchor: Option<(egui::Rect, egui::Pos2)>) {
    if scroll.abs() <= 0.01 {
        return;
    }
    let before = *camera;
    let direction = if nav.invert_zoom { -1.0 } else { 1.0 };
    let factor = (-scroll as f64 * direction * 0.0015).exp();
    camera.distance = (camera.distance * factor).clamp(0.05, 5.0e6);

    let Some((rect, cursor)) = anchor else {
        return;
    };
    // A panel with no area has no centre to measure the pointer from, and the
    // arithmetic below would hand the camera a target of NaN it could never be
    // steered back from.
    if !(rect.width() > 0.0 && rect.height() > 0.0) {
        return;
    }
    // The *realised* factor, not the asked-for one: at either end of the
    // distance range the clamp holds the zoom still, and the view has to hold
    // still with it rather than sliding sideways under a scroll that did
    // nothing.
    let factor = camera.distance / before.distance;
    let view = View::new(before, rect);
    let (right, up) = view.basis();
    let scale = view.mm_per_pixel_at(before.target);
    // Where the pointer is, as an offset from the target across the screen, in
    // millimetres. Screen y grows downward.
    let across = (cursor.x - view.centre.x) as f64 * scale;
    let upward = (view.centre.y - cursor.y) as f64 * scale;
    // That offset shrinks with the zoom, so the target moves the rest of the
    // way in to leave the point on screen where it was.
    camera.target = camera.target + (right * across + up * upward) * (1.0 - factor);
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

    // A locked view centre pins the point the camera turns about (`AppSettings::
    // lock_view_centre`). Orbit and zoom still work -- they are what the pin is
    // for -- but the two gestures that carry the point itself leave it alone: a
    // pan, and the zoom's walk towards the pointer, which falls back to zooming
    // about the frame centre.
    let locked = app.settings.lock_view_centre;
    if let Some(gesture) = gesture {
        if locked && gesture == Gesture::Pan {
            // Said out loud, or a drag that does nothing reads as a drag that
            // does not work.
            app.status = Status::Info("The view centre is locked, so the pan moved nothing".into());
        } else {
            let view = app.current_view();
            apply_gesture(&mut app.scene.camera, gesture, response.drag_delta(), &view);
        }
    }

    if response.hovered() {
        let (scroll, at) = ui.input(|i| (i.smooth_scroll_delta.y, i.pointer.hover_pos()));
        let rect = app.viewport_rect;
        let anchor = if locked { None } else { at.map(|at| (rect, at)) };
        apply_zoom(&mut app.scene.camera, &nav, scroll, anchor);
    }
}

/// The manipulator: hover highlighting, starting and running a drag, and
/// cancelling it with Escape.
///
/// Returns whether the pointer is the manipulator's this frame, so a click that
/// grabbed a handle does not also re-select whatever is behind it.
fn manipulate(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) -> bool {
    // Whether a move drag should snap to geometry this frame (issue 68), read
    // from the setting and, for the hold mode, the live key state. Read here on
    // every frame so a change of mode -- or the key going down mid-drag -- takes
    // effect at once.
    app.snap_requested = app.geometry_snap_wanted(|k| ui.input(|i| i.key_down(k)), ui.input(|i| i.modifiers));
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
    // A split counts as a group here: it has no dimensions of its own either,
    // and its pieces are meshes, which have none at all.
    let is_group = app.scene.node(id).is_group() || app.scene.node(id).is_split();
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

/// The lay-out grips of the selected pattern, if it is one (issue 67).
///
/// Every kind offers its own: the spacing and the number of copies along each
/// straight run, the radius and the span of a ring, the radius, rise and length
/// of a helix, the radii of a spiral. A mirror offers none -- it has neither a
/// distance nor a count -- and neither does a pattern whose numbers put every
/// grip on its own origin, where the move manipulator already is.
fn pattern_grips(app: &App) -> Vec<crate::app::PatternGrip> {
    match app.primary() {
        Some(id) => app.pattern_grips(id),
        None => Vec::new(),
    }
}

/// Drag a pattern's grips to lay it out by eye. Returns whether the pointer is
/// theirs this frame.
///
/// Each grip is its own widget, keyed by its label rather than its position in
/// the list: dragging the "Copies" grip *adds* copies, which can add a "Spacing"
/// grip beside it, and an index would shift out from under the drag that caused
/// it.
fn pattern_grips_interact(app: &mut App, ui: &mut egui::Ui, view: &View) -> bool {
    let Some(id) = app.primary() else { return false };
    let mut owned = false;
    for grip in pattern_grips(app) {
        let Some((screen, _)) = view.project(grip.at) else { continue };
        let rect = egui::Rect::from_center_size(screen, egui::Vec2::splat(16.0));
        let response = ui
            .interact(rect, ui.id().with((id, "pattern-grip", grip.label)), egui::Sense::drag())
            .on_hover_text(grip.label);
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(match grip.turn {
                // A turn is not a push or a pull, and there is no cursor for
                // "round": the hand says the grip is held and the arc under it
                // says which way it goes.
                Some(_) => egui::CursorIcon::Grabbing,
                // Every other grip slides along a line, and the pointer says
                // which line: an arrow across the screen for a run that lies
                // across it, and up the screen for one that stands up.
                None => slide_cursor(screen_direction(view, grip.at, grip.dir)),
            });
        }
        if response.dragged() {
            if let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) {
                if let Some(value) = app.pattern_grip_value(&grip, view, cursor) {
                    app.set_pattern_grip(id, grip.label, value, mods_from(ui));
                }
            }
        }
        owned |= response.dragged() || response.hovered();
    }
    owned
}

/// Which way a world direction runs on screen, at `at`. A zero vector where the
/// line does not project -- behind the eye, or edge on.
fn screen_direction(view: &View, at: Vec3, dir: Vec3) -> egui::Vec2 {
    // A millimetre along the line is enough to take its bearing and short
    // enough that the answer is about the line at `at` rather than about where
    // it ends up.
    match (view.project(at), view.project(at + dir)) {
        (Some((a, _)), Some((b, _))) => b - a,
        _ => egui::Vec2::ZERO,
    }
}

/// The resize cursor that matches a direction on screen, so a handle says which
/// way it will move before it is grabbed rather than always claiming to slide
/// left and right.
///
/// The four cursors cover the half-circle in 45-degree sectors, and a line has
/// no sense of direction -- pushing and pulling along it are the same
/// gesture -- so the bearing is folded into that half-circle first. Screen y
/// grows downward, which is why "right and down" is the north-west/south-east
/// diagonal rather than the other one.
pub fn slide_cursor(along: egui::Vec2) -> egui::CursorIcon {
    if along.length_sq() < 1e-6 {
        return egui::CursorIcon::ResizeHorizontal;
    }
    let bearing = along.y.atan2(along.x).to_degrees().rem_euclid(180.0);
    match bearing {
        b if !(22.5..157.5).contains(&b) => egui::CursorIcon::ResizeHorizontal,
        b if b < 67.5 => egui::CursorIcon::ResizeNwSe,
        b if b < 112.5 => egui::CursorIcon::ResizeVertical,
        _ => egui::CursorIcon::ResizeNeSw,
    }
}

/// Draw the selected pattern's lay-out grips: a leader line from the centre out
/// to each one, and a diamond on the grip itself. A span grip is drawn as an arc
/// round the ring it sets instead, since what it measures is the turn and not a
/// distance.
fn draw_pattern_grips(app: &App, painter: &egui::Painter, view: &View) {
    for grip in pattern_grips(app) {
        let Some((at, _)) = view.project(grip.at) else { continue };
        match grip.turn {
            Some((axis, zero, radius)) => {
                let tangent = axis.cross(zero);
                let mut arc: Vec<egui::Pos2> = Vec::new();
                // The whole way round to the grip, in one-degree steps, so the
                // arc shows the span the copies actually fill.
                let end = grip.at - grip.from;
                let span = end.dot(tangent).atan2(end.dot(zero)).to_degrees();
                let span = if span <= 0.0 { span + 360.0 } else { span };
                let steps = (span.abs().ceil() as usize).max(1);
                for i in 0..=steps {
                    let a = (span * i as f64 / steps as f64).to_radians();
                    let p = grip.from + zero * (radius * a.cos()) + tangent * (radius * a.sin());
                    if let Some((screen, _)) = view.project(p) {
                        arc.push(screen);
                    }
                }
                painter.add(egui::Shape::line(arc, egui::Stroke::new(1.0_f32, token::ACCENT.gamma_multiply(0.6))));
            }
            None => {
                if let Some((from, _)) = view.project(grip.from) {
                    painter.line_segment([from, at], egui::Stroke::new(1.0_f32, token::ACCENT.gamma_multiply(0.6)));
                }
            }
        }
        let r = 6.0;
        painter.add(egui::Shape::convex_polygon(
            vec![at + egui::vec2(0.0, -r), at + egui::vec2(r, 0.0), at + egui::vec2(0.0, r), at + egui::vec2(-r, 0.0)],
            token::ACCENT,
            egui::Stroke::NONE,
        ));
    }
}

/// The measure tool's pointer handling: a click drops a point, snapped to the
/// nearest feature within reach, and Escape clears the span or puts the tool
/// away (issue 69).
fn measure_interact(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        if app.measure.points.is_empty() {
            app.run(Command::MeasureTool);
        } else {
            app.measure.clear();
            app.status = Status::Info("Measurement cleared".into());
        }
        return;
    }
    // A right-click takes the last placed end back off, so a point put down in
    // the wrong place is undone where it was made rather than by reaching for
    // the panel. A right *drag* still orbits: only a click that never became one
    // is this gesture.
    if response.clicked_by(egui::PointerButton::Secondary) {
        app.measure_unplace();
        return;
    }
    if response.clicked_by(egui::PointerButton::Primary) {
        if let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) {
            if let Some(point) = app.measure_point_at(view, cursor) {
                app.measure_click(point);
            }
        }
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
}

fn select_under_cursor(app: &mut App, ui: &mut egui::Ui, view: &View) {
    let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) else { return };
    let (origin, direction) = view.ray(cursor);
    let adding = ui.input(|i| i.modifiers.command || i.modifiers.shift);
    match pick::pick(&app.scene, &app.evaluated, origin, direction) {
        Some(hit) => {
            // A click on a piece still held inside a collection means the
            // collection, the way a click anywhere on a pattern's output means
            // the pattern: the piece has no row, and selecting something the
            // tree cannot show is selecting it out of sight (issue 82).
            let id = app.scene.row_for(hit);
            // Unless the collection is already what is selected. Then the click
            // is about the piece, and it ticks it in the panel's list -- which
            // is how a piece out of thousands is found at all: by pointing at
            // it, rather than by reading names off a list.
            if id != hit && app.listed_collection() == Some(id) {
                app.tick_piece(hit, adding);
                return;
            }
            if adding {
                app.toggle_selected(id);
            } else {
                app.select_only(id);
            }
        }
        None => {
            if !adding {
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
            draw_gizmo(app, &painter, ui, &gizmo, view, app.scene.node(id).is_group() || app.scene.node(id).is_split());
        }
    }

    // The 3D cursor, where the next shape would land.
    draw_cursor(app, &painter, view);

    // Where a drag has snapped onto another body's feature (issue 68): a hollow
    // square on the caught point, in the accent so it reads as "this is what it
    // caught" the way a selection does.
    if let Some(at) = app.snap_indicator {
        if let Some((screen, _)) = view.project(at) {
            painter.rect_stroke(
                egui::Rect::from_center_size(screen, egui::Vec2::splat(11.0)),
                1.0,
                egui::Stroke::new(1.5_f32, token::ACCENT),
                egui::StrokeKind::Middle,
            );
        }
    }

    // A pattern's lay-out grips: a diamond on each of the numbers that place its
    // copies, dragged to lay them out by eye rather than by typing (issue 67).
    draw_pattern_grips(app, &painter, view);

    if app.measure.active {
        draw_measure(app, ui, &painter, view);
    }

    // The two things about this viewport that can differ from one moment to the
    // next. The projection was named here as well, and it is always orthographic
    // (`scene::Camera`, where a saved file's `orthographic` flag is read and
    // ignored) -- a word that cannot change is not information, and it was the
    // widest part of a strip whose whole job is to be read at a glance.
    let hud = format!("{} \u{00B7} {} frame", app.mode.label(), app.settings.handle_frame.label().to_lowercase());
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

/// The measure tool's marks: the placed points, the span between them once both
/// are down, its numbers, and -- while one end is placed -- a live line to the
/// feature under the pointer (issue 69). All in the measure colour, because it
/// reads distances and never touches the model.
fn draw_measure(app: &App, ui: &egui::Ui, painter: &egui::Painter, view: &View) {
    let colour = token::MEASURE;
    let mark = |at: Vec3, snapped: bool| {
        if let Some((screen, _)) = view.project(at) {
            // A snapped point gets a hollow square, an on-surface one a small
            // cross, so a glance says whether it caught a feature.
            if snapped {
                painter.rect_stroke(
                    egui::Rect::from_center_size(screen, egui::Vec2::splat(9.0)),
                    1.0,
                    egui::Stroke::new(1.5_f32, colour),
                    egui::StrokeKind::Middle,
                );
            } else {
                let r = 5.0;
                painter.line_segment(
                    [screen - egui::vec2(r, r), screen + egui::vec2(r, r)],
                    egui::Stroke::new(1.5_f32, colour),
                );
                painter.line_segment(
                    [screen - egui::vec2(r, -r), screen + egui::vec2(r, -r)],
                    egui::Stroke::new(1.5_f32, colour),
                );
            }
        }
    };

    for point in &app.measure.points {
        mark(point.at, point.kind.is_some());
    }

    // Where the *next* click would land, marked and named before it is made.
    //
    // This used to be drawn only once one end was down, which is exactly
    // backwards: the first point is the one placed with nothing else on screen
    // to judge it against, and it was placed blind (issue 78). The hover point is
    // resolved by the same call a click makes, so what is shown is what would be
    // taken -- including a point part-way along an edge, or where an axis crosses
    // a body.
    if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
        // Only over the viewport itself: the pointer out over a dock is not
        // aiming at anything in the scene.
        if painter.clip_rect().contains(cursor) {
            if let Some(hover) = app.measure_point_at(view, cursor) {
                mark(hover.at, hover.kind.is_some());
                // Name the feature the pointer has caught, so a snap is legible
                // rather than a guess.
                if let (Some(kind), Some((screen, _))) = (hover.kind, view.project(hover.at)) {
                    painter.text(
                        screen + egui::vec2(11.0, -11.0),
                        egui::Align2::LEFT_BOTTOM,
                        kind.label(),
                        egui::FontId::proportional(10.0),
                        colour,
                    );
                }
                // A live line from the first point, so the second click can be
                // aimed.
                if app.measure.points.len() == 1 {
                    let ends = (view.project(app.measure.points[0].at), view.project(hover.at));
                    if let (Some((a, _)), Some((b, _))) = ends {
                        painter.line_segment([a, b], egui::Stroke::new(1.0_f32, colour.gamma_multiply(0.6)));
                    }
                }
            }
        }
    }

    let Some((a, b)) = app.measure.span() else { return };
    let (Some((sa, _)), Some((sb, _))) = (view.project(a.at), view.project(b.at)) else { return };
    painter.line_segment([sa, sb], egui::Stroke::new(2.0_f32, colour));

    // The numbers, in a small panel by the middle of the span, kept there until
    // the tool is dismissed.
    let m = Measurement::between(a.at, b.at);
    let unit = app.unit();
    let suffix = unit.suffix();
    let fmt = |v: f64| simple3d_core::unit::format_length(v, unit);
    let lines = [
        format!("Distance  {}{suffix}", fmt(m.distance)),
        format!("\u{0394}  {}, {}, {} {suffix}", fmt(m.delta.x), fmt(m.delta.y), fmt(m.delta.z)),
        format!(
            "Incline  {}\u{00B0}   Bearing  {}\u{00B0}",
            simple3d_core::unit::format_angle(m.inclination_deg),
            simple3d_core::unit::format_angle(m.bearing_deg)
        ),
    ];
    let at = ((sa.to_vec2() + sb.to_vec2()) / 2.0).to_pos2() + egui::vec2(10.0, 8.0);
    let mut galleys = Vec::new();
    let mut size = egui::Vec2::ZERO;
    for line in &lines {
        let galley = painter.layout_no_wrap(line.clone(), egui::FontId::monospace(12.0), colour);
        size.x = size.x.max(galley.size().x);
        size.y += galley.size().y;
        galleys.push(galley);
    }
    let background = egui::Rect::from_min_size(at, size).expand(6.0);
    painter.rect_filled(background, 3.0, token::SURFACE_1.gamma_multiply(0.94));
    painter.rect_stroke(
        background,
        3.0,
        egui::Stroke::new(1.0_f32, colour.gamma_multiply(0.5)),
        egui::StrokeKind::Inside,
    );
    let mut y = at.y;
    for galley in galleys {
        let h = galley.size().y;
        painter.galley(egui::pos2(at.x, y), galley, colour);
        y += h;
    }
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
    let at = hit.or_else(|| view.ray_plane_ahead(pointer, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)));
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
/// change that: a face turns the camera to look at it straight on, and the dot
/// at its centre returns to the isometric view the cube is drawn from. Returns
/// true when it took the pointer, so a click on it does not also orbit.
fn view_cube(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect, view: &View) -> bool {
    let side = theme::metric::VIEW_CUBE;
    let box_rect =
        egui::Rect::from_min_size(rect.right_bottom() - egui::vec2(side + 12.0, side + 12.0), egui::Vec2::splat(side));
    // Click *and drag*: the drag turns the cube alone, which is the only way to
    // reach the three sides the camera cannot currently see (issue 34).
    let response = ui.interact(box_rect, cube_id(), egui::Sense::click_and_drag());
    let painter = ui.painter_at(box_rect.expand(2.0));
    painter.rect_filled(box_rect, 3.0, token::SURFACE_1.gamma_multiply(0.80));
    painter.rect_stroke(box_rect, 3.0, egui::Stroke::new(1.0_f32, token::SURFACE_3), egui::StrokeKind::Inside);

    let centre = box_rect.center();
    let reach = side * 0.30;

    // The cube follows the camera unless it has been turned by hand, and a
    // camera that moves takes the cube back with it: a spin is remembered
    // along with the camera it was started from, and is dropped the moment the
    // scene turns underneath it.
    let camera = (app.scene.camera.yaw, app.scene.camera.pitch);
    if app.cube_spin.is_some_and(|spin| spin.camera != camera) {
        app.cube_spin = None;
    }
    if response.dragged() {
        let delta = response.drag_delta();
        let (from_yaw, from_pitch) = app.cube_spin.map_or(camera, |spin| (spin.yaw, spin.pitch));
        app.cube_spin = Some(crate::app::CubeSpin {
            yaw: from_yaw - delta.x as f64 * 0.5,
            pitch: (from_pitch + delta.y as f64 * 0.5).clamp(-89.9, 89.9),
            camera,
        });
    }
    let (yaw, pitch) = app.cube_spin.map_or(camera, |spin| (spin.yaw, spin.pitch));

    let project = |v: Vec3| crate::view::cube_project(yaw, pitch, v, reach);
    let corner = |i: usize| {
        Vec3::new(
            if i & 1 == 0 { -1.0 } else { 1.0 },
            if i & 2 == 0 { -1.0 } else { 1.0 },
            if i & 4 == 0 { -1.0 } else { 1.0 },
        )
    };
    let at_zone = |zone: [i32; 3]| centre + project(Vec3::new(zone[0] as f64, zone[1] as f64, zone[2] as f64)).0;

    let hover = response.hover_pos();
    let centre_radius = side * 0.11;
    let over_centre = hover.is_some_and(|p| (p - centre).length() < centre_radius);
    // Which part of the cube the pointer is over -- a face, an edge or a corner
    // -- among the ones turned towards the eye, so a click never asks for the
    // side of the cube that cannot be seen. A drag in progress is turning the
    // cube, not choosing a view, so nothing is highlighted during one.
    let hovered_zone = match (over_centre, response.dragged(), hover) {
        (false, false, Some(p)) => crate::view::cube_zone_at(yaw, pitch, p - centre, reach),
        _ => None,
    };
    let hovered_face = hovered_zone
        .filter(|zone| crate::view::zone_order(*zone) == 1)
        .and_then(|zone| crate::view::CUBE_FACES.iter().position(|(normal, _, _)| *normal == zone));
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

    // An edge or a corner has no quad of its own, so the highlight is drawn
    // over the faces: the edge as a bar along itself, the corner as a dot on
    // it. Both in the selection colour, which is what "this is what a click
    // would take" means everywhere else in the application.
    if let Some(zone) = hovered_zone {
        match crate::view::zone_order(zone) {
            2 => {
                let axis = zone.iter().position(|c| *c == 0).unwrap_or(0);
                let mut a = zone;
                let mut b = zone;
                a[axis] = -1;
                b[axis] = 1;
                painter.line_segment([at_zone(a), at_zone(b)], egui::Stroke::new(3.5_f32, token::ACCENT));
            }
            3 => {
                painter.circle_filled(at_zone(zone), 4.5, token::ACCENT);
            }
            _ => {}
        }
    }

    // The centre dot: an isometric view, back to where the cube itself is
    // drawn from. It sits where no face label does, so it never covers one.
    let dot = if over_centre { token::ACCENT } else { token::TEXT_LO };
    painter.circle_filled(centre, centre_radius * 0.45, dot);

    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(if response.dragged() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::PointingHand
        });
    }
    let hint = match (over_centre, hovered_zone) {
        (true, _) => Some("Isometric".to_string()),
        (false, Some(zone)) => Some(crate::view::cube_zone_label(zone)),
        _ => None,
    };
    if let Some(hint) = hint {
        response.clone().on_hover_text(format!("{hint}\nDrag the cube to turn it without moving the model"));
    }
    if response.clicked() {
        // Whatever was chosen, the cube goes back to matching the camera: the
        // spin is a way of *reaching* a view, not a second orientation to keep.
        app.cube_spin = None;
        if over_centre {
            app.set_view(crate::view::ViewPreset::Isometric);
        } else if let Some(zone) = hovered_zone {
            let (to_yaw, to_pitch) = crate::view::cube_zone_angles(zone, app.scene.camera.yaw);
            app.turn_camera_to(to_yaw, to_pitch);
            app.status = Status::Info(format!("View: {}", crate::view::cube_zone_label(zone)));
        }
    }
    let _ = view;
    response.hovered() || response.clicked() || response.dragged()
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

    #[test]
    fn a_sliding_handle_points_the_way_it_actually_slides() {
        use egui::CursorIcon::*;
        // Every grip used to claim it slid left and right, whichever way its own
        // run ran -- a pattern stepping straight up the screen still asked for
        // the horizontal arrow.
        let cursor = |x: f32, y: f32| slide_cursor(egui::vec2(x, y));
        assert_eq!(cursor(1.0, 0.0), ResizeHorizontal);
        assert_eq!(cursor(0.0, 1.0), ResizeVertical);
        // Screen y grows downward, so right-and-down is the "\\" diagonal and
        // right-and-up is the "/" one.
        assert_eq!(cursor(1.0, 1.0), ResizeNwSe);
        assert_eq!(cursor(1.0, -1.0), ResizeNeSw);

        // A line has no sense of direction: pushing and pulling along it are the
        // same gesture, so the opposite bearing gives the same cursor.
        for (x, y) in [(1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, -1.0), (3.0, 1.0), (-1.0, 4.0)] {
            assert_eq!(cursor(x, y), cursor(-x, -y), "({x}, {y}) and its opposite disagree");
        }

        // The sectors meet where they should: just off the axis is still the
        // axis, and past the halfway line is the diagonal.
        assert_eq!(cursor(10.0, 3.0), ResizeHorizontal, "17 degrees off flat is still flat");
        assert_eq!(cursor(10.0, 6.0), ResizeNwSe, "31 degrees off flat is the diagonal");
        assert_eq!(cursor(3.0, 10.0), ResizeVertical);

        // A line that does not project -- edge on, or off the screen -- falls
        // back rather than picking a direction out of nothing.
        assert_eq!(cursor(0.0, 0.0), ResizeHorizontal);
    }

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
        apply_zoom(&mut normal, &keymap.nav, 10.0, None);

        keymap.nav.invert_zoom = true;
        let mut inverted = Camera::default();
        apply_zoom(&mut inverted, &keymap.nav, 10.0, None);

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
        apply_zoom(&mut still, &keymap.nav, 0.001, None);
        assert_eq!(still.distance, start);
    }

    /// The other half of moving freely about the grid (issue 72): the wheel
    /// zooms about the pointer, so whatever is under it stays under it. Zooming
    /// about the frame centre is what made the viewport read as bolted down --
    /// a corner of the model you were closing in on slid off the side of the
    /// frame, and it took a pan after every scroll to bring it back.
    #[test]
    fn zooming_keeps_whatever_is_under_the_pointer_under_it() {
        let keymap = Keymap::default();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
        let start = Camera { yaw: -55.0, pitch: 28.0, distance: 300.0, ..Camera::default() };

        for (yaw, pitch) in [(-55.0, 28.0), (-90.0, 0.0), (140.0, -35.0), (-90.0, 89.9)] {
            let start = Camera { yaw, pitch, ..start };
            for offset in [egui::vec2(-260.0, 150.0), egui::vec2(310.0, -210.0), egui::vec2(0.0, -320.0)] {
                let cursor = rect.center() + offset;
                // Any world point on the pointer's ray will do: the projection
                // is parallel, so every point along it lands on the same pixel.
                // The one this picks is on the plane through the eye, which is
                // sky rather than ground for an offset above the horizon --
                // exactly the case a raycast onto the ground would have had no
                // answer for.
                let (under, _) = View::new(start, rect).ray(cursor);
                for scroll in [120.0, -120.0] {
                    let mut camera = start;
                    apply_zoom(&mut camera, &keymap.nav, scroll, Some((rect, cursor)));
                    assert_ne!(camera.distance, start.distance, "the wheel did not zoom at all");
                    let after = View::new(camera, rect).project(under).unwrap().0;
                    assert!(
                        (after - cursor).length() < 0.01,
                        "the point under the pointer slid from {cursor:?} to {after:?} \
                         (yaw {yaw}, pitch {pitch}, scroll {scroll})"
                    );
                }
            }
        }

        // Without an anchor -- the pointer outside the panel, or a caller that
        // has no panel to speak of -- it is the zoom it always was, about the
        // frame centre.
        let cursor = rect.center() + egui::vec2(-260.0, 150.0);
        let mut centred = start;
        apply_zoom(&mut centred, &keymap.nav, 120.0, None);
        assert_ne!(centred.distance, start.distance);
        assert_eq!(centred.target, start.target, "zooming with no pointer to zoom about moved the camera");

        // At either end of the distance range the clamp holds the zoom still,
        // and the view has to hold still with it rather than sliding sideways
        // under a scroll that did nothing.
        let mut pinned = Camera { distance: 0.05, ..start };
        apply_zoom(&mut pinned, &keymap.nav, 120.0, Some((rect, cursor)));
        assert_eq!(pinned.distance, 0.05, "the near end of the range stopped holding");
        assert_eq!(pinned.target, start.target, "a zoom the clamp refused still moved the camera");

        // And a panel with no area is not a place to zoom about: the camera
        // keeps a target it can be steered from rather than being handed NaN.
        let mut sized = start;
        apply_zoom(&mut sized, &keymap.nav, 120.0, Some((egui::Rect::NOTHING, cursor)));
        assert_eq!(sized.target, start.target, "an empty panel moved the camera to {:?}", sized.target);
    }
}
