//! The property editor (spec section 7.3).
//!
//! Every field here is generated from the selected primitive's declaration in
//! the registry -- there is no per-primitive code. Values commit on Enter and on
//! leaving the field; unparseable text restores the previous value silently.
//!
//! The panel set follows the selection rather than greying out: with nothing
//! selected the dock shows the document's own settings, which is information
//! the user can actually act on, instead of a column of dead fields.

use crate::app::{App, Status};
use crate::theme::{self, token};
use crate::ui::{self, Commit};
use simple3d_core::config::Placement;
use simple3d_core::primitive::{ParamKind, ParamValue, ParamsExt};
use simple3d_core::scene::{Anchor, AxisStyle, Body, Colour, GroupOp, Node, NodeId, Visibility};
use simple3d_core::unit::{format_angle, format_length, format_number, Unit};
use simple3d_geom::Vec3;

/// A collapsible panel in the right dock: a header bar, and a padded body that
/// is only drawn when the panel is open.
fn section(ui: &mut egui::Ui, name: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    section_titled(ui, name, "", add_contents)
}

/// A section whose header carries a second, quieter word on the right: the
/// primitive's type beside "Dimensions", so the panel keeps a stable name while
/// still saying what is selected.
fn section_titled(ui: &mut egui::Ui, name: &str, note: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    let id = ui.id().with(("section", name));
    let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(true);
    let header = theme::panel_header(ui, name, |ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        theme::twisty(ui.painter(), rect.center(), open, token::TEXT_LO);
        if !note.is_empty() {
            ui.add(egui::Label::new(theme::hint(note)).selectable(false));
        }
    });
    if header.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(id, open));
    }
    if !open {
        return;
    }
    egui::Frame::NONE.inner_margin(egui::Margin { left: 8, right: 8, top: 6, bottom: 8 }).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
        add_contents(ui);
    });
}

/// The label column of a property row. Fixed width, so every panel's fields
/// start at the same x and a column of numbers reads as a column.
const LABEL_WIDTH: f32 = 84.0;

/// The narrowest a row can be and still be worth splitting into a label column
/// and a field column: the column itself, plus enough beside it for a number
/// and its unit. Below this the row stacks instead (issue 51).
const STACK_BELOW: f32 = LABEL_WIDTH + 130.0;

/// The narrowest an axis field can be and still show a measurement. Three of
/// them side by side below this is three fields nobody can read, so they stack.
const MIN_AXIS_FIELD: f32 = 52.0;

/// The margin a section's frame keeps at its right-hand edge, which is the
/// edge every row in it has to stay inside.
const EDGE_PAD: f32 = 8.0;

/// How a section renders the parameter rows it shares with every other
/// section: what its edits are called in the undo history, and where a value's
/// unit is written.
#[derive(Clone, Copy)]
struct RowStyle {
    /// What the undo step is called. A primitive's choices really are
    /// measurements -- "outer diameter or wall thickness" -- but a pattern's
    /// are its kind and its axis, and filing those under "Set measurement" made
    /// the undo history describe something the user had not done.
    edit_label: &'static str,
    unit: UnitPlace,
}

/// A shape's dimensions: the unit rides after the number it qualifies.
const DIMENSION_ROW: RowStyle = RowStyle { edit_label: "Set measurement", unit: UnitPlace::AfterField };
/// A pattern's numbers: the unit goes in the name, so the count and the
/// distances line up down one column.
const PATTERN_ROW: RowStyle = RowStyle { edit_label: "Set pattern", unit: UnitPlace::InLabel };

/// Where a value row writes its unit.
#[derive(Clone, Copy, PartialEq, Eq)]
enum UnitPlace {
    /// After the field, one size down and in the label colour. What the shape
    /// editors use: there the row's name is the measurement and the unit
    /// qualifies the number beside it.
    AfterField,
    /// In brackets on the end of the row's name, the way the transform rows
    /// read. The fields on the section are then all one width, whether their
    /// value carries a unit or not -- a pattern's copies count has no unit and
    /// its steps do, and the two used to sit at different widths down the same
    /// column.
    InLabel,
}

/// How much room is left on the line a row is currently laying out on:
/// from where the next control will start to the row's own right-hand edge.
///
/// Not [`egui::Ui::available_width`]. In a *wrapped* horizontal layout that
/// reports the width a new line would have, not what is left on this one -- so
/// a field sized by it started after the label column and still asked for the
/// whole row, and ran that far past the panel's edge (issue 57). The row's
/// right-hand edge is pinned by [`field_row`] before anything is drawn in it,
/// which is what makes this exact.
fn room_left(ui: &egui::Ui) -> f32 {
    (ui.max_rect().right() - ui.cursor().left()).max(0.0)
}

/// The width a label takes at the size the panel writes its asides in, plus the
/// gap before it -- what a control has to leave behind for a suffix.
fn suffix_room(ui: &egui::Ui, text: &str) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let width = ui.fonts(|fonts| {
        fonts.layout_no_wrap(text.to_string(), egui::FontId::proportional(theme::font::SMALL), token::TEXT_LO).size().x
    });
    width + ui.spacing().item_spacing.x
}

/// A width a control would like, capped at what the row actually has left.
fn fits(ui: &egui::Ui, wanted: f32) -> f32 {
    wanted.min(room_left(ui)).max(48.0)
}

/// Whether the panel is too narrow for a label column beside the fields.
fn stacked(ui: &egui::Ui) -> bool {
    ui.available_width() < STACK_BELOW
}

/// A row's name, in its own column, wrapped inside that column rather than
/// running under the field beside it.
fn row_label(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let galley = ui.painter().layout(
        text.to_string(),
        egui::FontId::proportional(theme::font::LABEL),
        token::TEXT_LO,
        LABEL_WIDTH,
    );
    // The column keeps its width whatever the name does with it, and grows
    // downwards for a name that needed two lines, so the field beside it is
    // still where a field is expected.
    let height = galley.size().y.max(theme::metric::INPUT_ROW);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(LABEL_WIDTH, height), egui::Sense::hover());
    ui.painter().galley(egui::pos2(rect.left(), rect.center().y - galley.size().y / 2.0), galley, token::TEXT_LO);
    response
}

/// One property row: what it is called, and whatever edits it.
///
/// Given the width for it, the name is a fixed column with the controls beside
/// it, so a column of numbers lines up down the panel. Dragged in narrower than
/// that, the name goes on its own line and the controls take the full width
/// underneath, rather than the two of them squeezing a field down to nothing or
/// pushing it off the panel edge (issue 51). Either way the controls are laid
/// out wrapped, so a row of choices that no longer fits across breaks onto a
/// second line instead of overflowing.
fn field_row(ui: &mut egui::Ui, label: &str, hover: &str, contents: impl FnOnce(&mut egui::Ui)) {
    if stacked(ui) {
        ui.vertical(|ui| {
            if !label.is_empty() {
                let name = ui.add(
                    egui::Label::new(egui::RichText::new(label).size(theme::font::LABEL).color(token::TEXT_LO))
                        .selectable(false)
                        .wrap(),
                );
                if !hover.is_empty() {
                    name.on_hover_text(hover);
                }
            }
            ui.horizontal_wrapped(contents);
        });
        return;
    }
    // The row's right-hand edge, fixed before anything is laid out inside it. A
    // wrapped horizontal ui lets its `max_rect` grow to hold whatever overflowed
    // it, so a row that ran off the panel once went on doing so for as long as
    // the panel was open. Pinned here, it cannot, and `room_left` is exact.
    let right = ui.max_rect().right().min(ui.clip_rect().right() - EDGE_PAD);
    ui.horizontal_wrapped(|ui| {
        ui.set_max_width((right - ui.max_rect().left()).max(LABEL_WIDTH));
        let name = row_label(ui, label);
        if !hover.is_empty() {
            name.on_hover_text(hover);
        }
        contents(ui);
    });
}

