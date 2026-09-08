//! The picture the tool draws of what the rule makes.

mod paint;
pub(crate) use paint::*;
mod key;
pub(crate) use key::*;

use super::*;
use crate::app::App;
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
