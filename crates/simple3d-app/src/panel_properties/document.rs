//! The document's own settings, when nothing is selected.

use super::*;
use crate::app::App;
use crate::theme::{self};
use crate::ui::{self};
use simple3d_core::config::Placement;
use simple3d_core::primitive::ParamKind;
use simple3d_core::scene::{AxisStyle, PreviewViewport};
use simple3d_core::unit::Unit;

/// With nothing selected the dock shows the document, not a set of disabled
/// fields: units, grid, the default that governs every curved surface, and how
/// big the scene has become.
pub(crate) fn document(app: &mut App, ui: &mut egui::Ui) {
    section(ui, "Document", |ui| {
        let unit = app.unit();
        field_row(ui, "Unit", "", |ui| {
            // Every fixed width here is a ceiling, not a size: the control gives
            // up width with the panel rather than pushing past its edge (issue 51).
            egui::ComboBox::from_id_salt("doc-unit")
                .selected_text(theme::value(unit.suffix()))
                .width(fits(ui, 72.0))
                .show_ui(ui, |ui| {
                    for option in Unit::ALL {
                        // Switching never rescales the model: the unit only
                        // changes what the fields read (spec section 4).
                        if ui.selectable_label(unit == option, option.suffix()).clicked() {
                            app.scene.settings.unit = option;
                            app.fields.clear();
                        }
                    }
                });
        });
        field_row(ui, &named("Grid", unit.suffix()), "", |ui| {
            let kind = ParamKind::Length { min: 1e-6 };
            let spacing = app.scene.settings.grid_spacing;
            let width = room_left(ui).max(40.0);
            let id = ui.id().with("doc-grid");
            ui.scope(|ui| {
                ui.set_width(width);
                let field = Scalar { grip: "Grid", id, kind, current: spacing, step: ui::scrub_increment(kind, unit) };
                scalar_field(app, ui, field, |app, mm, started| {
                    edit_or_touch(app, started, "Grid spacing", "scene:grid");
                    app.scene.settings.grid_spacing = mm.min(MAX_LENGTH);
                });
            });
        });
        step_row(app, ui);
        rotate_step_row(app, ui);
        // Where a new shape lands. It is a document question -- the same one the
        // grid, the step and the segment default answer -- and it used to sit
        // under the palette, where it read as part of the shapes rather than as
        // a setting. The palette still says which answer is in force.
        field_row(ui, "Add at", "Where a shape from the palette or the Add menu lands", |ui| {
            egui::ComboBox::from_id_salt("doc-placement")
                .selected_text(theme::value(app.settings.placement.label()))
                .width(fits(ui, 150.0))
                .show_ui(ui, |ui| {
                    for option in Placement::ALL {
                        ui.selectable_value(&mut app.settings.placement, option, option.label());
                    }
                });
        });
        // When a drag snaps to another body's vertices, edge midpoints and face
        // centres rather than only to the grid step (issue 68). The hint names
        // the current hold key so the "while held" mode is not a mystery.
        let snap_key = app.keymap.shortcut_text(simple3d_core::keymap::Command::SnapToGeometry);
        // The hold names itself on the closed box too, not only in the open
        // list: the mode a user is *in* is the one they need the key for, and
        // "while a key is held" without saying which is a riddle.
        let snap_label = |mode: simple3d_core::config::SnapMode| {
            if mode == simple3d_core::config::SnapMode::WhileHeld && !snap_key.is_empty() {
                format!("{} ({snap_key})", mode.label())
            } else {
                mode.label().to_string()
            }
        };
        field_row(
            ui,
            "Snap to geometry",
            "Snap a drag to the vertices, edge midpoints and face centres of other bodies.",
            |ui| {
                egui::ComboBox::from_id_salt("geometry-snap")
                    .selected_text(theme::value(snap_label(app.settings.geometry_snap)))
                    .width(fits(ui, 190.0))
                    .show_ui(ui, |ui| {
                        for option in simple3d_core::config::SnapMode::ALL {
                            ui.selectable_value(&mut app.settings.geometry_snap, option, snap_label(option));
                        }
                    });
            },
        );
        // The 3D cursor, as three numbers. Shift+right-click in the viewport
        // puts it roughly where it is wanted; this is where it is given the
        // exact place (issue 42).
        cursor_rows(app, ui);
        // And the other place in space the document works from: what the camera
        // is looking at.
        view_centre_rows(app, ui);
        field_row(ui, "Axes", "", |ui| {
            for (axis, name) in ["X", "Y", "Z"].into_iter().enumerate() {
                let mut on = app.scene.settings.axes_visible[axis];
                if theme::toggle(ui, &mut on, name)
                    .on_hover_text(format!("Draw the {name} axis through the origin"))
                    .changed()
                {
                    app.scene.settings.axes_visible[axis] = on;
                }
            }
        });
        field_row(
            ui,
            "Axis style",
            "Along the grid: X and Y are the grid's own lines through zero and travel with it. \
                 Pinned: a cross at the origin that fades out at its own length.",
            |ui| {
                for option in AxisStyle::ALL {
                    let showing = app.scene.settings.axis_style == option;
                    if theme::choice(ui, showing, option.label()).clicked() && !showing {
                        app.scene.settings.axis_style = option;
                    }
                }
            },
        );
        field_row(
            ui,
            "In-place preview",
            "What the viewport does while a tool's window is drawing a preview over it. It goes back to normal \
                 when the tool closes.",
            |ui| {
                egui::ComboBox::from_id_salt("preview-viewport")
                    .selected_text(theme::value(app.scene.settings.preview_viewport.label()))
                    .width(fits(ui, 190.0))
                    .show_ui(ui, |ui| {
                        for option in PreviewViewport::ALL {
                            ui.selectable_value(&mut app.scene.settings.preview_viewport, option, option.label());
                        }
                    });
            },
        );
        // The section plane is not here: it is switched on from the tool rail,
        // the View menu or its key, and everything about it -- the axis, the
        // offset and which side is kept -- lives in the window that stands over
        // the viewport while it is out (issue 72). It had a group of rows here
        // when it was new, which put the settings for a plane drawn in the
        // picture on the other side of the application from the picture.
        field_row(
            ui,
            "Plane marks",
            "Mark on a shape's surface where the ground plane, or either upright plane, cuts it",
            |ui| {
                let mut on = app.scene.settings.plane_marks;
                if ui.checkbox(&mut on, "").changed() {
                    app.scene.settings.plane_marks = on;
                }
            },
        );
        field_row(ui, "Segments", "", |ui| {
            let segments = app.scene.settings.default_segments as f64;
            let width = room_left(ui).max(40.0);
            let id = ui.id().with("doc-segments");
            ui.scope(|ui| {
                ui.set_width(width);
                let field = Scalar { grip: "Segments", id, kind: SEGMENTS, current: segments, step: 1.0 };
                scalar_field(app, ui, field, |app, count, started| {
                    edit_or_touch(app, started, "Default segments", "scene:segments");
                    app.scene.settings.default_segments = count as u32;
                });
            });
        });
        ui.add(
            egui::Label::new(theme::hint("Curves are circumscribed: a diameter of 50 measures 50 at its widest."))
                .selectable(false),
        );
    });
    section(ui, "Scene", |ui| {
        let unit = app.unit();
        match app.evaluated.mesh.bounds() {
            Some((lo, hi)) => {
                field_row(ui, "Bounds", "", |ui| {
                    ui.add(egui::Label::new(theme::numeric(ui::describe_size(hi - lo, unit))).selectable(false).wrap());
                });
            }
            None => {
                ui.add(egui::Label::new(theme::hint("Nothing in the scene yet.")).selectable(false));
            }
        }
        ui.add(
            egui::Label::new(theme::hint("Select a shape to edit it, or pick one from the palette.")).selectable(false),
        );
    });
}
