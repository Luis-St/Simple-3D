//! The two docks and what moves between them.
//!
//! A panel is a header bar and a body. The header is the whole of its interface:
//! click it to roll the panel up to that bar, drag it to put it in the other
//! dock or somewhere else in this one. There is no separate arrange mode and no
//! menu of positions -- the thing you want to move is the thing you drag.
//!
//! Where each panel *is* lives in `AppSettings::layout`, beside the dock widths
//! that were already persisted there, so an arrangement survives a restart.

use crate::app::App;
use crate::theme::{self, metric, token};
use scadstudio_core::config::{Layout, Panel, Side};

/// A panel being dragged, and where it would land if it were dropped now.
#[derive(Clone, Copy, Debug, Default)]
pub struct DockDrag {
    pub panel: Option<Panel>,
    pub target: Option<(Side, usize)>,
}

/// Where a pointer at `y` would drop a panel in a dock whose header bars are at
/// `headers` (each the vertical centre of one header, in order).
///
/// Split out from the drawing so the rule can be reasoned about on its own: the
/// drop goes above every header the pointer is above, which is the index of the
/// first header below it.
pub fn drop_index(headers: &[f32], y: f32) -> usize {
    headers.iter().filter(|centre| **centre < y).count()
}

/// Draw one dock. Returns nothing: everything it changes, it changes on `app`.
pub fn show(app: &mut App, ctx: &egui::Context, side: Side) {
    if app.settings.layout.docks_hidden {
        return;
    }
    let panels = app.settings.layout.panels(side).to_vec();
    if panels.is_empty() {
        // An empty dock still has to be a drop target, or a panel dragged out of
        // it could never be put back.
        if app.dock_drag.panel.is_none() {
            return;
        }
    }
    let width = match side {
        Side::Left => app.settings.outliner_width,
        Side::Right => app.settings.properties_width,
    };
    let frame = egui::Frame::NONE.fill(token::SURFACE_1);
    let builder = match side {
        Side::Left => egui::SidePanel::left("dock-left"),
        Side::Right => egui::SidePanel::right("dock-right"),
    };
    let mut new_width = width;
    let response =
        builder.frame(frame).resizable(true).default_width(width).width_range(200.0..=620.0).show(ctx, |ui| {
            new_width = ui.available_width();
            // Claim the dock's full width up front. egui remembers a panel by
            // the rectangle its *content* filled, so a panel whose contents
            // happened to be narrower than the dock would shrink the dock to
            // fit them -- and, being remembered, would keep shrinking it.
            ui.expand_to_include_rect(ui.max_rect());
            ui.set_min_width(new_width);
            ui.spacing_mut().item_spacing = egui::vec2(metric::GAP, 2.0);
            let filler = app.settings.layout.filler(side);
            let mut centres: Vec<f32> = Vec::new();

            // Everything above the filler stacks from the top; everything below
            // it stacks from the bottom, so the order on screen is the order in
            // the layout however many are collapsed.
            // With every panel rolled up there is no filler, and then every one
            // of them is a strip from the top -- the dock is a stack of headers.
            let split = filler.and_then(|f| panels.iter().position(|p| *p == f));
            let above = split.unwrap_or(panels.len());
            for panel in &panels[..above] {
                centres.push(strip(app, ui, *panel, side, true));
            }
            if let Some(split) = split {
                for panel in panels[split + 1..].iter().rev() {
                    centres.push(strip(app, ui, *panel, side, false));
                }
            }
            if let Some(panel) = filler {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(ui, |ui| {
                    let centre = header(app, ui, panel, side);
                    centres.push(centre);
                    body(app, ui, panel);
                });
            }
            centres.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            app.dock_headers.push((side, centres));
        });

    match side {
        Side::Left => app.settings.outliner_width = new_width,
        Side::Right => app.settings.properties_width = new_width,
    }
    app.dock_rects.push((side, response.response.rect));
}

/// A panel that is not the one taking the leftover height: a strip of its own,
/// resizable when it has a body and exactly one header high when it is rolled
/// up. Returns the vertical centre of its header.
fn strip(app: &mut App, ui: &mut egui::Ui, panel: Panel, side: Side, from_top: bool) -> f32 {
    let collapsed = app.settings.layout.is_collapsed(panel);
    let id = egui::Id::new(("dock-strip", side, panel));
    let frame = egui::Frame::NONE.fill(token::SURFACE_1);
    let builder = if from_top { egui::TopBottomPanel::top(id) } else { egui::TopBottomPanel::bottom(id) };
    let mut centre = 0.0;
    if collapsed {
        builder.frame(frame).resizable(false).exact_height(metric::ROW).show_inside(ui, |ui| {
            centre = header(app, ui, panel, side);
        });
    } else {
        builder
            .frame(frame)
            .resizable(true)
            .default_height(200.0)
            .height_range(metric::ROW + 24.0..=680.0)
            .show_inside(ui, |ui| {
                centre = header(app, ui, panel, side);
                body(app, ui, panel);
            });
    }
    centre
}