/// The id of a value field's scrub gesture, named after the value it scrubs
/// rather than taken from where the field sits in the layout. A gesture in
/// flight is remembered by this id (`App::scrub`), so a panel that relays itself
/// out mid-drag -- a section collapsing, a dock being resized -- must not change
/// it. It is also what lets a test put the pointer on a named field.
pub fn grip_id(name: &str) -> egui::Id {
    egui::Id::new(("scrub-grip", name))
}

/// One value field: a number that can be typed into or dragged, named by
/// `name` so the gesture survives the panel relaying itself out.
fn value_field(app: &mut App, ui: &mut egui::Ui, name: &str, field_id: egui::Id, shown: &str, step: f64) -> ui::Field {
    // The scrub state is lifted out and put back so the field can borrow the
    // buffers mutably without borrowing the whole application twice.
    let mut scrub = app.scrub;
    let outcome = app.fields.scrub_field(ui, field_id, grip_id(name), shown, step, &mut scrub);
    app.scrub = scrub;
    outcome
}

/// The panel's contents, without the dock around them, so the same panel can be
/// drawn in either dock.
pub fn show_inside(app: &mut App, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(theme::metric::GAP, 2.0);
    let (area, restore) = theme::list_scroll_area(ui);
    area.show(ui, |ui| {
        ui.set_style(restore);
        // Everything the panel edits, primary last -- the same order the
        // selection itself is in, so "the one being edited" is unambiguous.
        let targets: Vec<NodeId> = app.selection.iter().copied().filter(|id| app.scene.contains(*id)).collect();
        // The measure tool's own section, and only while it is out (issue 78):
        // the span it is holding is what the panel is for at that moment, and
        // with the tool put away there is nothing for the section to say. It
        // comes first because it is what the user is doing, and it is here rather
        // than in either branch below because a measurement has nothing to do
        // with what happens to be selected.
        if app.measure.active {
            section(ui, "Measure", |ui| measure(app, ui));
        }
        let Some(primary) = app.primary() else {
            document(app, ui);
            return;
        };
        section(ui, "Object", |ui| common(app, ui, &targets));
        match shared_type(app, &targets) {
            Some(type_id) => {
                let label = simple3d_core::primitive::lookup(&type_id).map(|s| s.label).unwrap_or("");
                let note =
                    if targets.len() > 1 { format!("{} \u{00D7} {label}", targets.len()) } else { label.to_string() };
                section_titled(ui, "Dimensions", &note, |ui| primitive(app, ui, &targets, &type_id));
            }
            None => match app.scene.node(primary).body.clone() {
                Body::Group { op } if targets.len() == 1 => section(ui, "Boolean", |ui| group(app, ui, primary, op)),
                Body::Pattern { .. } if targets.len() == 1 => section(ui, "Pattern", |ui| pattern(app, ui, primary)),
                // A selection of different types has no shared dimension to
                // offer. Saying so beats an empty panel or a set of fields that
                // would edit only one of them without saying which.
                _ => section(ui, "Dimensions", |ui| {
                    ui.add(
                        egui::Label::new(theme::hint(
                            "The selection mixes shapes, so there is no dimension they share. Transform below still \
                             applies to all of them.",
                        ))
                        .selectable(false),
                    );
                }),
            },
        }
        section(ui, "Transform", |ui| placement(app, ui, &targets));
        section(ui, "Measured", |ui| measurements(app, ui, primary, targets.len()));
    });
}

/// The primitive type every selected node has, or `None` when they are not all
/// the same kind of thing. This is what decides whether a Dimensions panel can
/// speak for the whole selection.
fn shared_type(app: &App, targets: &[NodeId]) -> Option<String> {
    let mut found: Option<String> = None;
    for id in targets {
        let Body::Primitive { type_id, .. } = &app.scene.node(*id).body else { return None };
        match &found {
            Some(first) if first != type_id => return None,
            Some(_) => {}
            None => found = Some(type_id.clone()),
        }
    }
    found
}

