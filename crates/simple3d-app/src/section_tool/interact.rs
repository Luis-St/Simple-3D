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
/// same rule, and the others follow the frame while it does. A tilted plane
/// slides along its own normal, which moves the offset by exactly the distance
/// dragged, the way an untilted one does along its axis.
pub fn interact(app: &mut App, ui: &mut egui::Ui, view: &View) -> bool {
    if !app.scene.settings.section.enabled {
        app.section_grab = None;
        app.section_hover = None;
        return false;
    }
    let mut owned = false;
    // Which grip the pointer is on, kept for the drawing: the arrows that say
    // which way the plane travels are put on that one alone, so five grips do
    // not become five pairs of arrows over the model.
    let mut live = None;
    let count = app.scene.settings.section_count();
    for which in 0..count {
        let section = *app.scene.settings.section_at(which);
        let travel = travel(&section);
        for (index, at) in grips(&frame(&section, app.evaluated.bounds)).into_iter().enumerate() {
            let Some((screen, _)) = view.project(at) else { continue };
            let response = ui
                .interact(
                    egui::Rect::from_center_size(screen, egui::Vec2::splat(GRIP + 5.0)),
                    ui.id().with(("section-grip", which, index)),
                    egui::Sense::drag(),
                )
                .on_hover_text(match section.tilted() {
                    true => "Slide the section along its normal".to_string(),
                    false => format!("Slide the section along {}", section.axis_label()),
                });

            if response.hovered() || response.dragged() {
                live = Some((which, index));
                ui.ctx().set_cursor_icon(crate::panel_viewport::slide_cursor(crate::panel_viewport::screen_direction(
                    view, at, travel,
                )));
            }
            // Where the pointer took hold of the plane, kept for the length of
            // the drag: without it the plane jumps so that the point grabbed
            // lands under the pointer, which for a grip in the middle of a
            // large frame is a jump of the whole model. Taking hold of a plane
            // also brings it up in the window, so the numbers there are the
            // ones moving.
            if response.drag_started() {
                app.section_tab = which;
                app.section_grab = ui
                    .input(|i| i.pointer.interact_pos())
                    .and_then(|cursor| view.ray_axis(cursor, Vec3::ZERO, travel))
                    .map(|under| section.offset - under);
            }
            if response.dragged() && app.section_tab == which {
                let grab = app.section_grab.unwrap_or(0.0);
                if let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) {
                    if let Some(under) = view.ray_axis(cursor, Vec3::ZERO, travel) {
                        // Snapped to the document's move step, and freed or
                        // coarsened by the same modifiers every other drag
                        // answers to.
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
    }
    app.section_hover = live;
    owned
}