/// The header bar: the panel's name, a twisty that says whether it is rolled up,
/// and the grip the whole bar is. Returns its vertical centre.
fn header(app: &mut App, ui: &mut egui::Ui, panel: Panel, side: Side) -> f32 {
    let collapsed = app.settings.layout.is_collapsed(panel);
    let full = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(full, metric::ROW), egui::Sense::click_and_drag());
    let dragging = app.dock_drag.panel == Some(panel);
    let fill = if dragging || response.hovered() { token::SURFACE_3 } else { token::SURFACE_2 };
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0_f32, token::SURFACE_3));
    theme::twisty(ui.painter(), egui::pos2(rect.left() + 11.0, rect.center().y), !collapsed, token::TEXT_LO);
    ui.painter().text(
        egui::pos2(rect.left() + 22.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        theme::header_text(panel.label()).text(),
        egui::FontId::proportional(theme::font::HEADER),
        token::TEXT_LO,
    );
    // The grip dots on the right say the bar can be dragged, in the one place a
    // pointer would go looking for them.
    for i in 0..3 {
        let x = rect.right() - 10.0;
        let y = rect.center().y - 4.0 + i as f32 * 4.0;
        ui.painter().hline(x - 5.0..=x, y, egui::Stroke::new(1.0_f32, token::TEXT_LO.gamma_multiply(0.6)));
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if response.clicked() {
        app.settings.layout.toggle_collapsed(panel);
    }
    if response.drag_started() {
        app.dock_drag.panel = Some(panel);
    }
    let _ = side;
    rect.center().y
}

fn body(app: &mut App, ui: &mut egui::Ui, panel: Panel) {
    match panel {
        Panel::Outliner => crate::panel_outliner::show_inside(app, ui),
        Panel::Primitives => crate::panel_primitives::show_inside(app, ui),
        Panel::Properties => crate::panel_properties::show_inside(app, ui),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drop_lands_above_the_header_it_is_dropped_on() {
        let headers = [20.0, 120.0, 400.0];
        assert_eq!(drop_index(&headers, 5.0), 0, "above everything means first");
        assert_eq!(drop_index(&headers, 60.0), 1);
        assert_eq!(drop_index(&headers, 300.0), 2);
        assert_eq!(drop_index(&headers, 900.0), 3, "below everything means last");
        assert_eq!(drop_index(&[], 42.0), 0, "an empty dock takes the panel at index nought");
    }

    #[test]
    fn a_panel_moves_between_docks_and_reorders_within_one() {
        let mut layout = Layout::default();
        assert_eq!(layout.side_of(Panel::Properties), Side::Right);

        layout.move_to(Panel::Properties, Side::Left, 0);
        assert_eq!(layout.left, vec![Panel::Properties, Panel::Outliner, Panel::Primitives]);
        assert!(layout.right.is_empty());
        assert_eq!(layout.side_of(Panel::Properties), Side::Left);

        // Reordering within a dock reads the index after the panel is lifted
        // out, so this really moves it one place down.
        layout.move_to(Panel::Properties, Side::Left, 1);
        assert_eq!(layout.left, vec![Panel::Outliner, Panel::Properties, Panel::Primitives]);

        // And past the end lands at the end rather than panicking.
        layout.move_to(Panel::Outliner, Side::Right, 99);
        assert_eq!(layout.right, vec![Panel::Outliner]);
    }

    #[test]
    fn the_filler_is_the_last_panel_that_is_not_rolled_up() {
        let mut layout = Layout::default();
        assert_eq!(layout.filler(Side::Left), Some(Panel::Primitives));
        layout.toggle_collapsed(Panel::Primitives);
        assert_eq!(layout.filler(Side::Left), Some(Panel::Outliner));
        layout.toggle_collapsed(Panel::Outliner);
        assert_eq!(layout.filler(Side::Left), None, "a dock of nothing but headers has no filler");
        layout.toggle_collapsed(Panel::Outliner);
        assert_eq!(layout.filler(Side::Left), Some(Panel::Outliner), "collapsing is a toggle");
    }

    #[test]
    fn a_layout_that_lost_or_duplicated_a_panel_is_repaired_rather_than_left_unreachable() {
        // A settings file from another version, or one edited by hand. A panel
        // that appears nowhere would have no way back.
        let mut layout = Layout { left: vec![], right: vec![], collapsed: vec![], docks_hidden: false };
        layout.repair();
        for panel in Panel::ALL {
            assert!(layout.left.contains(&panel) || layout.right.contains(&panel), "{panel:?} is unreachable");
        }

        let mut duplicated = Layout {
            left: vec![Panel::Outliner, Panel::Outliner, Panel::Properties],
            right: vec![Panel::Outliner, Panel::Primitives],
            collapsed: vec![],
            docks_hidden: false,
        };
        duplicated.repair();
        assert_eq!(duplicated.left, vec![Panel::Outliner, Panel::Properties]);
        assert_eq!(duplicated.right, vec![Panel::Primitives]);
    }

    #[test]
    fn hiding_the_docks_keeps_the_arrangement_exactly() {
        let mut layout = Layout::default();
        layout.move_to(Panel::Primitives, Side::Right, 0);
        layout.toggle_collapsed(Panel::Outliner);
        let before = layout.clone();
        layout.docks_hidden = true;
        layout.docks_hidden = false;
        assert_eq!(layout, before, "showing the docks again did not restore them exactly");
    }
}
