//! The viewport panel: navigation, picking, the manipulator overlay and the
//! bounding-box readout (spec sections 6.1, 6.2).
//!
//! The shaded image comes from the software rasterizer as a texture; the
//! manipulator, the bounding box and the drag readout are drawn on top with the
//! toolkit's 2D painter, so they are always visible and can be hovered.

use crate::app::{App, Status};
use crate::gizmo::{self, Drag, Gizmo, Handle, Mods};
use crate::pick;
use crate::render::{self, Grid, Item, Palette, Style};
use crate::view::View;
use scadstudio_core::config::DisplayMode;
use scadstudio_core::keymap::MouseButton;
use scadstudio_core::scene::NodeId;
use scadstudio_geom::Vec3;
use std::hash::{Hash, Hasher};

pub fn show(app: &mut App, ctx: &egui::Context) {
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
        let rect = ui.available_rect_before_wrap();
        app.viewport_rect = rect;
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

        let dark = ui.visuals().dark_mode;
        paint_scene(app, ui, rect, dark);
        navigate(app, ui, &response);
        let view = app.current_view();
        manipulate(app, ui, &response, &view);
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
        let selected: Vec<NodeId> = app
            .top_level_selection()
            .into_iter()
            .flat_map(|id| std::iter::once(id).chain(app.scene.descendants(id)))
            .collect();

        let mut items: Vec<Item> = vec![Item { renderable: &app.scene_renderable, style: Style::Solid }];
        // Ghosts before the selection outline, so the outline stays readable.
        if app.settings.show_ghosts {
            for (id, renderable) in &app.node_renderables {
                if !app.scene.get(*id).map(|n| n.visible).unwrap_or(true) {
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

/// Orbit, pan and zoom, all on remappable bindings that take effect immediately
/// (spec section 8.2, acceptance criterion 29).
fn navigate(app: &mut App, ui: &mut egui::Ui, response: &egui::Response) {
    let nav = app.keymap.nav;
    let (ctrl, shift, alt) = ui.input(|i| (i.modifiers.command, i.modifiers.shift, i.modifiers.alt));

    // Which of our bindings is the drag that is happening?
    let mut orbiting = false;
    let mut panning = false;
    for (button, egui_button) in [
        (MouseButton::Left, egui::PointerButton::Primary),
        (MouseButton::Middle, egui::PointerButton::Middle),
        (MouseButton::Right, egui::PointerButton::Secondary),
    ] {
        if !response.dragged_by(egui_button) {
            continue;
        }
        // Pan is tested first: it usually carries an extra modifier on top of
        // orbit's binding, and the exact-match rule keeps them apart.
        if nav.pan.matches(button, ctrl, shift, alt) {
            panning = true;
        } else if nav.orbit.matches(button, ctrl, shift, alt) {
            orbiting = true;
        }
    }
    // A manipulator drag owns the pointer while it is running.
    if app.drag.is_some() {
        orbiting = false;
        panning = false;
    }

    let delta = response.drag_delta();
    if orbiting {
        app.scene.camera.yaw -= delta.x as f64 * 0.4;
        app.scene.camera.pitch = (app.scene.camera.pitch + delta.y as f64 * 0.4).clamp(-89.9, 89.9);
    }
    if panning {
        let view = app.current_view();
        let (right, up) = view.basis();
        let scale = view.mm_per_pixel_at(app.scene.camera.target);
        app.scene.camera.target =
            app.scene.camera.target - right * (delta.x as f64 * scale) + up * (delta.y as f64 * scale);
    }

    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.01 {
            let direction = if nav.invert_zoom { -1.0 } else { 1.0 };
            let factor = (-scroll as f64 * direction * 0.0015).exp();
            app.scene.camera.distance = (app.scene.camera.distance * factor).clamp(0.05, 5.0e6);
        }
    }
}

/// The manipulator: hover highlighting, starting and running a drag, and
/// cancelling it with Escape.
fn manipulate(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    let Some(id) = app.primary() else {
        app.drag = None;
        app.hover_handle = None;
        return;
    };
    let Some(gizmo) = app.gizmo_for(id) else {
        app.hover_handle = None;
        return;
    };
    let is_group = app.scene.node(id).is_group();
    let cursor = ui.input(|i| i.pointer.hover_pos());

    // Escape during a drag cancels it and restores the pre-drag values exactly.
    if app.drag.is_some() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        if let Some(drag) = app.drag.take() {
            drag.cancel(&mut app.scene);
            app.touch();
            app.fields.clear();
            app.status = Status::Info("Drag cancelled".into());
        }
        return;
    }

    if let Some(drag) = &mut app.drag {
        if response.drag_stopped() || ui.input(|i| i.pointer.any_released()) {
            app.drag = None;
            app.history.close();
            app.fields.clear();
            return;
        }
        if let Some(cursor) = cursor {
            let mods = mods_from(ui);
            let snap = app.scene.settings.grid_spacing;
            let rotate_snap = app.settings.rotate_snap_deg;
            let unit = app.scene.settings.unit;
            drag.update(&mut app.scene, &gizmo, view, cursor, mods, snap, rotate_snap, unit);
            // The property editor tracks the handle live, and the preview follows.
            app.fields.clear();
            app.touch();
        }
        return;
    }

    app.hover_handle = cursor.and_then(|cursor| gizmo.hit_test(view, cursor, is_group));

    if response.drag_started_by(egui::PointerButton::Primary) {
        if let (Some(cursor), Some(handle)) = (cursor, app.hover_handle) {
            // One snapshot for the whole drag, so it undoes in a single step.
            app.edit(
                match app.mode {
                    gizmo::Mode::Move => "Move",
                    gizmo::Mode::Rotate => "Rotate",
                    gizmo::Mode::Resize => "Resize",
                },
                None,
            );
            app.drag = Drag::begin(&app.scene, &gizmo, id, handle, view, cursor);
        }
    }

    // A plain click that did not grab a handle selects whatever is under it.
    if response.clicked_by(egui::PointerButton::Primary) && app.hover_handle.is_none() {
        select_under_cursor(app, ui, view);
    }
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
            draw_box(&painter, view, lo, hi, ui.visuals().selection.bg_fill, 1.5);
            label_box(&painter, ui, view, lo, hi, app.unit(), ui.visuals().selection.bg_fill);
        }
        if let Some((lo, hi)) = app.evaluated.mesh.bounds() {
            draw_box(&painter, view, lo, hi, ui.visuals().weak_text_color(), 1.0);
        }
    }

    if let Some(id) = app.primary() {
        if let Some(gizmo) = app.gizmo_for(id) {
            draw_gizmo(app, &painter, ui, &gizmo, view, app.scene.node(id).is_group());
        }
    }

    // Axis legend, in the same colours as the origin axes.
    let mut cursor = rect.left_top() + egui::vec2(10.0, 10.0);
    for (axis, name) in ["X", "Y", "Z"].iter().enumerate() {
        painter.text(cursor, egui::Align2::LEFT_TOP, *name, egui::FontId::monospace(12.0), gizmo::axis_colour(axis));
        cursor.x += 16.0;
    }
    let info_colour = ui.visuals().text_color();
    let mut lines = vec![
        format!(
            "{} - {} - {} frame",
            app.mode.label(),
            if app.scene.camera.orthographic { "orthographic" } else { "perspective" },
            app.settings.handle_frame.label()
        ),
        format!("{}", app.settings.display_mode.label()),
        // Which measurement convention tessellation follows, stated in the
        // interface because it decides whether a printed hole fits.
        "Curves are circumscribed: a diameter of 50 measures 50 at its widest".to_string(),
    ];
    if app.settings.display_mode == DisplayMode::Wireframe {
        lines.push("Wireframe shows creased edges only".into());
    }
    let mut y = rect.left_top().y + 28.0;
    for line in lines {
        painter.text(
            egui::pos2(rect.left() + 10.0, y),
            egui::Align2::LEFT_TOP,
            line,
            egui::FontId::proportional(11.0),
            info_colour,
        );
        y += 15.0;
    }

    // The live numeric value at the cursor during a drag.
    if let Some(drag) = &app.drag {
        if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
            let at = cursor + egui::vec2(14.0, -18.0);
            let galley = painter.layout_no_wrap(
                drag.readout.clone(),
                egui::FontId::monospace(13.0),
                ui.visuals().strong_text_color(),
            );
            let background = egui::Rect::from_min_size(at, galley.size()).expand(4.0);
            painter.rect_filled(background, 3.0, ui.visuals().extreme_bg_color.gamma_multiply(0.9));
            painter.galley(at, galley, ui.visuals().strong_text_color());
        }
    }
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
    unit: scadstudio_core::unit::Unit,
    colour: egui::Color32,
) {
    let size = hi - lo;
    let centre = (lo + hi) * 0.5;
    let labels = [
        (Vec3::new(centre.x, lo.y, lo.z), scadstudio_core::unit::format_length(size.x, unit)),
        (Vec3::new(hi.x, centre.y, lo.z), scadstudio_core::unit::format_length(size.y, unit)),
        (Vec3::new(hi.x, lo.y, centre.z), scadstudio_core::unit::format_length(size.z, unit)),
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
