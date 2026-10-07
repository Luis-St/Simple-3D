//! The settings for putting objects onto a face, and the viewport's click that picks it (issue 118).

use super::*;
use crate::app::{App, Status};
use crate::view::View;
use crate::{popup, theme};
use simple3d_core::primitive::ParamKind;
use simple3d_core::scene::NodeId;

pub(super) fn onto_controls(app: &mut App, ui: &mut egui::Ui, tool: &mut ArrangeTool) {
    let hint = match tool.landing {
        None => "Click a face of another object in the viewport.",
        Some(_) => "Click another face to change it; a right-click lets it go.",
    };
    ui.add(egui::Label::new(theme::hint(hint)).selectable(false));
    ui.add_space(4.0);
    egui::Grid::new("arrange-onto").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        popup::label(ui, "Where", "Where on the face the objects' middle lands.");
        ui.horizontal(|ui| {
            for (onto, name, hover) in [
                (OntoAt::Centre, "Face centre", "The middle of the flat face, or the clicked point on a curved one."),
                (OntoAt::Clicked, "Where clicked", "The point the face was clicked at."),
                (OntoAt::Below, "Below them", "Straight below their middle, as if they fell onto the face."),
            ] {
                if theme::choice(ui, tool.onto == onto, name).on_hover_text(hover).clicked() {
                    tool.onto = onto;
                }
            }
        });
        ui.end_row();

        popup::label(ui, "Turn", "Turn the objects so their own bottom faces the face, or keep their turn.");
        ui.checkbox(&mut tool.turn, "Bottom onto the face");
        ui.end_row();

        popup::label(ui, &format!("Gap ({})", app.unit().suffix()), "How far above the face they rest.");
        number(app, ui, "lift", ParamKind::Length { min: 0.0 }, &mut tool.lift);
        ui.end_row();
    });
}

/// A click picks the face to put the objects onto; a right-click lets it go.
pub(crate) fn onto_interact(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    if response.clicked_by(egui::PointerButton::Secondary) {
        if let Some(tool) = app.arrange_tool.as_mut() {
            tool.landing = None;
        }
        return;
    }
    if !response.clicked_by(egui::PointerButton::Primary) {
        return;
    }
    let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) else { return };
    match landing_under(app, view, cursor) {
        Some(landing) => {
            if let Some(tool) = app.arrange_tool.as_mut() {
                tool.landing = Some(landing);
            }
        }
        None => app.status = Status::Info("Click a face of another object to put the objects onto".into()),
    }
}

/// The face under `cursor`, passing through the objects being put down: a flat face lands them on
/// its middle, a facet of a curved one on the clicked point.
pub(crate) fn landing_under(app: &App, view: &View, cursor: egui::Pos2) -> Option<Landing> {
    let (at, triangle, normal) = hit_under(app, view, cursor)?;
    let mesh = &app.evaluated.mesh;
    let face = simple3d_geom::push_pull::flat_face(mesh, triangle).filter(|face| !face.curved);
    Some(match face {
        Some(face) => {
            let normal = if face.normal.dot(normal) < 0.0 { face.normal * -1.0 } else { face.normal };
            Landing { centre: face.centre, at, normal }
        }
        None => Landing { centre: at, at, normal },
    })
}

/// Where the pointer meets the model outside the objects being put down, and the normal there.
pub(crate) fn hit_under(app: &App, view: &View, cursor: egui::Pos2) -> Option<(Vec3, usize, Vec3)> {
    let tool = app.arrange_tool.as_ref()?;
    let moving = tool.arranged(&app.scene);
    let (origin, dir) = view.ray(cursor);
    crate::pick::surface_hit(&app.evaluated.mesh, origin, dir, |source| {
        let owner = NodeId::from(source);
        moving.iter().any(|&id| id == owner || app.scene.is_ancestor_of(id, owner))
    })
}
