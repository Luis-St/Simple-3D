//! What the run came to.

use super::*;
use crate::app::App;
use crate::simplify_tool::distance;
use crate::theme;
use simple3d_geom::shaping::MAX_TRIANGLES;

/// The triangle counts and how far the surface moved, or that a run is going.
pub(crate) fn summary(app: &App, ui: &mut egui::Ui, tool: &ShapeTool) {
    let before = tool.original.triangle_count();
    let text = match (&tool.shown, &tool.job) {
        (_, Some(job)) if job.elapsed().as_millis() > 150 => "Working\u{2026}".to_string(),
        (None, _) => "Working\u{2026}".to_string(),
        (Some(shown), _) => {
            let after = shown.mesh.triangle_count();
            let moved = distance(shown.moved, app.unit());
            let mut text = match after == before {
                true => format!("{before} triangles, moving the surface at most {moved}."),
                false => format!("{before} triangles refined to {after}, moving the surface at most {moved}."),
            };
            if shown.capped {
                text.push_str(&format!(
                    " Refining stopped at {MAX_TRIANGLES} triangles, so some edges are longer than asked; \
                     a longer edge keeps the whole mesh even."
                ));
            }
            text
        }
    };
    ui.add(egui::Label::new(theme::hint(text)).selectable(false));
}
