//! Resolving a header drag into a move between docks.

use super::*;
use crate::app::App;
use crate::theme::{metric, token};
use simple3d_core::config::{Layout, Side};

/// Work out where a drag would drop, draw the line that says so, and apply it on
/// release. Called once a frame, after both docks have drawn themselves, since
/// it needs both their rectangles.
pub fn resolve_drag(app: &mut App, ctx: &egui::Context) {
    app.dock_drag.target = None;
    let Some(panel) = app.dock_drag.panel else {
        app.dock_headers.clear();
        app.dock_rects.clear();
        return;
    };
    let pointer = ctx.input(|i| i.pointer.interact_pos());
    if let Some(pointer) = pointer {
        // The nearer dock wins when the pointer is over neither: a drag that
        // ends in the viewport has to land somewhere, and the side it is on is
        // the least surprising answer.
        let side = app
            .dock_rects
            .iter()
            .find(|(_, rect)| rect.contains(pointer))
            .map(|(side, _)| *side)
            .unwrap_or(if pointer.x < ctx.screen_rect().center().x { Side::Left } else { Side::Right });
        let centres = app.dock_headers.iter().find(|(s, _)| *s == side).map(|(_, c)| c.clone()).unwrap_or_default();
        let index = drop_index(&centres, pointer.y);
        app.dock_drag.target = Some((side, index));

        if let Some((_, rect)) = app.dock_rects.iter().find(|(s, _)| *s == side) {
            let y = centres.get(index).copied().unwrap_or(rect.bottom() - 1.0) - metric::ROW / 2.0;
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("dock-drop")));
            painter.hline(rect.x_range(), y, egui::Stroke::new(2.0_f32, token::ACCENT));
        }
    }
    if ctx.input(|i| i.pointer.any_released()) {
        if let Some((side, index)) = app.dock_drag.target {
            app.settings.layout.move_to(panel, side, index);
        }
        app.dock_drag = DockDrag::default();
    }
    app.dock_headers.clear();
    app.dock_rects.clear();
}

/// The whole layout, back to how it ships.
pub fn reset(app: &mut App) {
    app.settings.layout = Layout::default();
    app.settings.outliner_width = 260.0;
    app.settings.properties_width = 320.0;
}
