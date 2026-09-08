//! The picture the tool draws of what the rule makes.

use super::*;
use crate::app::App;
use crate::render::{self, Grid, Item, Palette, Style};
use crate::theme;
use crate::view::View;
use simple3d_core::pattern;

/// The id the cross that drops the last stage answers to. Named for the same
/// reason `preview_id` is: a test asks where it was drawn rather than guessing.
pub(crate) fn drop_stage_id() -> egui::Id {
    egui::Id::new("pattern-drop-stage")
}

/// The id the preview's picture answers to. Named rather than taken from the
/// layout, so a test can ask the context where the picture was drawn and check
/// what is in it -- the same bargain every other grip in the application makes.
pub(crate) fn preview_id() -> egui::Id {
    egui::Id::new("pattern-preview")
}

/// What the rule lays out, as a viewport.
///
/// It was a flat scatter of dots, one per copy, because a rule is a handful of
/// numbers and nobody reads a helix out of six of them. Dots answer "how many
/// and roughly where" and nothing else, though, and the question a rule is
/// actually judged by -- what the shape looks like repeated -- needs the shape.
/// So this is the same render the viewport behind the window is drawing, from a
/// camera of its own that the pointer can turn.
///
/// Everything but the camera comes from the main window: the display mode, the
/// grid, the axes, the plane marks. An axis switched off out there is switched
/// off in here, because there is one set of view settings in the application
/// and this is not a second one.
pub(crate) fn preview(app: &mut App, ui: &mut egui::Ui) {
    let params = app.pattern_tool_params();
    ui.horizontal(|ui| {
        // Clear of the divider, the way a panel header stands clear of its dock's
        // edge. The picture below is left flush with it: a viewport against a rule
        // reads as a viewport, and a *word* against one reads as crowding.
        ui.add_space(theme::metric::PANEL_PAD);
        ui.add(egui::Label::new(theme::header_text("Lays out")).selectable(false));
        let (wanted, made) = pattern::instance_count(&params);
        let note = if wanted > made {
            format!("{made} copies -- {wanted} were asked for, which is more than can be drawn")
        } else if app.pattern_tool_is_empty() {
            // Otherwise the dots are a picture with no caption: the rule works,
            // and what is missing is the shape it has nothing to repeat.
            format!("{made} copies, marked -- put a shape in the pattern to see it repeated")
        } else {
            format!("{made} copies")
        };
        ui.add(egui::Label::new(theme::hint(note)).selectable(false));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("Frame")
                .on_hover_text("Put the preview back: the viewport's angle, framed on the pattern")
                .clicked()
            {
                app.reset_pattern_preview();
            }
        });
    });
    ui.add_space(4.0);

    // Whatever is left of the column, which is what makes this a viewport and
    // not a stamp: it grows with the window.
    let rect = ui.available_rect_before_wrap();
    if rect.width() < 32.0 || rect.height() < 32.0 {
        return;
    }
    // Framed once, when the tool opens, and then only when the Frame button asks
    // for it. It used to frame itself again whenever the rule started laying its
    // copies out somewhere else, which meant every number typed moved the
    // camera: a picture turned and zoomed to look at one end of a run jumped
    // back to the whole of it on the next keystroke. Where the picture is
    // looking from is the user's, and Frame is how they hand it back.
    if !app.pattern_preview_framed {
        app.frame_pattern_preview((rect.width() / rect.height().max(1.0)) as f64);
    }
    ui.allocate_rect(rect, egui::Sense::hover());
    let response = ui.interact(rect, preview_id(), egui::Sense::click_and_drag());
    paint_preview(app, ui, rect);
    if app.pattern_tool_is_empty() {
        paint_placements(app, ui, rect);
    }

    // The same navigation bindings the viewport uses, read from the keymap on
    // every frame, so a rebinding applies here as immediately as it does there.
    let nav = app.keymap.nav;
    let (ctrl, shift, alt) = ui.input(|i| (i.modifiers.command, i.modifiers.shift, i.modifiers.alt));
    let held = [
        response.dragged_by(egui::PointerButton::Primary),
        response.dragged_by(egui::PointerButton::Middle),
        response.dragged_by(egui::PointerButton::Secondary),
    ];
    if let Some(gesture) = crate::panel_viewport::nav_gesture(&nav, held, ctrl, shift, alt) {
        let view = View::new(app.pattern_preview_camera, rect);
        crate::panel_viewport::apply_gesture(&mut app.pattern_preview_camera, gesture, response.drag_delta(), &view);
    }
    if response.hovered() {
        let (scroll, at) = ui.input(|i| (i.smooth_scroll_delta.y, i.pointer.hover_pos()));
        crate::panel_viewport::apply_zoom(&mut app.pattern_preview_camera, &nav, scroll, at.map(|at| (rect, at)));
    }
    // No cursor of its own. The viewport this is a copy of leaves the pointer
    // alone while it is orbited, and a preview that swapped it for a hand said
    // the picture was something to pick up.
}

