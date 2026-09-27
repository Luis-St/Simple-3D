//! What the cut would come to, and the outlines it previews.

use super::*;
use crate::app::App;
use crate::{theme, ui};
use simple3d_geom::Vec3;

/// What the plan comes to: how many cells, or why it cannot be cut.
pub(crate) fn summary(app: &mut App, ui: &mut egui::Ui, tool: &SplitTool) {
    match tool.plan.refusal(tool.bounds) {
        Some(why) => {
            ui.add(egui::Label::new(egui::RichText::new(why).size(theme::font::LABEL).color(theme::token::ACCENT)));
        }
        None => {
            let cells = tool.plan.planned(tool.bounds);
            let size = tool.bounds.1 - tool.bounds.0;
            ui.add(
                egui::Label::new(theme::hint(format!(
                    "Up to {cells} cells over {}. A cell the shape does not reach makes no piece, so there will \
                     usually be fewer pieces than cells.",
                    ui::describe_size(size, app.unit())
                )))
                .selectable(false),
            );
        }
    }
}

/// Where the cuts fall, in world space, drawn over the model by the renderer (issue 82) so the cells
/// are judged on the shape and depth-tested. Loops go out through the tool's `placement`.
pub(crate) fn preview_loops(app: &App) -> Vec<Vec<Vec3>> {
    let Some(tool) = app.split_tool.as_ref() else { return Vec::new() };
    if tool.plan.refusal(tool.bounds).is_some() {
        return Vec::new();
    }
    tool.plan
        .preview_loops(tool.bounds, PREVIEW_LOOPS)
        .into_iter()
        .map(|loop_| loop_.into_iter().map(|point| tool.placement.point(point)).collect())
        .collect()
}
