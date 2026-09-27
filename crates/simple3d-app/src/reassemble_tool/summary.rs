//! What the mesh turned out to be, in a sentence.

use super::*;
use crate::theme;
use simple3d_geom::reassemble::{Assembly, Shape};

/// Found bodies counted by kind ("12 objects: 5 boxes, 3 cylinders and 4 kept as meshes"), which
/// shows whether recognition worked; the outliner lists them after applying.
pub(crate) fn tally(assembly: &Assembly) -> String {
    let objects = assembly.objects();
    let mut counted: Vec<(&'static str, &'static str, usize)> = Vec::new();
    for part in &assembly.parts {
        match counted.iter_mut().find(|(singular, _, _)| *singular == part.shape.label()) {
            Some((_, _, count)) => *count += 1,
            None => counted.push((part.shape.label(), part.shape.plural(), 1)),
        }
    }
    // Shapes first, meshes last: what was recovered is the news.
    counted.sort_by_key(|(singular, _, count)| (*singular == Shape::Mesh.label(), std::cmp::Reverse(*count)));
    let mut kinds: Vec<String> = counted
        .into_iter()
        .map(|(singular, plural, count)| format!("{count} {}", if count == 1 { singular } else { plural }))
        .collect();
    if assembly.rest_bodies > 0 {
        kinds.push(format!("{} more bodies kept in one mesh", assembly.rest_bodies));
    }
    let groups = assembly.groups.iter().filter(|group| group.len() > 1).count();
    let assemblies = match groups {
        0 => String::new(),
        1 => ", in one group".to_string(),
        many => format!(", in {many} groups"),
    };
    format!("{objects} object{}: {}{assemblies}", crate::ui::plural(objects), list(&kinds))
}

/// "a, b and c", as a sentence reads.
fn list(items: &[String]) -> String {
    match items {
        [] => "nothing".to_string(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// What the run came to, or that it is still going.
pub(crate) fn summary(ui: &mut egui::Ui, tool: &ReassembleTool) {
    let text = match (&tool.found, &tool.job) {
        (_, Some(job)) if job.elapsed().as_millis() > 150 => "Working\u{2026}".to_string(),
        (None, _) => "Working\u{2026}".to_string(),
        (Some(found), _) if found.assembly.is_nothing() => {
            "Nothing was found in it: the mesh is one body, and it is not a shape this can rebuild. \
             There is nothing to reassemble."
                .to_string()
        }
        (Some(found), _) => tally(&found.assembly),
    };
    ui.add(egui::Label::new(theme::hint(text)).selectable(false));
    // Say when the preview is too large to draw, beside its checkbox, or it looks broken.
    if tool.outlines && tool.found.as_ref().is_some_and(|found| !found.drawn) {
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(theme::hint(format!(
                "More than {PREVIEW_LOOPS} lines is too much to draw over the model, so what was \
                 found is not shown. The window still says what it is."
            )))
            .selectable(false),
        );
    }
}