/// With nothing selected the dock shows the document, not a set of disabled
/// fields: units, grid, the default that governs every curved surface, and how
/// big the scene has become.
fn document(app: &mut App, ui: &mut egui::Ui) {
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
        field_row(ui, "Grid", "", |ui| {
            let mut spacing = unit.from_mm(app.scene.settings.grid_spacing);
            if ui.add(egui::DragValue::new(&mut spacing).range(1e-6..=1e6).speed(0.1)).changed() {
                app.edit("Grid spacing", Some("scene:grid"));
                app.scene.settings.grid_spacing = unit.to_mm(spacing).max(1e-6);
            }
            ui.add(egui::Label::new(theme::hint(unit.suffix())).selectable(false));
        });
        step_row(app, ui);
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
        field_row(ui, "Axes", "", |ui| {
            for (axis, name) in ["X", "Y", "Z"].into_iter().enumerate() {
                let mut on = app.scene.settings.axes_visible[axis];
                if ui
                    .toggle_value(&mut on, name)
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
                    if ui.selectable_label(showing, option.label()).clicked() && !showing {
                        app.scene.settings.axis_style = option;
                    }
                }
            },
        );
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
            let mut segments = app.scene.settings.default_segments as f64;
            if ui.add(egui::DragValue::new(&mut segments).range(3.0..=512.0).speed(0.5).max_decimals(0)).changed() {
                app.edit("Default segments", Some("scene:segments"));
                app.scene.settings.default_segments = segments.round() as u32;
            }
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

/// Where the 3D cursor is, to the millimetre.
///
/// The cursor is `None` when it has never been moved, which means the origin,
/// so the fields read zero and typing into one places it: the same two states
/// the viewport draws, without a third way of saying "nowhere".
fn cursor_rows(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    let at = app.cursor.unwrap_or(Vec3::ZERO);
    let mut components = [at.x, at.y, at.z];
    let mut changed = false;
    field_row(
        ui,
        "3D cursor",
        "Where a new shape lands when \u{201C}Add at\u{201D} is the cursor. \
             Shift+right-click in the viewport puts it under the pointer.",
        |ui| {
            // Three numbers and a unit out of whatever the row has: they shrink
            // with the panel, and wrap onto a second line rather than shrinking
            // past being readable (issue 51).
            let each = ((ui.available_width() - 26.0) / 3.0 - ui.spacing().item_spacing.x).clamp(44.0, 56.0);
            for (axis, value) in components.iter_mut().enumerate() {
                let mut shown = unit.from_mm(*value);
                let field =
                    egui::DragValue::new(&mut shown).speed(unit.from_mm(app.move_snap()).max(1e-6)).max_decimals(4);
                let response = ui.add_sized(egui::vec2(each, theme::metric::INPUT_ROW), field);
                if response.changed() {
                    *value = unit.to_mm(shown);
                    changed = true;
                }
                let _ = axis;
            }
            ui.add(egui::Label::new(theme::hint(unit.suffix())).selectable(false));
        },
    );
    if changed {
        app.cursor = Some(Vec3::new(components[0], components[1], components[2]));
    }
    field_row(ui, "", "", |ui| {
        if ui
            .add_enabled(app.cursor.is_some(), egui::Button::new("Back to the origin"))
            .on_hover_text("The cursor goes back to 0, 0, 0")
            .clicked()
        {
            app.cursor = None;
            app.status = Status::Info("3D cursor back at the origin".into());
        }
    });
}

/// The measure tool's span as numbers: both ends as editable fields, and the
/// distance, per-axis delta and angles between them (issues 69, 78).
///
/// The ends are editable because a measurement is often *between* named places
/// rather than between two things there is geometry to point at -- and because
/// having clicked one end approximately, correcting it by a tenth of a
/// millimetre should not mean clicking again and hoping.
fn measure(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    let placed = app.measure.points.len();
    for (index, label) in [(0_usize, "Start"), (1, "End")] {
        let point = app.measure.points.get(index).copied();
        // An end can be typed only once the start is down; before that it would
        // be a point with nothing to measure to.
        let enabled = index <= placed;
        let mut components = match point {
            Some(p) => [p.at.x, p.at.y, p.at.z],
            None => [0.0; 3],
        };
        let mut changed = false;
        let hover = match point.and_then(|p| p.kind) {
            Some(kind) => format!("Caught the {} of a body. Type here to place it exactly.", kind.label()),
            None if point.is_some() => "Click in the viewport to move it, or type it exactly.".to_string(),
            None => "Click in the viewport to place it, or type it here.".to_string(),
        };
        field_row(ui, label, &hover, |ui| {
            let each = ((ui.available_width() - 26.0) / 3.0 - ui.spacing().item_spacing.x).clamp(44.0, 56.0);
            for value in components.iter_mut() {
                let mut shown = unit.from_mm(*value);
                let field =
                    egui::DragValue::new(&mut shown).speed(unit.from_mm(app.move_snap()).max(1e-6)).max_decimals(4);
                let response =
                    ui.add_enabled_ui(enabled, |ui| ui.add_sized(egui::vec2(each, theme::metric::INPUT_ROW), field));
                if response.inner.changed() {
                    *value = unit.to_mm(shown);
                    changed = true;
                }
            }
            ui.add(egui::Label::new(theme::hint(unit.suffix())).selectable(false));
        });
        if changed {
            app.measure.set_point(index, Vec3::new(components[0], components[1], components[2]));
        }
    }

    match app.measure.span() {
        Some((a, b)) => {
            let m = crate::app::Measurement::between(a.at, b.at);
            let suffix = unit.suffix();
            field_row(ui, "Distance", "", |ui| {
                ui.add(
                    egui::Label::new(theme::numeric(format!("{} {suffix}", format_length(m.distance, unit))))
                        .selectable(false)
                        .wrap(),
                );
            });
            field_row(ui, "\u{0394}", "The span, axis by axis", |ui| {
                ui.add(
                    egui::Label::new(theme::numeric(format!(
                        "{}, {}, {} {suffix}",
                        format_length(m.delta.x, unit),
                        format_length(m.delta.y, unit),
                        format_length(m.delta.z, unit)
                    )))
                    .selectable(false)
                    .wrap(),
                );
            });
            field_row(ui, "Angle", "Above the ground plane, and around it from +X towards +Y", |ui| {
                ui.add(
                    egui::Label::new(theme::numeric(format!(
                        "{}\u{00B0} incline   {}\u{00B0} bearing",
                        format_angle(m.inclination_deg),
                        format_angle(m.bearing_deg)
                    )))
                    .selectable(false)
                    .wrap(),
                );
            });
        }
        None => {
            ui.add(
                egui::Label::new(theme::hint(
                    "Click two features in the viewport. The pointer catches corners, edges, face centres, the \
                     marks the planes through zero leave on a body, and the axes themselves -- whatever the frame \
                     shows; right-click takes the last one back.",
                ))
                .selectable(false),
            );
        }
    }
    field_row(ui, "", "", |ui| {
        // Named for what it clears: the panel has another Clear in it, and a
        // button that only says "Clear" beside a set of numbers is a question.
        if ui.add_enabled(placed > 0, egui::Button::new("Clear the span")).clicked() {
            app.measure.clear();
            app.status = Status::Info("Measurement cleared".into());
        }
        if ui.button("Put the tool away").clicked() {
            app.toggle_measure();
        }
    });
}

fn common(app: &mut App, ui: &mut egui::Ui, targets: &[NodeId]) {
    let Some(&id) = targets.last() else { return };
    let node = app.scene.node(id);
    let is_root = id == app.scene.root();
    let mut name = node.name.clone();
    let visibility = node.visibility();
    let mixed_visibility = targets.iter().any(|t| app.scene.node(*t).visibility() != visibility);
    let mut anchor = node.anchor;
    let many = targets.len() > 1;

    field_row(ui, "Name", "", |ui| {
        if many {
            // Renaming several nodes to one name would make the outliner
            // unreadable, so the field says what is selected instead.
            ui.add(egui::Label::new(theme::value(format!("{} objects selected", targets.len()))).selectable(false));
        } else if ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY)).changed() {
            app.edit("Rename", Some(&format!("name:{id}")));
            if let Some(node) = app.scene.get_mut(id) {
                node.name = name;
            }
        }
    });

    // Three states rather than a checkbox: hidden means *gone*, and a body that
    // has to be seen while it is positioned -- the one about to be subtracted --
    // is a ghost, which is a property of that body and not of the document.
    field_row(
        ui,
        "Shown",
        "Visible: part of the model. Ghost: excluded from the model, drawn as a translucent shell. \
             Hidden: excluded and not drawn at all.",
        |ui| {
            ui.add_enabled_ui(!is_root, |ui| {
                for option in Visibility::ALL {
                    let showing = !mixed_visibility && visibility == option;
                    if ui.selectable_label(showing, option.label()).clicked()
                        && (mixed_visibility || visibility != option)
                    {
                        app.edit("Visibility", None);
                        for target in targets {
                            if let Some(node) = app.scene.get_mut(*target) {
                                node.set_visibility(option);
                            }
                        }
                    }
                }
            });
        },
    );

    field_row(
        ui,
        "Colour",
        "What this node is painted. Painting a group paints everything in it, \
             and the colour follows each surface through a boolean.",
        |ui| {
            // The swatch starts from whatever the node shows now -- its own colour,
            // one inherited from a group above it, or the theme's colour for an
            // unpainted solid -- so opening the picker never jumps to black.
            let inherited = app.scene.effective_colour(id);
            let mut rgb = inherited.map_or_else(|| unpainted_swatch(ui.visuals().dark_mode), |c| c.0);
            let mixed = targets.iter().any(|t| app.scene.effective_colour(*t) != inherited);
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                // One undo step for a whole drag through the picker, the way a
                // scrubbed field is one step.
                app.paint(targets, Some(Colour(rgb)), Some("colour"));
            }
            // Enabled only where clearing would do something: a node that merely
            // inherits a group's colour has none of its own to take away.
            let painted = targets.iter().any(|t| app.scene.subtree_is_painted(*t));
            if ui.add_enabled(painted, egui::Button::new("Clear")).on_hover_text("Back to the theme's colour").clicked()
            {
                app.paint(targets, None, None);
            }
            if mixed {
                ui.add(egui::Label::new(theme::value("mixed")).selectable(false));
            }
        },
    );

    // The same swatches the outliner's menu offers, and the colours this
    // document has actually been painted in: opening the picker to find a
    // colour that is already in the project is the slow way round.
    swatch_row(app, ui, "", &theme::PAINT_PRESETS.map(|(name, colour)| (name.to_string(), colour)), targets);
    let recent: Vec<(String, egui::Color32)> = app
        .custom_recent_colours()
        .iter()
        .map(|c| (format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]), egui::Color32::from_rgb(c[0], c[1], c[2])))
        .collect();
    if !recent.is_empty() {
        swatch_row(app, ui, "Recent", &recent, targets);
    }

    field_row(ui, "Anchor", "Where this node's origin sits. Changing it moves the origin, never the shape.", |ui| {
        let mixed = targets.iter().any(|t| app.scene.node(*t).anchor != anchor);
        for option in Anchor::ALL {
            let showing = !mixed && anchor == option;
            if ui.selectable_label(showing, option.label()).clicked() && (mixed || anchor != option) {
                anchor = option;
                app.edit("Anchor", None);
                for target in targets {
                    if let Some(node) = app.scene.get_mut(*target) {
                        node.anchor = anchor;
                    }
                }
            }
        }
    });
}

