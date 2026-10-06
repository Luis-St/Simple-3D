//! Painting the scene into the panel.

use super::*;
use crate::app::App;
use crate::render::{self, Grid, Item, Palette, Style};
use crate::view::View;
use simple3d_core::scene::NodeId;

/// Rasterise the scene into a texture (reusing the last one when unchanged) and paint it into
/// `slot`. Runs at the end of the frame so the picture uses the camera the frame's gestures left
/// (issue 102).
pub(crate) fn paint_scene(
    app: &mut App,
    ui: &mut egui::Ui,
    rect: egui::Rect,
    dark: bool,
    slot: egui::layers::ShapeIdx,
) {
    let pixels_per_point = ui.ctx().pixels_per_point();
    let size = [
        (rect.width() * pixels_per_point).round().max(1.0) as usize,
        (rect.height() * pixels_per_point).round().max(1.0) as usize,
    ];

    // A picture query the on-screen frame could not answer is answered by redrawing (`gpu/depth.rs`).
    if app.gpu.as_ref().is_some_and(|gpu| gpu.depth_wanted()) {
        app.invalidate_image();
    }
    let key = image_key(app, size, dark);
    if key != app.image_key || app.texture.is_none() {
        let palette = Palette::for_dark_mode(dark);
        // Framebuffer coordinates (origin top-left, unit pixel), not the panel rect in points, or the
        // model would be offset from its manipulator.
        let render_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size[0] as f32, size[1] as f32));
        let view = View::new(app.scene.camera, render_rect);
        // Asked before the scene's renderables are borrowed below, since the crossing cache is on the app.
        let section = crate::section_tool::cut(app, view.forward());
        // Hidden nodes get no body and no outline, but still a manipulator.
        let selected: Vec<NodeId> =
            app.top_level_selection().into_iter().filter(|&id| app.scene.is_shown(id)).collect();
        // Ticked pieces are outlined and glow through what is in front (issue 82), since a split piece
        // usually sits inside its shape.
        let ticked: Vec<NodeId> = app.piece_ticks.iter().copied().filter(|&id| app.scene.is_shown(id)).collect();

        // While a tool previews, the document's setting says what the viewport hides (issue 82).
        let preview = app.preview_subject();
        let mode = app.scene.settings.preview_viewport;
        let solid = match preview.filter(|_| !mode.keeps_other_bodies()).and_then(|id| app.node_renderables.get(&id)) {
            // Only the previewed object, drawn as the model.
            Some(only) => only,
            None => &app.scene_renderable,
        };
        let mut items: Vec<Item> = vec![Item { renderable: solid, style: Style::Solid }];
        // A dragged body is drawn moved from its own renderable, or as a per-pixel boolean when it is
        // an operand.
        let whole = std::ptr::eq(solid, &app.scene_renderable);
        let ready = if whole { app.csg_ready() } else { Vec::new() };
        let mut live = render::Live { ready: ready.iter().map(|shape| &**shape).collect(), ..Default::default() };
        let csg = app.live_csg().filter(|_| whole);
        let dragged = match &csg {
            Some(csg) => Some((csg.carried, csg.range.clone(), csg.xform)),
            None => app.live_move().filter(|_| whole),
        };
        if let Some((id, part, moved)) = &dragged {
            match &csg {
                Some(csg) => {
                    live.hidden.push((solid.id, part.clone()));
                    live.csg = Some(render::CsgPreview {
                        leaves: csg.leaves.iter().map(|(leaf, placed)| (&**leaf, *placed)).collect(),
                        program: csg.program.clone(),
                        carried: csg.carried_leaves.clone(),
                        tag: solid.bodies.get(part.start as usize).map_or(0, |&body| render::body_tag(body, 0)),
                    });
                }
                None => {
                    if let Some(own) = app.node_renderables.get(id) {
                        live.hidden.push((solid.id, part.clone()));
                        items.push(Item { renderable: own, style: Style::Solid });
                    }
                }
            }
            // Its own renderable and its descendants', however they are drawn.
            for (other, renderable) in &app.node_renderables {
                if other == id || app.scene.is_ancestor_of(*id, *other) {
                    live.placed.push((renderable.id, *moved));
                }
            }
        }
        // Ghosts before the selection outline, so the outline stays readable.
        let ghosts = app.ghosts();
        for (id, renderable) in &app.node_renderables {
            // A ghost group shows its children as ghosts, so an assembly can be positioned before subtracting.
            if ghosts.iter().any(|&g| g == *id || app.scene.is_ancestor_of(g, *id)) {
                items.push(Item { renderable, style: Style::Ghost });
            }
        }
        for id in &selected {
            if let Some(renderable) = app.node_renderables.get(id) {
                items.push(Item { renderable, style: Style::Selected });
            }
        }
        for id in &ticked {
            if let Some(renderable) = app.node_renderables.get(id) {
                items.push(Item { renderable, style: Style::Glow });
            }
        }

        // Where the align tool would put things, drawn as templates of its own objects (issue 70).
        let templates = crate::arrange_tool::templates(app)
            .into_iter()
            .filter_map(|(id, xform)| app.node_renderables.get(&id).map(|renderable| (&**renderable, xform)))
            .collect();
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
            templates,
            live,
            // Tool previews (split cells, simplify triangles) are depth-tested by the renderer (issue 82);
            // only one tool is open at a time.
            preview: crate::split_tool::preview_loops(app)
                .into_iter()
                .chain(crate::simplify_tool::preview_loops(app))
                .chain(crate::reassemble_tool::preview_loops(app))
                .collect(),
            // The section planes (issue 71), cutting the side the camera looks from (issue 109).
            section,
        };
        // The GPU builds the frame from the request; the CPU prepares primitives only when it draws.
        match app.gpu.as_mut() {
            Some(gpu) => match {
                gpu.place(rect.min, pixels_per_point);
                gpu.render(&request)
            } {
                Ok(id) => app.gpu_texture = Some(id),
                Err(why) => {
                    // The driver refused: report once and fall back to software rather than show nothing.
                    app.gpu_error = Some(why);
                    app.gpu = None;
                    app.gpu_texture = None;
                }
            },
            None => app.gpu_texture = None,
        }
        if app.gpu_texture.is_none() {
            let image = render::render_prepared(&request, &render::prepare_frame(&request)).into_color_image();
            match &mut app.texture {
                Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
                None => app.texture = Some(ui.ctx().load_texture("viewport", image, egui::TextureOptions::LINEAR)),
            }
        }
        app.image_key = key;
    }

    // Either way the result is a texture painted over the panel.
    let drawn = app.gpu_texture.or_else(|| app.texture.as_ref().map(|texture| texture.id()));
    if let Some(id) = drawn {
        ui.painter().set(
            slot,
            egui::Shape::image(
                id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            ),
        );
    }
}
