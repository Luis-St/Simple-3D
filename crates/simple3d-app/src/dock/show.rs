//! Drawing one dock and the panels in it.

use super::*;
use crate::app::App;
use crate::theme::{metric, token};
use simple3d_core::config::{Panel, Side};

/// Draw one dock; everything it changes is on `app`.
pub fn show(app: &mut App, ctx: &egui::Context, side: Side) {
    if app.settings.layout.docks_hidden {
        return;
    }
    let panels = app.settings.layout.panels(side).to_vec();
    if panels.is_empty() {
        // An empty dock is still a drop target, or panels could never return.
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
            // Claim the full width, since egui remembers a panel by its content rect and narrow contents
            // would keep shrinking the dock.
            ui.expand_to_include_rect(ui.max_rect());
            ui.set_min_width(new_width);
            ui.spacing_mut().item_spacing = egui::vec2(metric::GAP, 2.0);
            let filler = app.settings.layout.filler(side);
            let mut centres: Vec<f32> = Vec::new();

            // Panels above the filler stack from the top, those below from the bottom, keeping layout order.
            // With everything rolled up there is no filler, just a stack of headers.
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

/// A panel that is not the filler: its own strip, resizable with a body, one header high when rolled
/// up. Returns its header's vertical centre.
pub(crate) fn strip(app: &mut App, ui: &mut egui::Ui, panel: Panel, side: Side, from_top: bool) -> f32 {
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

pub(crate) fn body(app: &mut App, ui: &mut egui::Ui, panel: Panel) {
    match panel {
        Panel::Outliner => crate::panel_outliner::show_inside(app, ui),
        Panel::Primitives => crate::panel_primitives::show_inside(app, ui),
        Panel::Properties => crate::panel_properties::show_inside(app, ui),
    }
}
