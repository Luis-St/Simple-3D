//! What the mesh turned out to be, in a sentence.

use super::*;
use crate::theme;
use simple3d_geom::reassemble::{Assembly, Shape};

/// What was found, counted up by kind: "12 objects: 5 boxes, 3 cylinders and 4
/// kept as meshes".
///
/// Counted rather than listed. A list of every body is the outliner, which is
/// where it belongs once this is pressed; what has to be readable *before* it
/// is pressed is whether the recognition worked at all -- twenty boxes is a
/// reassembly, and twenty meshes is a mesh that has merely been chopped up.
pub(crate) fn tally(assembly: &Assembly) -> String {
    let objects = assembly.objects();
    let mut counted: Vec<(&'static str, &'static str, usize)> = Vec::new();
    for part in &assembly.parts {
        match counted.iter_mut().find(|(singular, _, _)| *singular == part.shape.label()) {
            Some((_, _, count)) => *count += 1,
            None => counted.push((part.shape.label(), part.shape.plural(), 1)),
        }
    }
    // The shapes first and the meshes last, whatever order the bodies came in:
    // what was recovered is the news, and what was not is the caveat.
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
    format!("{objects} object{}: {}{assemblies}", if objects == 1 { "" } else { "s" }, list(&kinds))
}

/// "a, b and c" -- the way a sentence reads rather than the way a list prints.
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
    // Said only when it is true, and said where the checkbox that turns it on
    // is looked at: a preview that silently does not appear reads as a setting
    // that does not work.
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