/// A row of colour swatches that paints the selection when one is clicked.
///
/// Plain buttons rather than a picker, for the same reason the outliner's menu
/// uses them: one click, and the colour is on the shape.
fn swatch_row(app: &mut App, ui: &mut egui::Ui, label: &str, colours: &[(String, egui::Color32)], targets: &[NodeId]) {
    let mut chosen: Option<Colour> = None;
    // The label column is kept even when empty, so the swatches line up under
    // the picker rather than under the labels.
    field_row(ui, label, "", |ui| {
        for (name, colour) in colours {
            let swatch = egui::Button::new("")
                .fill(*colour)
                .stroke(egui::Stroke::new(1.0_f32, token::SURFACE_3))
                .min_size(egui::vec2(16.0, 16.0));
            if ui.add(swatch).on_hover_text(name).clicked() {
                chosen = Some(Colour([colour.r(), colour.g(), colour.b()]));
            }
        }
    });
    if let Some(colour) = chosen {
        app.paint(targets, Some(colour), None);
    }
}

/// The colour an unpainted solid is drawn in, which is where the picker starts.
fn unpainted_swatch(dark: bool) -> [u8; 3] {
    let solid = crate::render::Palette::for_dark_mode(dark).solid;
    [solid[0], solid[1], solid[2]]
}

/// The pattern editor (issue 67): the kind and its numbers, driven from the
/// shared parameter list, plus a line saying what it currently makes.
fn pattern(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let params = app.scene.node(id).params().cloned().unwrap_or_default();
    let unit = app.unit();
    let targets = [id];
    for param in simple3d_core::pattern::PARAMS {
        if !simple3d_core::pattern::param_visible(param, &params) {
            continue;
        }
        param_field(app, ui, &targets, id, param, unit, PATTERN_ROW);
    }
    let (wanted, copies) = simple3d_core::pattern::instance_count(&params);
    let children = app.scene.node(id).children.len();
    let note = if children == 0 {
        "Put shapes under this pattern in the outliner -- or add one with it selected -- and it repeats them."
            .to_string()
    } else {
        format!("{copies} copies of {children} shape{}.", if children == 1 { "" } else { "s" })
    };
    ui.add(egui::Label::new(theme::hint(note)).selectable(false));
    // The numbers above are not the only way in: every one of them that places
    // a copy has a handle in the viewport, on the copy it places (issue 67).
    let grips = app.pattern_grips(id).len();
    if grips > 0 {
        ui.add(
            egui::Label::new(theme::hint(format!(
                "{grips} handle{} in the viewport lay{} this out by eye.",
                if grips == 1 { "" } else { "s" },
                if grips == 1 { "s" } else { "" }
            )))
            .selectable(false),
        );
    }
    // Said out loud rather than silently drawing fewer: a grid multiplies its
    // three counts, so it is easy to ask for a hundred million copies without
    // meaning to, and a pattern that quietly stopped short would just look wrong.
    if wanted > copies {
        ui.add(
            egui::Label::new(theme::hint(format!(
                "Capped at {copies} -- {wanted} copies were asked for, which is more than can be drawn."
            )))
            .selectable(false),
        );
    }
}

fn group(app: &mut App, ui: &mut egui::Ui, id: NodeId, current: GroupOp) {
    let mut op = current;
    // Wrapped, not merely laid out left to right: the four names together are
    // wider than the dock at its default width, and a row that overflows widens
    // the whole column behind it -- which is what used to carry the Z field of
    // Position, Rotation and Scale off the panel with no way to reach it.
    field_row(ui, "Operation", "", |ui| {
        for option in GroupOp::ALL {
            if ui.selectable_label(op == option, option.label()).clicked() && op != option {
                op = option;
                app.edit("Operation", None);
                if let Some(node) = app.scene.get_mut(id) {
                    node.body = Body::Group { op };
                }
            }
        }
    });

    let children = app.scene.node(id).children.clone();
    if op.order_matters() {
        // When a difference group is selected, state plainly which child is the
        // base (spec section 7.3).
        match app.scene.difference_base(id) {
            Some(base) => {
                ui.add(
                    egui::Label::new(theme::hint(format!(
                        "Base: {}. Every visible child below it is cut out of it.",
                        app.scene.node(base).name
                    )))
                    .selectable(false),
                );
            }
            None => {
                ui.colored_label(token::ACCENT, "No visible child, so there is nothing to cut.");
            }
        }
    }

    for (index, child) in children.iter().enumerate() {
        let child = *child;
        ui.horizontal(|ui| {
            let name = app.scene.node(child).name.clone();
            let is_base = op.order_matters() && Some(child) == app.scene.difference_base(id);
            let cut = op.order_matters() && !is_base;
            let mark = if is_base {
                "base"
            } else if cut {
                "cut"
            } else {
                ""
            };
            // The two reorder buttons are placed first, pinned to the right-hand
            // edge, and the name takes whatever is left: laid out the other way
            // round, a long name in a narrow panel pushed the buttons off the
            // edge, out of reach (issue 51).
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(index + 1 < children.len(), egui::Button::new("\u{25BE}").small()).clicked() {
                    app.edit("Reorder", None);
                    app.scene.reorder(child, 1);
                }
                if ui.add_enabled(index > 0, egui::Button::new("\u{25B4}").small()).clicked() {
                    app.edit("Reorder", None);
                    app.scene.reorder(child, -1);
                }
                if !mark.is_empty() {
                    ui.add(
                        egui::Label::new(egui::RichText::new(mark).size(theme::font::SMALL).color(if cut {
                            token::DANGER
                        } else {
                            token::TEXT_LO
                        }))
                        .selectable(false),
                    );
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let text = theme::value(format!("{}. {name}", index + 1));
                    if ui.selectable_label(app.is_selected(child), text).clicked() {
                        app.select_only(child);
                    }
                });
            });
        });
    }
    if children.is_empty() {
        ui.add(egui::Label::new(theme::hint("This group is empty.")).selectable(false));
    }
}

/// What one node currently holds for a parameter.
fn param_value(app: &App, id: NodeId, key: &str, default: ParamValue) -> ParamValue {
    app.scene.node(id).params().and_then(|p| p.get(key).copied()).unwrap_or(default)
}

fn primitive(app: &mut App, ui: &mut egui::Ui, targets: &[NodeId], type_id: &str) {
    let Some(spec) = simple3d_core::primitive::lookup(type_id) else {
        ui.colored_label(token::DANGER, format!("Unknown primitive type \"{type_id}\""));
        return;
    };
    let Some(&id) = targets.last() else { return };
    let unit = app.unit();
    let params = app.scene.node(id).params().cloned().unwrap_or_default();

    for param in spec.params {
        if !spec.param_visible(param, &params) {
            continue;
        }
        param_field(app, ui, targets, id, param, unit, DIMENSION_ROW);
    }

    if spec.segmented {
        field_row(ui, "Segments", "Overrides the scene default for this object's curved surfaces.", |ui| {
            let mut overridden = app.scene.node(id).segments.is_some();
            if ui.checkbox(&mut overridden, "").changed() {
                app.edit("Segment override", None);
                let default = app.scene.settings.default_segments;
                for target in targets {
                    if let Some(node) = app.scene.get_mut(*target) {
                        node.segments = if overridden { Some(default) } else { None };
                    }
                }
            }
            let default = app.scene.settings.default_segments;
            match app.scene.node(id).segments {
                Some(current) => {
                    let mut value = current as f64;
                    if ui.add(egui::DragValue::new(&mut value).range(3.0..=512.0).max_decimals(0)).changed() {
                        app.edit("Segments", Some(&format!("segments:{id}")));
                        for target in targets {
                            if let Some(node) = app.scene.get_mut(*target) {
                                node.segments = Some(value.round() as u32);
                            }
                        }
                    }
                }
                None => {
                    ui.add(egui::Label::new(theme::hint(format!("{default} (scene default)"))).selectable(false));
                }
            }
        });
    }
}

