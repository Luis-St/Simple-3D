//! Painting the scene into the panel.

use super::*;
use crate::app::App;
use crate::render::{self, Grid, Item, Palette, Style};
use crate::view::View;
use simple3d_core::scene::NodeId;

/// Rasterize the scene into a texture, reusing the last image while nothing that
/// affects it has changed, and paint it into the place `slot` reserved for it
/// earlier in the frame.
///
/// The slot is why this runs at the *end* of the viewport's frame rather than at
/// the start: the picture is made from the camera the frame's own gestures have
/// left behind, so nothing drawn over it is a frame ahead of it (issue 102).
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
        let selected: Vec<NodeId> =
            app.top_level_selection().into_iter().filter(|&id| app.scene.is_shown(id)).collect();
        // A ticked piece is outlined like a selection and *glows through*
        // whatever is in front of it: a piece of a split usually sits inside
        // the shape it was cut from, where an outline has nothing on screen to
        // draw itself around (issue 82).
        let ticked: Vec<NodeId> = app.piece_ticks.iter().copied().filter(|&id| app.scene.is_shown(id)).collect();

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
        for id in &ticked {
            if let Some(renderable) = app.node_renderables.get(id) {
                items.push(Item { renderable, style: Style::Glow });
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
            // The split tool's cells, drawn on the model with the depth buffer
            // rather than over the finished picture, so the far side of the
            // shape hides the ones behind it (issue 82).
            preview: crate::split_tool::preview_loops(app),
            // The plane the model is cut with, while there is one (issue 71).
            section: app.scene.settings.section.plane(),
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
