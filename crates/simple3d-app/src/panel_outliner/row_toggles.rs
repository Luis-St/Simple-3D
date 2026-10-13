//! The switches at a row's right edge: visibility, and whether it is a negative part (issue 119).

use crate::app::App;
use crate::icon::{self, Glyph};
use crate::theme::token;
use simple3d_core::scene::NodeId;

/// Side and spacing of one switch.
const SIZE: f32 = 16.0;
const STEP: f32 = 20.0;

/// Draws the switches and flips what was clicked. Returns the leftmost switch's left edge, for the
/// name to stop at, and whether the pointer is over one, so the row does not take the click too.
pub(crate) fn toggles(
    app: &mut App,
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    id: NodeId,
    shadowed: bool,
) -> (f32, bool) {
    let node = app.scene.node(id);
    let (visible, negative) = (node.visible, node.negative);
    let is_root = id == app.scene.root();
    let sense = if shadowed { egui::Sense::hover() } else { egui::Sense::click() };
    let at = |slot: f32| {
        egui::Rect::from_min_size(
            egui::pos2(rect.right() - 22.0 - slot * STEP, rect.top() + 3.0),
            egui::Vec2::splat(SIZE),
        )
    };
    let (eye_rect, cut_rect) = (at(0.0), at(1.0));
    let eye = ui.interact(eye_rect, ui.id().with((id, "eye")), sense);
    let cut = ui.interact(cut_rect, ui.id().with((id, "negative")), sense);
    // The root has neither, but keeps the space so names line up.
    if is_root {
        return (cut_rect.left(), false);
    }

    // Visibility, always drawn, dimmed when hidden.
    let colour = if eye.hovered() && !shadowed {
        token::TEXT_HI
    } else if visible {
        token::TEXT_LO
    } else {
        token::TEXT_LO.gamma_multiply(0.45)
    };
    icon::draw(painter, eye_rect.shrink(1.0), if visible { Glyph::Eye } else { Glyph::EyeOff }, colour);
    if eye.clicked() && !shadowed {
        app.edit("Toggle visibility", None);
        if let Some(node) = app.scene.get_mut(id) {
            node.visible = !visible;
        }
    }

    // Negative part: lit in the cut colour when on, faint otherwise so it can be found.
    let colour = if cut.hovered() && !shadowed {
        token::TEXT_HI
    } else if negative {
        token::DANGER
    } else {
        token::TEXT_LO.gamma_multiply(0.3)
    };
    icon::draw(painter, cut_rect.shrink(1.0), Glyph::Negative, colour);
    let cut = if shadowed {
        cut
    } else {
        cut.on_hover_text(if negative {
            "Negative part: cut from the other objects in its group. Click to make it solid"
        } else {
            "Click to make it a negative part, cut from the other objects in its group"
        })
    };
    if cut.clicked() && !shadowed {
        app.set_negative(&[id], !negative);
    }
    (cut_rect.left(), (eye.hovered() || cut.hovered()) && !shadowed)
}