/// Orange dots where the rule would put a copy.
///
/// A pattern is *built* empty: the tool makes one out of the selection, and a
/// selection of nothing makes a pattern with nothing in it. That pattern has no
/// geometry, so the render behind this has nothing to draw and the picture is
/// the bare grid -- which reads as a preview that does not work rather than as
/// a pattern with nothing in it yet. The rule still has placements, and while
/// the shape is missing they are the whole of what the window has to show.
///
/// Drawn over the render rather than into it, in the picture's own camera, so
/// the dots sit on the grid exactly where the copies will and turn with an
/// orbit like everything else. The original is the one every other copy is a
/// copy *of*, so it is the one drawn brightest.
pub(crate) fn paint_placements(app: &App, ui: &egui::Ui, rect: egui::Rect) {
    let view = View::new(app.pattern_preview_camera, rect);
    let painter = ui.painter_at(rect);
    for (index, at) in app.pattern_placements().iter().enumerate() {
        // Orthographic, so there is no behind-the-camera to test for: every
        // placement lands somewhere, and the clip takes the ones off the
        // picture.
        let Some((screen, _)) = view.project(*at) else { continue };
        let (radius, colour) =
            if index == 0 { (5.0, theme::token::ACCENT) } else { (3.5, theme::token::ACCENT.gamma_multiply(0.7)) };
        painter.circle_filled(screen, radius, colour);
    }
}

/// Rasterize the preview, reusing the last image while nothing that affects it
/// has changed -- the same bargain the viewport's own image strikes, and the
/// reason a dialog that redraws sixty times a second can hold a render at all.
///
/// Always the software rasterizer: the GPU renderer draws into a texture owned
/// by the main window's context, and this is a second native window.
pub(crate) fn paint_preview(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect) {
    // Capped, unlike the viewport's own image: this one is rasterized in
    // software on every frame of an orbit, and on a large screen the dialog's
    // half of it is a million pixels. Past the cap the picture is drawn scaled,
    // which costs a little sharpness and keeps the drag smooth.
    let pixels_per_point = ui.ctx().pixels_per_point().min(PREVIEW_MAX_PX / rect.width().max(rect.height()));
    let size = [
        (rect.width() * pixels_per_point).round().max(1.0) as usize,
        (rect.height() * pixels_per_point).round().max(1.0) as usize,
    ];
    let dark = ui.visuals().dark_mode;
    let key = preview_key(app, size, dark);
    if key != app.pattern_preview_key || app.pattern_preview_texture.is_none() {
        // Scoped so the borrow of the renderables ends before the texture,
        // which lives on the same application, is written to.
        let image = {
            // The framebuffer's own space, not the dialog's position on screen.
            let render_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size[0] as f32, size[1] as f32));
            let mut items = vec![Item { renderable: &app.scene_renderable, style: Style::Solid }];
            // The pattern itself wears the selection outline, so which copies
            // the rule is laying out is never in doubt.
            if let Some(renderable) = app.pattern_tool.and_then(|id| app.node_renderables.get(&id)) {
                items.push(Item { renderable, style: Style::Selected });
            }
            let request = render::Request {
                view: View::new(app.pattern_preview_camera, render_rect),
                size,
                mode: app.settings.display_mode,
                palette: Palette::for_dark_mode(dark),
                grid: Grid {
                    visible: app.scene.settings.grid_visible,
                    spacing: app.scene.settings.grid_spacing,
                    axes: app.scene.settings.axes_visible,
                    style: app.scene.settings.axis_style,
                    plane_marks: app.scene.settings.plane_marks,
                },
                items,
                // The pattern's own preview is the picture itself, not a set of
                // loops drawn over it.
                preview: Vec::new(),
                // The document's section, like every other view setting here:
                // a preview of a model that is being looked into should be
                // looked into as well.
                section: app.scene.settings.section.plane(),
            };
            let prepared = render::prepare_frame(&request);
            render::render_prepared(&request, &prepared).to_color_image()
        };
        match &mut app.pattern_preview_texture {
            Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
            None => {
                app.pattern_preview_texture =
                    Some(ui.ctx().load_texture("pattern-preview", image, egui::TextureOptions::LINEAR));
            }
        }
        app.pattern_preview_key = key;
    }
    if let Some(texture) = &app.pattern_preview_texture {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
}

/// Everything the preview's image is drawn from. Unchanged, and the last one is
/// still what should be on screen.
pub(crate) fn preview_key(app: &App, size: [usize; 2], dark: bool) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    size.hash(&mut hasher);
    dark.hash(&mut hasher);
    app.evaluation_generation.hash(&mut hasher);
    app.renderable_key.hash(&mut hasher);
    (app.settings.display_mode as u8).hash(&mut hasher);
    app.scene.settings.grid_visible.hash(&mut hasher);
    app.scene.settings.grid_spacing.to_bits().hash(&mut hasher);
    app.scene.settings.axes_visible.hash(&mut hasher);
    app.scene.settings.axis_style.hash(&mut hasher);
    app.scene.settings.plane_marks.hash(&mut hasher);
    crate::section_tool::hash_section(&app.scene.settings.section, &mut hasher);
    let camera = app.pattern_preview_camera;
    for value in
        [camera.target.x, camera.target.y, camera.target.z, camera.distance, camera.yaw, camera.pitch, camera.fov_deg]
    {
        value.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}
