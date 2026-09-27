//! The section tool's window.

use super::*;
use crate::app::App;
use crate::panel_properties::{axis_row, component, field_row, named, room_left, scalar_field, Scalar, POINT};
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::theme;
use crate::ui;
use simple3d_core::keymap::Command;
use simple3d_core::primitive::ParamKind;
use simple3d_core::scene::SectionKeep;
use simple3d_core::unit::format_length;
use simple3d_geom::Vec3;

/// The section's window, open exactly while the section is on (issue 72); the chevron folds it
/// away while the cut stays.
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if !app.scene.settings.section.enabled {
        return;
    }
    let bounds = app.viewport_rect;
    // A document may have fewer sections than the previous one.
    app.section_tab = app.section_tab.min(app.scene.settings.section_count() - 1);
    // Pin the centre of a plane from an older file, or enabled before there was a model.
    let bounds_now = app.evaluated.bounds;
    for index in 0..app.scene.settings.section_count() {
        app.scene.settings.section_at_mut(index).pin_centre(bounds_now);
    }
    // Taken out of the map so the popup can hold it mutably while the contents hold the app.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event =
        popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: "Section", width: WIDTH }, |ui| {
            // Scrolls on short viewports rather than pushing the button off screen.
            popup::scrolling_body(ui, bounds, |ui| body(app, ui));
            popup::action_row(ui, |ui| actions(app, ui));
        });
    app.popups.insert(KEY, placement);
    // The cross turns the section off, like the row's button.
    if event == PopupEvent::Closed && app.scene.settings.section.enabled {
        app.run(Command::ToggleSection);
    }
}

/// A plane turn in degrees, wrapped into [0, 360) like a node's rotation.
const TILT: ParamKind = ParamKind::Angle { min: 0.0, max: 360.0, wrap: true };

/// A side of the plane's rectangle: any positive length.
const SIDE: ParamKind = ParamKind::Length { min: 1e-3 };

/// The plane's settings: axis, offset, tilt and kept side (issues 71, 72, 109). The offset is
/// an ordinary scalar field, so it can be typed exactly or scrubbed, and the grips drive it too.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    tabs(app, ui);
    let unit = app.unit();
    let hover = "The axis the section plane stands perpendicular to. Picking one, or the one it is on, stands it \
                 straight again";
    field_row(ui, "Plane", hover, |ui| {
        for (axis, name) in ["X", "Y", "Z"].into_iter().enumerate() {
            let section = *app.section();
            let showing = section.axis() == axis;
            if theme::choice(ui, showing, name).clicked() && !(showing && !section.tilted()) {
                // Picking an axis resets the tilt, which is also the one-click way to straighten a plane.
                app.section_mut().tilt = [0.0; 3];
                if !showing {
                    app.section_mut().axis = axis;
                    // The old offset is meaningless on the new axis, so the plane goes to the model's middle.
                    let middle = middle_of(app.evaluated.bounds, axis);
                    app.set_section_offset(middle);
                    app.recentre_section();
                } else {
                    app.status = crate::app::Status::Info(readout(app));
                }
            }
        }
    });
    field_row(ui, &named("At", unit.suffix()), "Where the plane sits along its axis", |ui| {
        let step = unit.from_mm(app.move_snap()).max(1e-6);
        let width = room_left(ui).max(40.0);
        let id = ui.id().with(("section-offset", app.section_tab));
        ui.scope(|ui| {
            ui.set_width(width);
            let field = Scalar { grip: "Section", id, kind: POINT, current: app.section().offset, step };
            // No undo step: moving the plane is not an edit.
            scalar_field(app, ui, field, |app, mm, _| app.set_section_offset(mm));
        });
    });
    // Turned about the plane's centre per axis, to cut slanted walls square (issue 109).
    axis_row(app, ui, "Turn (deg)", |app, ui, axis, name| {
        let field = Scalar {
            grip: name,
            id: ui.id().with(("section-tilt", app.section_tab, axis)),
            kind: TILT,
            current: app.section().tilt[axis],
            step: app.settings.rotate_snap_deg.max(1.0),
        };
        // No undo step, as for the offset.
        scalar_field(app, ui, field, |app, degrees, _| {
            app.section_mut().tilt[axis] = degrees;
            app.status = crate::app::Status::Info(readout(app));
        });
    });
    size_rows(app, ui);
    field_row(ui, "Keeps", "Which side of the plane stays in the picture", |ui| {
        for (keep, label, hover) in [
            (SectionKeep::Auto, "Auto", "Cut away the side facing the camera"),
            (SectionKeep::Below, "Below", "Keep what is below the plane"),
            (SectionKeep::Above, "Above", "Keep what is above the plane"),
            (
                SectionKeep::Motion,
                "Motion",
                "Cut away what the plane has been slid past, and keep what lies ahead of it",
            ),
        ] {
            let showing = app.section().keep == keep;
            if theme::choice(ui, showing, label).on_hover_text(hover).clicked() && !showing {
                app.section_mut().keep = keep;
                app.status = crate::app::Status::Info(readout(app));
            }
        }
    });
}

