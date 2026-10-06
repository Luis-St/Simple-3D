//! The edits an object holds, push/pull's (issue 73) and the round tool's (issue 88): each one's
//! number, and the two ways back out.

use super::*;
use crate::app::{App, Status};
use crate::ui::{self};
use simple3d_core::primitive::ParamKind;
use simple3d_core::scene::{NodeId, ObjectEdit, Placing, RoundKind};

/// What a row's buttons asked for, done after the list is drawn so the list is not changed under it.
enum Action {
    Extract(usize),
    Remove(usize),
}

/// How a row names its edit, what its number means, and what taking it out makes.
fn describe(edit: &ObjectEdit) -> (&'static str, &'static str, &'static str) {
    match edit {
        ObjectEdit::Push(edit) => match edit.placing {
            Placing::Add => ("Added", "How far the face was pushed out.", "push / pull"),
            Placing::Cut => ("Cut", "How far the face was pulled in.", "push / pull"),
        },
        ObjectEdit::Round(edit) => match edit.kind {
            RoundKind::Round => ("Rounded", "The radius the edges and corners were rounded to.", "rounding"),
            RoundKind::Chamfer => ("Bevelled", "How far back along each face the bevel starts.", "bevel"),
        },
    }
}

/// One row per edit, in the order they apply. Extracting one makes it nodes of its own and leaves
/// the model as it is; removing one reverts the object.
pub(crate) fn face_edits(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let unit = app.unit();
    let kind = ParamKind::Length { min: 0.01 };
    let mut action = None;
    let edits: Vec<(&'static str, &'static str, f64)> = app
        .scene
        .node(id)
        .edits
        .iter()
        .map(|edit| {
            let (name, help, _) = describe(edit);
            (name, help, edit.size())
        })
        .collect();
    for (index, (name, help, size)) in edits.into_iter().enumerate() {
        let label = format!("{} {name}", index + 1);
        field_row(ui, &named(&label, unit.suffix()), help, |ui| {
            let field_id = ui.id().with(("face-edit", id, index));
            // Each row's own grip, or two rows would scrub as one widget.
            let grip = format!("Edit size {}", index + 1);
            ui.scope(|ui| {
                ui.set_width((room_left(ui) - 120.0).max(40.0));
                let field =
                    Scalar { grip: &grip, id: field_id, kind, current: size, step: ui::scrub_increment(kind, unit) };
                scalar_field(app, ui, field, |app, mm, started| {
                    edit_or_touch(app, started, &format!("{name} size"), &format!("face-edit:{id}:{index}"));
                    app.scene.set_edit_size(id, index, mm);
                });
            });
            if ui.button("Extract").on_hover_text("Make it an object of its own; the model stays as it is").clicked() {
                action = Some(Action::Extract(index));
            }
            if ui.button("Remove").on_hover_text("Take it away, putting the object back as it was").clicked() {
                action = Some(Action::Remove(index));
            }
        });
    }
    match action {
        Some(Action::Extract(index)) => extract(app, id, index),
        Some(Action::Remove(index)) => {
            let Some(edit) = app.scene.node(id).edits.get(index) else { return };
            let what = describe(edit).2;
            app.edit(&format!("Remove {what}"), None);
            app.scene.remove_edit(id, index);
            app.status = Status::Info(format!("Removed the {what}; the object is back as it was"));
        }
        None => {}
    }
}

/// Take edit `index` out of `id` as nodes of its own, and select them.
fn extract(app: &mut App, id: NodeId, index: usize) {
    let Some(edit) = app.scene.node(id).edits.get(index) else { return };
    let what = describe(edit).2;
    // Tried on a copy, so a refusal leaves no empty undo step behind.
    let mut scene = app.scene.clone();
    let frames = &app.evaluated.node_frames;
    let made = match edit {
        ObjectEdit::Push(_) => scene.extract_face_edit(id, index, frames).map(|node| vec![node]),
        ObjectEdit::Round(_) => {
            // The cutters are remade against the model without the rounding, as it was made: on the
            // rounded one a corner already cut away no longer stops an edge's cutter there.
            let mut without = app.scene.clone();
            without.remove_edit(id, index);
            let model =
                simple3d_core::eval::Evaluator::new().evaluate(&without, &simple3d_core::eval::Cancel::new()).mesh;
            scene.extract_round_edit(id, index, frames, &app.evaluated.node_world_bounds, &model)
        }
    };
    let Some(made) = made.filter(|made| !made.is_empty()) else {
        app.status = Status::Warning(format!("That {what} could not be taken out"));
        return;
    };
    app.edit(&format!("Extract {what}"), None);
    app.scene = scene;
    app.select_only(made[0]);
    for &node in &made[1..] {
        app.toggle_selected(node);
    }
    let names: Vec<String> = made.iter().map(|&node| app.scene.node(node).name.clone()).collect();
    app.status = Status::Info(format!("Extracted the {what} as {}", names.join(" and ")));
}
