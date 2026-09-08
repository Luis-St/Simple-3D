//! Drawing the preview's picture.

use super::*;
use crate::app::App;
use crate::render::{self, Grid, Item, Palette, Style};
use crate::theme;
use crate::view::View;

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
