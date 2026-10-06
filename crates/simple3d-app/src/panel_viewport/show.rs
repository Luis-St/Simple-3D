//! Laying the viewport out, and what one frame of it costs.

use super::*;
use crate::app::App;
use std::hash::{Hash, Hasher};

pub fn show(app: &mut App, ctx: &egui::Context) {
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
        let rect = ui.available_rect_before_wrap();
        app.viewport_rect = rect;
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        // Reserve the picture's paint slot now and fill it at the end of the frame, so the model is drawn
        // from the same camera as the overlays after this frame's navigation (issue 102).
        let scene = ui.painter().add(egui::Shape::Noop);

        // The cube gets the pointer first, or a face click would also orbit.
        let cube = view_cube_interact(app, ui, rect);
        if !cube.taken {
            navigate(app, ui, &response);
            // The measure tool owns the pointer while out: clicks pick features to measure.
            if app.measure.active {
                let view = app.current_view();
                measure_interact(app, ui, &response, &view);
            } else if app.round_tool.is_some() {
                // Rounding picks edges and corners rather than objects (issue 88).
                let view = app.current_view();
                crate::round_tool::interact(app, ui, &response, &view);
            } else if crate::arrange_tool::wants_pointer(app) {
                // Spreading along a path, the clicks build the path (issue 70).
                let view = app.current_view();
                crate::arrange_tool::interact(app, ui, &response, &view);
            } else {
                let view = app.current_view();
                place_cursor(app, ui, &response, &view);
                let view = app.current_view();
                // Pattern grips take the pointer before the manipulator (issue 67).
                let grips_owned = pattern_grips_interact(app, ui, &view);
                // The section grip likewise slides the cut rather than selecting behind it (issue 71).
                let section_owned = crate::section_tool::interact(app, ui, &view);
                // Push/pull drags faces rather than handles (issue 73).
                let owned = grips_owned
                    || section_owned
                    || if app.mode == crate::gizmo::Mode::PushPull {
                        crate::push_pull_tool::interact(app, ui, &response, &view)
                    } else {
                        manipulate(app, ui, &response, &view)
                    };
                // Picking is outside the manipulator, since with nothing selected there is no gizmo and the first
                // click was lost.
                if !owned && response.clicked_by(egui::PointerButton::Primary) {
                    select_under_cursor(app, ui, &view);
                }
            }
        }
        // One camera for the whole picture, read after the frame's gestures.
        let dark = ui.visuals().dark_mode;
        paint_scene(app, ui, rect, dark, scene);
        let view = app.current_view();
        view_cube_paint(app, ui, &cube);
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
    // A preview opening or closing, or its setting changing, changes what is drawn (issue 82).
    app.preview_subject().hash(&mut hasher);
    // The split cells are in the picture, so their numbers are part of the key.
    if let Some(tool) = app.split_tool.as_ref() {
        tool.hash_preview(&mut hasher);
    }
    // The simplify wireframe switch is not reflected elsewhere in the key (issue 106).
    if let Some(tool) = app.simplify_tool.as_ref() {
        tool.wireframe.hash(&mut hasher);
    }
    // The align tool's templates are not in the scene either (issue 70).
    if let Some(tool) = app.arrange_tool.as_ref() {
        tool.hash_preview(&mut hasher);
    }
    // The reassembly overlay is not the mesh, so it needs its own key part (issue 108).
    if let Some(tool) = app.reassemble_tool.as_ref() {
        tool.hash_preview(&mut hasher);
    }
    app.scene.settings.preview_viewport.hash(&mut hasher);
    app.scene.settings.grid_visible.hash(&mut hasher);
    app.scene.settings.grid_spacing.to_bits().hash(&mut hasher);
    app.scene.settings.axes_visible.hash(&mut hasher);
    app.scene.settings.axis_style.hash(&mut hasher);
    app.scene.settings.plane_marks.hash(&mut hasher);
    // The cut is part of the picture, so every number that moves it redraws it.
    for section in app.scene.settings.sections() {
        crate::section_tool::hash_section(section, &mut hasher);
    }
    // Push/pull's swept solid is drawn in the picture (issue 73).
    if let Some((distance, _)) = app.push_pull.drag.as_ref().and_then(|drag| drag.prism.as_ref()) {
        distance.to_bits().hash(&mut hasher);
    }
    // The round tool's ghost is drawn in the picture too (issue 88).
    app.round_tool.as_ref().and_then(|tool| tool.picture_key()).hash(&mut hasher);
    // A GPU-drawn dragged body moves without anything above changing.
    if let Some(moved) = app.live_csg().map(|csg| csg.xform).or_else(|| app.live_move().map(|(_, _, moved)| moved)) {
        moved.hash_bits(&mut hasher);
    }
    let camera = app.scene.camera;
    for value in
        [camera.target.x, camera.target.y, camera.target.z, camera.distance, camera.yaw, camera.pitch, camera.fov_deg]
    {
        value.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}