/// Render one parameter's row -- a choice, a checkbox or a number field --
/// writing edits to every selected node. Shared by the primitive editor and the
/// pattern editor (issue 67), which drive it from different parameter lists.
fn param_field(
    app: &mut App,
    ui: &mut egui::Ui,
    targets: &[NodeId],
    id: NodeId,
    param: &simple3d_core::primitive::ParamSpec,
    unit: Unit,
    style: RowStyle,
) {
    let value = param_value(app, id, param.key, param.default);
    match param.kind {
        // Radio-style choices where a measurement is ambiguous.
        // The options flow across the row and wrap when they run out of it,
        // rather than each taking a line of its own. A pattern's kind is six
        // choices and its axis is three, and stacked they pushed everything
        // below them -- the numbers those choices govern -- off the bottom of
        // the panel. Wrapped, a narrow panel still ends up with one per line,
        // which is the layout this replaces, so nothing is lost at any width.
        ParamKind::Choice { options } => {
            field_row(ui, param.label, "", |ui| {
                let mut chosen = value.as_u32();
                for (index, option) in options.iter().enumerate() {
                    if ui.selectable_label(chosen == index as u32, *option).clicked() && chosen != index as u32 {
                        chosen = index as u32;
                        app.edit(style.edit_label, None);
                        for target in targets {
                            set_param(app, *target, param.key, ParamValue::Choice(chosen));
                            sync_wall_mode(app, *target, param.key, chosen);
                        }
                    }
                }
            });
        }
        ParamKind::Bool => {
            field_row(ui, param.label, "", |ui| {
                let mut on = value.as_bool();
                if ui.checkbox(&mut on, "").changed() {
                    app.edit("Set flag", None);
                    for target in targets {
                        set_param(app, *target, param.key, ParamValue::Bool(on));
                    }
                }
            });
        }
        kind => {
            // Worked out before the row is laid out, because where it goes is
            // part of the row's name in one of the two placements.
            let unit_text = match kind {
                ParamKind::Length { .. } => unit.suffix(),
                ParamKind::Angle { .. } => "deg",
                _ => "",
            };
            let in_label = style.unit == UnitPlace::InLabel && !unit_text.is_empty();
            let name = if in_label { format!("{} ({unit_text})", param.label) } else { param.label.to_string() };
            let suffix = if in_label { "" } else { unit_text };
            field_row(ui, &name, "", |ui| {
                let step = ui::scrub_increment(kind, unit);
                // A lock toggle where the type offers one: a sphere's three
                // diameters, a cylinder's two.
                if param.lock_group != 0 {
                    let locked = is_locked(app, id, param.lock_group);
                    let response = crate::icon::button(ui, crate::icon::Glyph::Group, 18.0, locked, true);
                    if response
                        .on_hover_text(if locked {
                            "Locked equal; click to unlock"
                        } else {
                            "Click to lock these equal"
                        })
                        .clicked()
                    {
                        toggle_lock(app, id, param.lock_group, param.key);
                    }
                }
                // Where the unit rides after the field, room is left for it so
                // it never competes with the number it qualifies; where it is in
                // the name, there is nothing to leave room for and the field
                // takes the whole column.
                let field_width = (room_left(ui) - suffix_room(ui, suffix)).max(40.0);
                let field_id = ui.id().with((id, param.key));
                // With several nodes selected, a field shows the value they
                // agree on and an em dash when they do not.
                let shown = ui::shared_text(
                    targets.iter().map(|t| ui::show_param(param_value(app, *t, param.key, param.default), unit)),
                );
                // The field is the grip: dragging it changes the value
                // without going near the keyboard, and clicking it opens it
                // for typing.
                let outcome = ui
                    .scope(|ui| {
                        ui.set_width(field_width);
                        value_field(app, ui, param.label, field_id, &shown, step)
                    })
                    .inner;
                if !suffix.is_empty() {
                    ui.add(egui::Label::new(theme::hint(suffix)).selectable(false));
                }
                if let Some(scrubbed) = outcome.scrubbed {
                    scrub_param(app, targets, param, kind, unit, scrubbed.delta, scrubbed.started);
                }
                if let Some(text) = outcome.committed {
                    set_shared_param(app, targets, param, kind, unit, field_id, text);
                }
            });
        }
    }
}

/// Apply one typed value to every selected node.
///
/// An absolute entry gives them all the same number; a delta (`+2`) is resolved
/// against each node's own value, which is the whole point of having one --
/// "two millimetres wider" means something different for every shape it is
/// applied to.
///
/// A value none of them can read leaves every one of them alone and marks the
/// field. Nothing partial: the selection does not end up half-edited.
fn set_shared_param(
    app: &mut App,
    targets: &[NodeId],
    param: &simple3d_core::primitive::ParamSpec,
    kind: ParamKind,
    unit: Unit,
    field_id: egui::Id,
    text: String,
) {
    // The em dash is what the field shows for a disagreement; leaving it there
    // and tabbing away must not write it to anything.
    if text.trim() == ui::MIXED {
        app.fields.accept(field_id);
        return;
    }
    let mut resolved: Vec<(NodeId, ParamValue)> = Vec::new();
    for target in targets {
        let current = ui::param_number(param_value(app, *target, param.key, param.default));
        match ui::commit_param(&text, kind, unit, current) {
            Commit::Value(value) => resolved.push((*target, value)),
            Commit::Revert => {
                app.fields.reject(field_id, text.clone());
                app.status = Status::Info(format!("\"{text}\" is not a number this field can take"));
                return;
            }
        }
    }
    app.fields.accept(field_id);
    let coalesce = format!("param:{:?}:{}", targets.last(), param.key);
    app.edit(&format!("Set {}", param.label), Some(&coalesce));
    for (target, value) in resolved {
        set_param(app, target, param.key, value);
        apply_lock(app, target, param.lock_group, param.key, value);
    }
}

