//! Dragging the plane by a grip.

use super::*;
use crate::app::App;
use crate::view::View;
use simple3d_geom::Vec3;

/// Drag any of the grips to slide the plane. Returns whether the pointer
/// belongs to one of them this frame, so a drag that moves the plane does not
/// also select what is behind it.
///
/// Every grip is the same control in a different place, so they share the one
/// drag: whichever is taken hold of, the plane slides along its axis by the
/// same rule, and the others follow the frame while it does.
pub fn interact(app: &mut App, ui: &mut egui::Ui, view: &View) -> bool {
    let section = app.scene.settings.section;
    if !section.enabled {
        app.section_grab = None;
        app.section_hover = None;
        return false;
    }
    let axis = section.axis();
    let mut owned = false;
    // Which grip the pointer is on, kept for the drawing: the arrows that say
    // which way the plane travels are put on that one alone, so five grips do
    // not become five pairs of arrows over the model.
    let mut live = None;
    for (index, at) in grips(&frame(&section, app.evaluated.mesh.bounds())).into_iter().enumerate() {
        let Some((screen, _)) = view.project(at) else { continue };
        let response = ui
            .interact(
                egui::Rect::from_center_size(screen, egui::Vec2::splat(GRIP + 5.0)),
                ui.id().with(("section-grip", index)),
                egui::Sense::drag(),
            )
            .on_hover_text(format!("Slide the section along {}", section.axis_label()));

        if response.hovered() || response.dragged() {
            live = Some(index);
            ui.ctx().set_cursor_icon(crate::panel_viewport::slide_cursor(crate::panel_viewport::screen_direction(
                view,
                at,
                travel(axis),
            )));
        }
        // Where the pointer took hold of the plane, kept for the length of the
        // drag: without it the plane jumps so that the point grabbed lands under
        // the pointer, which for a grip in the middle of a large frame is a jump
        // of the whole model.
        if response.drag_started() {
            app.section_grab = ui
                .input(|i| i.pointer.interact_pos())
                .and_then(|cursor| view.ray_axis(cursor, Vec3::ZERO, travel(axis)))
                .map(|under| section.offset - under);
        }
        if response.dragged() {
            let grab = app.section_grab.unwrap_or(0.0);
            if let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) {
                if let Some(under) = view.ray_axis(cursor, Vec3::ZERO, travel(axis)) {
                    // Snapped to the document's move step, and freed or coarsened
                    // by the same modifiers every other drag answers to.
                    let wanted = crate::panel_viewport::mods_from(ui).snap(under + grab, app.move_snap());
                    app.set_section_offset(wanted);
                }
            }
        }
        if response.drag_stopped() {
            app.section_grab = None;
        }
        owned |= response.dragged() || response.hovered();
    }
    app.section_hover = live;
    owned
}