/// The sections as tabs, choosing which one the fields edit; all sections cut at once.
fn tabs(app: &mut App, ui: &mut egui::Ui) {
    let hover = "Which section the fields below edit. Every section cuts at the same time";
    field_row(ui, "Section", hover, |ui| {
        let count = app.scene.settings.section_count();
        for which in 0..count {
            let showing = app.section_tab == which;
            if theme::choice(ui, showing, &(which + 1).to_string()).clicked() && !showing {
                app.section_tab = which;
                app.status = crate::app::Status::Info(readout(app));
            }
        }
        // As many as the renderer can cut with at once.
        let room = count < simple3d_geom::section::MAX_CUTS;
        let add = ui.add_enabled(room, egui::Button::new("+")).on_hover_text("Add a section that cuts as well");
        if add.clicked() {
            add_section(app);
        }
    });
}

/// A new section on the next axis through the model's middle, so it differs from the current one.
fn add_section(app: &mut App) {
    let axis = (app.section().axis() + 1) % 3;
    let fresh = simple3d_core::scene::SectionView {
        enabled: true,
        axis,
        offset: middle_of(app.evaluated.bounds, axis),
        centre: app.evaluated.bounds.map(|bounds| simple3d_core::scene::SectionView::pivot(Some(bounds))),
        ..Default::default()
    };
    app.section_tab = app.scene.settings.add_section(fresh);
    app.status = crate::app::Status::Info(readout(app));
}

/// The plane's size: whole model or a custom rectangle; side fields shown only when custom.
fn size_rows(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    let hover = "Auto runs the plane through the whole model; Custom cuts out only the rectangle it is given";
    field_row(ui, "Size", hover, |ui| {
        for (custom, label) in [(false, "Auto"), (true, "Custom")] {
            let showing = app.section().custom_size == custom;
            if theme::choice(ui, showing, label).clicked() && !showing {
                let bounds = app.evaluated.bounds;
                let section = app.section_mut();
                // The first custom size is the current frame, so switching changes nothing on screen.
                if custom && section.size.iter().any(|&side| side <= 0.0) {
                    section.size = auto_size(section, bounds);
                }
                section.custom_size = custom;
                app.status = crate::app::Status::Info(readout(app));
            }
        }
    });
    if !app.section().custom_size {
        return;
    }
    let step = unit.from_mm(app.move_snap()).max(1e-6);
    for (index, name, hover) in
        [(0, "Width", "The rectangle's size across the plane"), (1, "Height", "The rectangle's size up the plane")]
    {
        field_row(ui, &named(name, unit.suffix()), hover, |ui| {
            let width = room_left(ui).max(40.0);
            let id = ui.id().with(("section-size", app.section_tab, index));
            ui.scope(|ui| {
                ui.set_width(width);
                let current = app.section().size[index];
                let field = Scalar { grip: name, id, kind: SIDE, current, step };
                // No undo step, as for the offset.
                scalar_field(app, ui, field, |app, mm, _| {
                    app.section_mut().size[index] = mm;
                    app.status = crate::app::Status::Info(readout(app));
                });
            });
        });
    }
}

/// Footer buttons: turn off, remove the shown section, or recentre it.
pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    if ui::dialog_button(ui, "Done", true).clicked() {
        app.run(Command::ToggleSection);
    }
    let count = app.scene.settings.section_count();
    if count > 1
        && ui.add(egui::Button::new("Remove")).on_hover_text("Take this section away; the others stay").clicked()
    {
        app.scene.settings.remove_section(app.section_tab);
        app.section_tab = app.section_tab.min(count - 2);
        app.status = crate::app::Status::Info(readout(app));
    }
    // Recentre, at the other end like the measure tool's Clear, for a plane swept out of the model.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if ui
            .add(egui::Button::new("Back to the middle"))
            .on_hover_text("Stand the plane, and the point it turns about, in the middle of the model again")
            .clicked()
        {
            let middle = middle_of(app.evaluated.bounds, app.section().axis());
            app.set_section_offset(middle);
            app.recentre_section();
        }
    });
}

/// The plane's status line text: its position, and tilt if any.
pub fn readout(app: &App) -> String {
    let section = *app.section();
    let unit = app.unit();
    let name = match app.scene.settings.section_count() {
        1 => "Section".to_string(),
        _ => format!("Section {}", app.section_tab + 1),
    };
    let at = format!("{name} at {} = {} {}", section.axis_label(), format_length(section.offset, unit), unit.suffix());
    let at = match section.tilted() {
        true => {
            let [x, y, z] = section.tilt.map(simple3d_core::unit::format_angle);
            format!("{at}, turned {x} / {y} / {z} deg")
        }
        false => at,
    };
    let at = match section.custom_size {
        true => format!(
            "{at}, {} x {} {}",
            format_length(section.size[0], unit),
            format_length(section.size[1], unit),
            unit.suffix()
        ),
        false => at,
    };
    match section.keep {
        SectionKeep::Auto => at,
        SectionKeep::Below => format!("{at}, keeping what is below it"),
        SectionKeep::Above => format!("{at}, keeping what is above it"),
        SectionKeep::Motion => format!("{at}, keeping what lies ahead of it"),
    }
}

/// The model's middle along `axis`. Used when enabling, since offset zero may cut nothing.
pub fn middle_of(bounds: Option<(Vec3, Vec3)>, axis: usize) -> f64 {
    match bounds {
        Some((lo, hi)) => (component(lo, axis) + component(hi, axis)) * 0.5,
        None => 0.0,
    }
}
