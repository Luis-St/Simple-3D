//! What the simplification came to, in two numbers.

use super::*;
use crate::app::App;
use crate::theme;
use simple3d_core::unit::{format_length, Unit};

/// A distance in the document's unit.
pub(crate) fn distance(mm: f64, unit: Unit) -> String {
    format!("{} {}", format_length(mm, unit), unit.suffix())
}

/// Result triangle count and surface deviation, or that a run is going. The deviation is an upper
/// bound against the original mesh, the only on-screen sign a result is safe to print.
pub(crate) fn summary(app: &App, ui: &mut egui::Ui, tool: &SimplifyTool) {
    let before = tool.original.triangle_count();
    let unit = app.unit();
    let text = match (&tool.shown, &tool.job) {
        (_, Some(job)) if job.elapsed().as_millis() > 150 => "Working\u{2026}".to_string(),
        (None, _) => "Working\u{2026}".to_string(),
        (Some(shown), _) if shown.mesh.triangle_count() >= before => format!(
            "Nothing was dropped: every collapse left is refused by what the settings keep. Ask for less \
             detail, or give up one of the three things below it. ({before} triangles)"
        ),
        (Some(shown), _) => {
            let after = shown.mesh.triangle_count();
            format!(
                "{before} triangles down to {after} ({}%), moving the surface at most {}.",
                (after * 100).div_ceil(before.max(1)),
                distance(shown.deviation, unit)
            )
        }
    };
    ui.add(egui::Label::new(theme::hint(text)).selectable(false));
    // Said beside its checkbox when the preview is not drawn, or it reads as broken (issue 23).
    let drawn = tool.shown.is_some()
        && app.evaluated.node_meshes.get(&tool.target).is_some_and(|mesh| mesh.triangle_count() > WIREFRAME_LIMIT);
    if tool.wireframe && drawn {
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(theme::hint(format!(
                "More than {WIREFRAME_LIMIT} triangles is too fine to draw over the shape, so the \
                 triangles are not shown. The shape in the viewport is the result itself."
            )))
            .selectable(false),
        );
    }
}
