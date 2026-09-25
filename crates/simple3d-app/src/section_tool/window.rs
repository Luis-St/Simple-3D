//! The tool's own window.

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

/// The section's window, drawn over the viewport once a frame while the plane
/// is out (issue 72).
///
/// It stands open for exactly as long as the section is on -- there is no
/// separate switch for the window, because a section with its settings put away
/// and a section that is off are the same picture. The chevron is what puts the
/// window out of the way while the cut stays.
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if !app.scene.settings.section.enabled {
        return;
    }
    let bounds = app.viewport_rect;
    // A document opened with fewer sections than the last one had.
    app.section_tab = app.section_tab.min(app.scene.settings.section_count() - 1);
    // A plane from a file written before its centre was kept, or switched on
    // before there was a model, is fixed where it stands now -- from here on,
    // moving a body does not move it.
    let bounds_now = app.evaluated.bounds;
    for index in 0..app.scene.settings.section_count() {
        app.scene.settings.section_at_mut(index).pin_centre(bounds_now);
    }
    // Taken out of the map for the duration, so the popup may hold it mutably
    // while its contents hold the application.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event =
        popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: "Section", width: WIDTH }, |ui| {
            // Four rows and a hint is a short window until the rows stack on a
            // narrow one, and a viewport can be short: the body scrolls rather
            // than pushing the button off the bottom of the screen.
            popup::scrolling_body(ui, bounds, |ui| body(app, ui));
            popup::action_row(ui, |ui| actions(app, ui));
        });
    app.popups.insert(KEY, placement);
    // The cross means the same thing the button in the row means: the plane is
    // put away and the model is whole again.
    if event == PopupEvent::Closed && app.scene.settings.section.enabled {
        app.run(Command::ToggleSection);
    }
}

/// A turn of the plane about one axis, in degrees: any amount, read back as a
/// direction in [0, 360) the way a node's rotation is.
const TILT: ParamKind = ParamKind::Angle { min: 0.0, max: 360.0, wrap: true };

/// A side of the plane's own rectangle: any length but none at all.
const SIDE: ParamKind = ParamKind::Length { min: 1e-3 };

/// The plane's settings: the axis it stands perpendicular to, where along that
/// axis it sits, how far it is turned off it, and which side of it is cut away
/// (issues 71, 72, 109).
///
/// The offset is a scalar field like any other, so it can be typed exactly and
/// scrubbed with the pointer -- and the grips in the viewport slide the same
/// number. A plane that can only be dragged cannot be put at 12.5 mm, and one
/// that can only be typed cannot be swept through a part to find where the wall
/// gets thin.
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
                // Picking an axis is picking a plane square to it, so any turn
                // goes -- which is also the one-click way to straighten a plane
                // that has been turned, by picking the axis it is already on.
                app.section_mut().tilt = [0.0; 3];
                if !showing {
                    app.section_mut().axis = axis;
                    // The old offset is a place on a different axis, so the
                    // plane goes back to the middle of the model rather than to
                    // wherever that number happens to land on this one.
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
            // No undo step: moving the plane is not an edit -- see the module
            // header.
            scalar_field(app, ui, field, |app, mm, _| app.set_section_offset(mm));
        });
    });
    // Turned about the plane's centre, about each axis in turn, so a
    // wall that runs at a slant can be cut square to it (issue 109). The same
    // three chipped fields a node's rotation has, scrubbed by the same step.
    axis_row(app, ui, "Turn (deg)", |app, ui, axis, name| {
        let field = Scalar {
            grip: name,
            id: ui.id().with(("section-tilt", app.section_tab, axis)),
            kind: TILT,
            current: app.section().tilt[axis],
            step: app.settings.rotate_snap_deg.max(1.0),
        };
        // No undo step, for the same reason the offset has none.
        scalar_field(app, ui, field, |app, degrees, _| {
            app.section_mut().tilt[axis] = degrees;
            app.status = crate::app::Status::Info(readout(app));
        });
    });
    size_rows(app, ui);
    field_row(ui, "Keeps", "Which side of the plane stays in the picture", |ui| {
        // Auto keeps the far side from the camera; Below and Above are named by
        // the coordinate and stay put however the model is orbited; Motion
        // keeps what lies ahead of the way the plane was last slid.
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

/// The sections as tabs: which one the fields below are editing, and a way to
/// add another. Every section cuts at once, whichever is showing -- the tab
/// only says whose numbers these are.
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
        // As many as the renderer can cut with at once, and no more.
        let room = count < simple3d_geom::section::MAX_CUTS;
        let add = ui.add_enabled(room, egui::Button::new("+")).on_hover_text("Add a section that cuts as well");
        if add.clicked() {
            add_section(app);
        }
    });
}

/// A new section, standing on the axis after the one showing and in the middle
/// of the model along it: one on top of the other would cut nothing new and
/// could not be told apart from it.
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

/// How big the plane is: through the whole model, or a rectangle of its own
/// that only the material straight behind is cut away by. The two sides are
/// only shown while the size is custom -- an automatic plane has no numbers to
/// set, and two dead fields would be two rows of the model hidden for nothing.
fn size_rows(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    let hover = "Auto runs the plane through the whole model; Custom cuts out only the rectangle it is given";
    field_row(ui, "Size", hover, |ui| {
        for (custom, label) in [(false, "Auto"), (true, "Custom")] {
            let showing = app.section().custom_size == custom;
            if theme::choice(ui, showing, label).clicked() && !showing {
                let bounds = app.evaluated.bounds;
                let section = app.section_mut();
                // A first custom size is the frame as it already stands, so
                // switching over changes nothing on screen until a side is.
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
                // No undo step, for the same reason the offset has none.
                scalar_field(app, ui, field, |app, mm, _| {
                    app.section_mut().size[index] = mm;
                    app.status = crate::app::Status::Info(readout(app));
                });
            });
        });
    }
}

/// The buttons along the foot: put the plane away, take the section showing
/// away while there are others, or stand it back in the middle of the model.
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
    // At the other end of the row, the way the measure tool's Clear is: a plane
    // swept out past the model shows an uncut shape and no sign of what to do
    // about it, and orbiting round to find the frame again is the long way back.
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

/// What the plane is doing, for the status line: where it stands, and how far
/// it is turned when it is.
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

/// Put the plane back in the middle of the model, along the axis it is on.
///
/// What "on" means for a section that has just been switched on: an offset of
/// zero cuts nothing at all for a part that does not straddle the origin, and a
/// section that appears to do nothing reads as a broken one.
pub fn middle_of(bounds: Option<(Vec3, Vec3)>, axis: usize) -> f64 {
    match bounds {
        Some((lo, hi)) => (component(lo, axis) + component(hi, axis)) * 0.5,
        None => 0.0,
    }
}
