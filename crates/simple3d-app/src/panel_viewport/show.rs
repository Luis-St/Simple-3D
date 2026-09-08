//! Laying the viewport out, and what one frame of it costs.

use super::*;
use crate::app::App;
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
                // The section plane's grip, on the same terms: a drag on it
                // slides the cut rather than selecting what is behind it
                // (issue 71).
                let section_owned = crate::section_tool::interact(app, ui, &view);
                let owned = grips_owned || section_owned || manipulate(app, ui, &response, &view);
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
    });
}

pub(crate) fn image_key(app: &App, size: [usize; 2], dark: bool) -> u64 {
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
    // The cells themselves are in the picture now, so every number that moves
    // them is part of what the picture was drawn from.
    if let Some(tool) = app.split_tool.as_ref() {
        tool.hash_preview(&mut hasher);
    }
    app.scene.settings.preview_viewport.hash(&mut hasher);
    app.scene.settings.grid_visible.hash(&mut hasher);
    app.scene.settings.grid_spacing.to_bits().hash(&mut hasher);
    app.scene.settings.axes_visible.hash(&mut hasher);
    app.scene.settings.axis_style.hash(&mut hasher);
    app.scene.settings.plane_marks.hash(&mut hasher);
    // The cut is part of the picture, so every number that moves it redraws it.
    crate::section_tool::hash_section(&app.scene.settings.section, &mut hasher);
    let camera = app.scene.camera;
    for value in
        [camera.target.x, camera.target.y, camera.target.z, camera.distance, camera.yaw, camera.pitch, camera.fov_deg]
    {
        value.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}