fn placement(app: &mut App, ui: &mut egui::Ui, targets: &[NodeId]) {
    let unit = app.unit();
    let Some(&primary) = targets.last() else { return };

    // Three columns of numbers, each fronted by its axis colour: the row says
    // which axis is which without spending a character on saying so. The drag
    // is on the fields themselves, here as everywhere else in the panel.
    axis_row(app, ui, &format!("Position ({})", unit.suffix()), |app, ui, axis, name| {
        let field_id = ui.id().with((primary, "pos", axis));
        let shown =
            ui::shared_text(targets.iter().map(|t| format_length(component(app.scene.node(*t).position, axis), unit)));
        let step = unit.from_mm(app.move_snap());
        let outcome = value_field(app, ui, name, field_id, &shown, step);
        if let Some(scrubbed) = outcome.scrubbed {
            scrub_transform(app, targets, axis, unit.to_mm(scrubbed.delta), false, scrubbed.started);
        }
        if let Some(text) = outcome.committed {
            if text.trim() == ui::MIXED {
                app.fields.accept(field_id);
                return;
            }
            let mut resolved: Vec<(NodeId, f64)> = Vec::new();
            for target in targets {
                let current = component(app.scene.node(*target).position, axis);
                match ui::commit_length(&text, unit, current) {
                    Some(value) => resolved.push((*target, value)),
                    None => {
                        app.fields.reject(field_id, text.clone());
                        app.status = Status::Info(format!("\"{text}\" is not a number this field can take"));
                        return;
                    }
                }
            }
            app.fields.accept(field_id);
            app.edit("Set position", Some(&format!("pos:{primary}:{axis}")));
            for (target, value) in resolved {
                if let Some(node) = app.scene.get_mut(target) {
                    let mut p = node.position;
                    set_component(&mut p, axis, value);
                    node.position = p;
                }
            }
        }
    });

    axis_row(app, ui, "Rotation (deg)", |app, ui, axis, name| {
        let field_id = ui.id().with((primary, "rot", axis));
        let shown = ui::shared_text(targets.iter().map(|t| format_angle(component(app.scene.node(*t).rotation, axis))));
        let step = app.settings.rotate_snap_deg.max(1.0);
        let outcome = value_field(app, ui, name, field_id, &shown, step);
        if let Some(scrubbed) = outcome.scrubbed {
            scrub_transform(app, targets, axis, scrubbed.delta, true, scrubbed.started);
        }
        if let Some(text) = outcome.committed {
            if text.trim() == ui::MIXED {
                app.fields.accept(field_id);
                return;
            }
            let mut resolved: Vec<(NodeId, f64)> = Vec::new();
            for target in targets {
                let current = component(app.scene.node(*target).rotation, axis);
                match ui::commit_angle(&text, current) {
                    Some(value) => resolved.push((*target, value)),
                    None => {
                        app.fields.reject(field_id, text.clone());
                        app.status = Status::Info(format!("\"{text}\" is not a number this field can take"));
                        return;
                    }
                }
            }
            app.fields.accept(field_id);
            app.edit("Set rotation", Some(&format!("rot:{primary}:{axis}")));
            for (target, value) in resolved {
                if let Some(node) = app.scene.get_mut(target) {
                    let mut r = node.rotation;
                    set_component(&mut r, axis, value);
                    node.rotation = r;
                }
            }
        }
    });

    // Scale is a factor, not a measurement, so it has no unit and reads in the
    // same three-column row as the two above it. It is the one control that
    // resizes a *group*: a group has no dimensions of its own to type into.
    axis_row(app, ui, "Scale (x)", |app, ui, axis, name| {
        let field_id = ui.id().with((primary, "scale", axis));
        let shown = ui::shared_text(
            targets.iter().map(|t| format_number(component(Node::sane_scale(app.scene.node(*t).scale), axis), 4)),
        );
        let outcome = value_field(app, ui, name, field_id, &shown, 0.05);
        if let Some(scrubbed) = outcome.scrubbed {
            scrub_scale(app, targets, axis, scrubbed.delta, scrubbed.started);
        }
        if let Some(text) = outcome.committed {
            if text.trim() == ui::MIXED {
                app.fields.accept(field_id);
                return;
            }
            let mut resolved: Vec<(NodeId, f64)> = Vec::new();
            for target in targets {
                let current = component(Node::sane_scale(app.scene.node(*target).scale), axis);
                match ui::commit_factor(&text, current) {
                    Some(value) => resolved.push((*target, value)),
                    None => {
                        app.fields.reject(field_id, text.clone());
                        app.status = Status::Info(format!("\"{text}\" is not a number this field can take"));
                        return;
                    }
                }
            }
            app.fields.accept(field_id);
            app.edit("Set scale", Some(&format!("scale:{primary}:{axis}")));
            for (target, value) in resolved {
                if let Some(node) = app.scene.get_mut(target) {
                    let mut s = Node::sane_scale(node.scale);
                    set_component(&mut s, axis, value);
                    node.scale = s;
                }
            }
        }
    });
    if targets.iter().any(|t| app.scene.node(*t).scale != Vec3::ONE) {
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(theme::hint("Scale is a factor on top of the dimensions.")).selectable(false));
            if ui.small_button("Reset to 1").clicked() {
                app.edit("Reset scale", None);
                for target in targets {
                    if let Some(node) = app.scene.get_mut(*target) {
                        node.scale = Vec3::ONE;
                    }
                }
            }
        });
    }

    step_row(app, ui);
    ui.add(egui::Label::new(theme::hint("Rotations are applied X, then Y, then Z.")).selectable(false));
    if targets.len() > 1 {
        ui.add(
            egui::Label::new(theme::hint(
                "A value applies to all of them; a delta (\u{201C}+2\u{201D}, \u{201C}- 5\u{201D}) applies to each \
                 from where it already is.",
            ))
            .selectable(false),
        );
    }
}

/// How far one step of a move or a resize goes, in the display unit.
///
/// It sits here, under the fields it governs, rather than only in the document
/// settings: the step is something you change *while* nudging something into
/// place, and going looking for it in another panel is the wrong five seconds.
/// The same value is on the Document panel, so it is also reachable with nothing
/// selected.
pub fn step_row(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    field_row(ui, "Step", "", |ui| {
        let mut step = unit.from_mm(app.scene.settings.snap_step);
        let response = ui
            .add(egui::DragValue::new(&mut step).range(1e-6..=1e6).speed(0.05))
            .on_hover_text("How far one nudge, and one snapped step of a move or resize drag, goes.");
        if response.changed() {
            app.edit("Step", Some("scene:step"));
            app.scene.settings.snap_step = unit.to_mm(step).max(1e-6);
        }
        ui.add(egui::Label::new(theme::hint(unit.suffix())).selectable(false));
    });
}

/// One labelled row of three axis fields, each preceded by its colour chip. The
/// chip is handed to the caller as that field's scrub grip.
fn axis_row(
    app: &mut App,
    ui: &mut egui::Ui,
    label: &str,
    mut field: impl FnMut(&mut App, &mut egui::Ui, usize, &str),
) {
    field_row(ui, label, "", |ui| {
        // Three fields, three chips, and the gaps between them all have to come
        // out of the row: getting this wrong pushes the Z field off the panel.
        // The panel's own edge decides how much there is to share out, never the
        // widest row above -- one row wide enough to overflow would otherwise
        // take Z with it.
        let available = room_left(ui);
        let chips = 3.0 * (theme::AXIS_CHIP_WIDTH + ui.spacing().item_spacing.x);
        let gaps = 2.0 * ui.spacing().item_spacing.x;
        let each = (available - chips - gaps) / 3.0;
        // Below a width where a number is still readable, the three axes go one
        // to a line at full width rather than three unusable slivers (issue 51).
        // Each keeps its colour chip, which is what says which axis it is.
        if each < MIN_AXIS_FIELD {
            ui.vertical(|ui| {
                for axis in 0..3 {
                    ui.horizontal(|ui| {
                        theme::axis_chip(ui, ui.id().with((label, axis)), axis);
                        let name = format!("{label}:{axis}");
                        field(app, ui, axis, &name);
                    });
                }
            });
            return;
        }
        for axis in 0..3 {
            // The chip is a label, not a handle: it says which axis this column
            // is, and the field beside it carries the drag.
            theme::axis_chip(ui, ui.id().with((label, axis)), axis);
            let name = format!("{label}:{axis}");
            ui.scope(|ui| {
                ui.set_width(each);
                field(app, ui, axis, &name);
            });
        }
    });
}

/// One frame of a scrub on a dimension.
///
/// `started` is the only frame that takes an undo snapshot. Every frame after
/// it goes through `touch`, which re-evaluates without recording -- so a drag
/// across forty pixels is one step to undo, not forty.
fn scrub_param(
    app: &mut App,
    targets: &[NodeId],
    param: &simple3d_core::primitive::ParamSpec,
    kind: ParamKind,
    unit: Unit,
    delta: f64,
    started: bool,
) {
    if started {
        app.edit(&format!("Scrub {}", param.label), None);
    }
    for target in targets {
        let current = ui::param_number(param_value(app, *target, param.key, param.default));
        let shown = match kind {
            ParamKind::Length { .. } => unit.from_mm(current),
            _ => current,
        };
        let next = ui::value_from_display(kind, unit, shown + delta);
        set_param(app, *target, param.key, next);
        apply_lock(app, *target, param.lock_group, param.key, next);
    }
    app.touch();
    app.fields.clear();
}

