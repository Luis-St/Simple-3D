//! What the cut would come to, and the outlines it previews.

use super::*;
use crate::app::App;
use crate::{theme, ui};
use simple3d_geom::Vec3;

/// What the plan comes to: how many cells it lays over the shape, or why it
/// cannot be cut at all.
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

/// Where the cuts will fall, in world space, for the renderer to draw over the
/// model (issue 82).
///
/// This is the tool's preview, and it is in the viewport rather than in the
/// window on purpose. A plan drawn inside the window can only ever show the
/// tiling seen straight down its own axis; the question that actually stops
/// people -- will this cut fall through the middle of that boss, is the grid
/// turned the way I think it is -- is a question about the *shape*, and it is
/// answered by drawing the cells on the shape and turning the model.
///
/// The loops go to the renderer rather than to the 2D painter so that the depth
/// buffer can have them: a cell on the far side of the solid is behind it, and
/// a grid drawn through the shape reads as floating in front of it. The cells
/// live in the shape's own frame, so every loop goes out through the tool's
/// `placement` on the way.
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