/// One frame of a scrub on a position (millimetres) or a rotation (degrees).
fn scrub_transform(app: &mut App, targets: &[NodeId], axis: usize, delta: f64, rotation: bool, started: bool) {
    if started {
        app.edit(if rotation { "Scrub rotation" } else { "Scrub position" }, None);
    }
    for target in targets {
        let Some(node) = app.scene.get_mut(*target) else { continue };
        let mut v = if rotation { node.rotation } else { node.position };
        let next = crate::gizmo::get_axis(v, axis) + delta;
        set_component(&mut v, axis, next);
        if rotation {
            node.rotation = v;
        } else {
            node.position = v;
        }
    }
    app.touch();
    app.fields.clear();
}

/// One frame of a scrub on a scale field. The grip steps by 0.05 -- a twentieth
/// is a visible change on any shape, where a millimetre-sized step would be
/// nothing on a factor.
fn scrub_scale(app: &mut App, targets: &[NodeId], axis: usize, delta: f64, started: bool) {
    if started {
        app.edit("Scrub scale", None);
    }
    for target in targets {
        let Some(node) = app.scene.get_mut(*target) else { continue };
        let mut s = Node::sane_scale(node.scale);
        let next = (crate::gizmo::get_axis(s, axis) + delta).max(Node::MIN_SCALE);
        set_component(&mut s, axis, next);
        node.scale = s;
    }
    app.touch();
    app.fields.clear();
}

fn measurements(app: &mut App, ui: &mut egui::Ui, id: NodeId, selected: usize) {
    if selected > 1 {
        ui.add(
            egui::Label::new(theme::hint(format!("Measured from {}, the last one selected.", app.scene.node(id).name)))
                .selectable(false),
        );
    }
    let unit = app.unit();
    match app.evaluated.node_world_bounds.get(&id).copied() {
        Some((lo, hi)) => {
            field_row(ui, "Size", "", |ui| {
                ui.add(egui::Label::new(theme::numeric(ui::describe_size(hi - lo, unit))).selectable(false).wrap());
            });
            field_row(ui, "Centre", "", |ui| {
                ui.add(
                    egui::Label::new(theme::numeric(format!(
                        "{}, {}, {} {}",
                        format_length((lo.x + hi.x) / 2.0, unit),
                        format_length((lo.y + hi.y) / 2.0, unit),
                        format_length((lo.z + hi.z) / 2.0, unit),
                        unit.suffix()
                    )))
                    .selectable(false)
                    .wrap(),
                );
            });
        }
        None => {
            ui.add(egui::Label::new(theme::hint("No geometry yet.")).selectable(false));
        }
    }
    if let Some(error) = app.evaluated.error_for(id) {
        ui.colored_label(token::DANGER, &error.message);
    }
}

fn component(v: Vec3, axis: usize) -> f64 {
    crate::gizmo::get_axis(v, axis)
}

fn set_component(v: &mut Vec3, axis: usize, value: f64) {
    crate::gizmo::set_axis(v, axis, value);
}

fn set_param(app: &mut App, id: NodeId, key: &str, value: ParamValue) {
    if let Some(params) = app.scene.get_mut(id).and_then(|n| n.params_mut()) {
        params.insert(key.to_string(), value);
    }
}

/// Whether a lock group's parameters currently hold the same value. The lock is
/// not stored in the model -- it is a property of the numbers themselves, so a
/// project file has no hidden state that could disagree with what it shows.
fn is_locked(app: &App, id: NodeId, group: u8) -> bool {
    let Some(spec) = app.scene.node(id).spec() else { return false };
    let Some(params) = app.scene.node(id).params() else { return false };
    let keys: Vec<&str> = spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key).collect();
    match keys.split_first() {
        Some((first, rest)) => {
            let reference = params.num(first);
            rest.iter().all(|k| (params.num(k) - reference).abs() < 1e-9)
        }
        None => false,
    }
}

/// Clicking the lock either equalises the group to the clicked field's value or,
/// if it is already locked, nudges one member so it visibly unlocks.
fn toggle_lock(app: &mut App, id: NodeId, group: u8, key: &str) {
    let Some(spec) = app.scene.node(id).spec() else { return };
    let keys: Vec<String> = spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key.to_string()).collect();
    if is_locked(app, id, group) {
        // Nothing to change: the fields are already independent as far as the
        // model is concerned. Just tell the user.
        app.status = Status::Info("Unlocked; edit the diameters independently".into());
        return;
    }
    let Some(value) = app.scene.node(id).params().map(|p| p.num(key)) else { return };
    app.edit("Lock dimensions", None);
    for other in keys {
        set_param(app, id, &other, ParamValue::Length(value));
    }
}

/// When a locked group's member changes, bring the others with it.
fn apply_lock(app: &mut App, id: NodeId, group: u8, key: &str, value: ParamValue) {
    if group == 0 {
        return;
    }
    // Read the lock state from before this edit: the field just changed, so the
    // group is no longer equal, and asking now would always say "unlocked".
    let Some(spec) = app.scene.node(id).spec() else { return };
    let keys: Vec<String> = spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key.to_string()).collect();
    let others: Vec<&String> = keys.iter().filter(|k| k.as_str() != key).collect();
    let Some(params) = app.scene.node(id).params() else { return };
    // Locked before the edit means every *other* member still agrees with every
    // other member.
    let was_locked = match others.split_first() {
        Some((first, rest)) => {
            let reference = params.num(first);
            rest.iter().all(|k| (params.num(k) - reference).abs() < 1e-9)
        }
        None => false,
    };
    if !was_locked {
        return;
    }
    let others: Vec<String> = others.into_iter().cloned().collect();
    for other in others {
        set_param(app, id, &other, value);
    }
    app.fields.clear();
}

/// Switching a tube between "wall thickness" and "inner diameter" carries the
/// current geometry across, so the shape does not jump when the user only meant
/// to change how they express it.
fn sync_wall_mode(app: &mut App, id: NodeId, key: &str, chosen: u32) {
    if key != "wall_mode" {
        return;
    }
    let Some(params) = app.scene.node(id).params().cloned() else { return };
    let outer = params.num("outer_diameter");
    if chosen == 1 {
        let inner = (outer - 2.0 * params.num("wall_thickness")).clamp(0.0, outer);
        set_param(app, id, "inner_diameter", ParamValue::Length(inner));
    } else {
        let wall = ((outer - params.num("inner_diameter")) / 2.0).max(0.0);
        set_param(app, id, "wall_thickness", ParamValue::Length(wall));
    }
    app.fields.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use simple3d_core::primitive;
    use simple3d_core::scene::Scene;

    /// An `App` on a headless context, pointed at a throwaway config directory
    /// so a test cannot read or write the developer's own.
    fn headless_app() -> App {
        let dir = std::env::temp_dir().join(format!(
            "simple3d-props-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        App::with_config_dir(&egui::Context::default(), None, dir)
    }

    /// Two plates of different widths, selected together.
    fn two_plates(app: &mut App) -> (NodeId, NodeId) {
        let root = app.scene.root();
        let a = app.scene.add_primitive("plate", root, 0).unwrap();
        let b = app.scene.add_primitive("plate", root, 1).unwrap();
        set_param(app, a, "width", ParamValue::Length(40.0));
        set_param(app, b, "width", ParamValue::Length(60.0));
        app.selection = vec![a, b];
        (a, b)
    }

    fn width_of(app: &App, id: NodeId) -> f64 {
        app.scene.node(id).params().unwrap().num("width")
    }

    fn width_spec() -> &'static primitive::ParamSpec {
        primitive::lookup("plate").unwrap().params.iter().find(|p| p.key == "width").unwrap()
    }

    #[test]
    fn a_field_over_a_multi_selection_shows_the_shared_value_or_an_em_dash() {
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        let unit = app.unit();
        let shown = |app: &App, ids: &[NodeId]| {
            ui::shared_text(
                ids.iter().map(|t| ui::show_param(param_value(app, *t, "width", ParamValue::Length(0.0)), unit)),
            )
        };
        assert_eq!(shown(&app, &[a, b]), ui::MIXED, "two different widths must not claim to be one");
        assert_eq!(shown(&app, &[a]), "40");
        set_param(&mut app, b, "width", ParamValue::Length(40.0));
        assert_eq!(shown(&app, &[a, b]), "40", "two equal widths are one value, not a dash");
    }

    #[test]
    fn an_absolute_value_applies_to_the_whole_selection_and_a_delta_applies_per_node() {
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        let unit = app.unit();
        let param = width_spec();
        let field = egui::Id::new("width-field");

        set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "25".into());
        assert_eq!((width_of(&app, a), width_of(&app, b)), (25.0, 25.0), "an absolute value is one value for all");

        set_param(&mut app, a, "width", ParamValue::Length(40.0));
        set_param(&mut app, b, "width", ParamValue::Length(60.0));
        set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "+2".into());
        assert_eq!(
            (width_of(&app, a), width_of(&app, b)),
            (42.0, 62.0),
            "a delta is relative to each node's own value"
        );

        // And the field's other tricks reach the model the same way.
        set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "4cm".into());
        assert_eq!((width_of(&app, a), width_of(&app, b)), (40.0, 40.0), "a value in another unit did not convert");
        set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "12+8".into());
        assert_eq!((width_of(&app, a), width_of(&app, b)), (20.0, 20.0), "an expression was not evaluated");
    }

    #[test]
    fn an_em_dash_left_alone_edits_nothing() {
        // Tabbing through a panel of mixed values must not flatten them.
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        let unit = app.unit();
        let param = width_spec();
        set_shared_param(&mut app, &[a, b], param, param.kind, unit, egui::Id::new("f"), ui::MIXED.into());
        assert_eq!((width_of(&app, a), width_of(&app, b)), (40.0, 60.0));
    }

    #[test]
    fn a_value_that_cannot_be_read_leaves_every_node_alone_and_marks_the_field() {
        // Acceptance criterion 14, and the design's rule that the typed text
        // stays put: it is the thing the user has to correct.
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        let unit = app.unit();
        let param = width_spec();
        let field = egui::Id::new("width-field");
        let before = app.history.revision();

        set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "wide-ish".into());
        assert_eq!((width_of(&app, a), width_of(&app, b)), (40.0, 60.0), "a rejected entry changed the model");
        assert_eq!(app.history.revision(), before, "a rejected entry took an undo step");
        assert!(app.fields.is_rejected(field), "the field was not marked");

        // Correcting it clears the mark.
        set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "30".into());
        assert!(!app.fields.is_rejected(field));
        assert_eq!((width_of(&app, a), width_of(&app, b)), (30.0, 30.0));
    }

    #[test]
    fn a_whole_scrub_is_one_undo_step() {
        // Forty snapshots for one drag would make undo useless exactly where it
        // is needed most.
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        let unit = app.unit();
        let param = width_spec();
        let steps = app.history.undo_len();

        for frame in 0..40 {
            scrub_param(&mut app, &[a, b], param, param.kind, unit, 0.5, frame == 0);
        }
        assert_eq!(width_of(&app, a), 60.0, "the scrub did not accumulate");
        assert_eq!(width_of(&app, b), 80.0, "each node scrubs from its own value");
        assert_eq!(app.history.undo_len(), steps + 1, "the drag left more than one step to undo");

        app.history.undo(&mut app.scene);
        assert_eq!((width_of(&app, a), width_of(&app, b)), (40.0, 60.0), "one undo did not put the drag back");
    }

    #[test]
    fn a_position_scrub_moves_every_selected_node_by_the_same_amount() {
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        app.scene.get_mut(b).unwrap().position = Vec3::new(10.0, 0.0, 0.0);
        for frame in 0..10 {
            scrub_transform(&mut app, &[a, b], 0, 1.0, false, frame == 0);
        }
        assert_eq!(app.scene.node(a).position.x, 10.0);
        assert_eq!(app.scene.node(b).position.x, 20.0);
        app.history.undo(&mut app.scene);
        assert_eq!(app.scene.node(a).position.x, 0.0);
        assert_eq!(app.scene.node(b).position.x, 10.0);
    }

    #[test]
    fn a_selection_of_one_kind_of_shape_gets_a_dimensions_panel_and_a_mixed_one_does_not() {
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        assert_eq!(shared_type(&app, &[a, b]).as_deref(), Some("plate"));
        let root = app.scene.root();
        let sphere = app.scene.add_primitive("sphere", root, 2).unwrap();
        assert_eq!(shared_type(&app, &[a, sphere]), None, "a plate and a sphere share no dimension");
        let group = app.scene.add_group(GroupOp::Union, root, 3);
        assert_eq!(shared_type(&app, &[group]), None, "a group has no dimensions of its own");
    }

    #[test]
    fn every_registry_parameter_maps_to_a_field_kind_the_editor_draws() {
        // The editor has a branch per `ParamKind`; a kind it did not handle would
        // silently render nothing, so check every declared parameter falls into
        // one of them.
        for spec in primitive::REGISTRY {
            for param in spec.params {
                let handled = matches!(
                    param.kind,
                    ParamKind::Length { .. }
                        | ParamKind::Angle { .. }
                        | ParamKind::Count { .. }
                        | ParamKind::Bool
                        | ParamKind::Choice { .. }
                );
                assert!(handled, "{}.{} has an unhandled kind", spec.type_id, param.key);
            }
        }
    }

    #[test]
    fn a_lock_group_reads_as_locked_when_its_members_agree() {
        let mut scene = Scene::new();
        let root = scene.root();
        let id = scene.add_primitive("sphere", root, 0).unwrap();
        let mut app_scene = scene.clone();
        // Defaults are all 20, so the group starts locked.
        assert!(locked_in(&app_scene, id, 1));
        app_scene.get_mut(id).unwrap().params_mut().unwrap().insert("diameter_y".into(), ParamValue::Length(30.0));
        assert!(!locked_in(&app_scene, id, 1));
        let _ = scene;
    }

    /// The same rule as `is_locked`, against a bare scene so it can be tested
    /// without an `App`.
    fn locked_in(scene: &Scene, id: NodeId, group: u8) -> bool {
        let spec = scene.node(id).spec().unwrap();
        let params = scene.node(id).params().unwrap();
        let keys: Vec<&str> = spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key).collect();
        let (first, rest) = keys.split_first().unwrap();
        let reference = params.num(first);
        rest.iter().all(|k| (params.num(k) - reference).abs() < 1e-9)
    }

    #[test]
    fn only_the_relevant_wall_parameter_is_shown() {
        let spec = primitive::lookup("tube").unwrap();
        let mut params = spec.default_params();
        params.insert("wall_mode".into(), ParamValue::Choice(0));
        let visible: Vec<&str> = spec.params.iter().filter(|p| spec.param_visible(p, &params)).map(|p| p.key).collect();
        assert!(visible.contains(&"wall_thickness"));
        assert!(!visible.contains(&"inner_diameter"));

        params.insert("wall_mode".into(), ParamValue::Choice(1));
        let visible: Vec<&str> = spec.params.iter().filter(|p| spec.param_visible(p, &params)).map(|p| p.key).collect();
        assert!(visible.contains(&"inner_diameter"));
        assert!(!visible.contains(&"wall_thickness"));
    }
}
