//! The tool that cuts a shape into a pattern of smaller pieces (issue 82).
//!
//! "Break into separate objects" finds the pieces a shape is *already* in. This
//! cuts one that is in a single piece: into squares, rectangles, triangles or
//! hexagons, running through the shape along an axis and optionally cut into
//! layers across it as well. Both end in the same place -- a
//! [`Body::Split`](simple3d_core::scene::Body::Split) standing where the shape
//! stood, holding the pieces and the shape itself -- so both are undone by the
//! same Join back together, however long afterwards.
//!
//! The window is where the pattern is chosen, and it draws the cells over the
//! shape's own outline while they are being chosen: the one question a number
//! of millimetres cannot answer on its own is what it looks like against the
//! thing being cut. Nothing is cut until Split is pressed, and the cutting
//! itself happens on a thread -- see [`crate::worker::SplitJob`] -- because a
//! hexagon tiling over a plate is hundreds of booleans and an interface that
//! stops answering is one nobody can tell from a crashed one.

use crate::app::{App, Modal, Status};
use crate::worker::SplitJob;
use crate::{theme, ui};
use simple3d_core::scene::NodeId;
use simple3d_core::unit::Unit;
use simple3d_geom::tiling::{self, CellKind, Tiling};
use simple3d_geom::{Mesh, Vec3};
use std::sync::Arc;

/// What the tool is working on while its window is open.
pub struct SplitTool {
    pub target: NodeId,
    /// The shape as it stands, baked in its own frame: what the cells are cut
    /// out of, what the estimate is counted over, and what the picture is drawn
    /// from. Baked once, when the tool opens -- the window is modal, so nothing
    /// can change underneath it.
    pub mesh: Arc<Mesh>,
    pub bounds: (Vec3, Vec3),
    pub tiling: Tiling,
    /// The numbers as typed, one string per field. A field is only read back
    /// into the tiling when what is in it parses, so a half-typed number is
    /// left alone rather than snapping to something while it is being written.
    pub text: Fields,
}

pub struct Fields {
    pub size: String,
    pub depth: String,
    pub angle: String,
    pub layer: String,
    pub offset: [String; 2],
}

impl Fields {
    fn from(tiling: &Tiling, unit: Unit) -> Fields {
        let length = |mm: f64| simple3d_core::unit::format_length(mm, unit);
        Fields {
            size: length(tiling.size),
            depth: length(tiling.depth),
            angle: simple3d_core::unit::format_angle(tiling.angle),
            layer: length(tiling.layer),
            offset: [length(tiling.offset[0]), length(tiling.offset[1])],
        }
    }
}

impl App {
    /// Open the tool on the selection (issue 82).
    ///
    /// It opens on the numbers the last split used rather than on a default, so
    /// cutting a second shape the same way is one gesture; what a split was cut
    /// with is also stored on the split itself, so opening the tool on one
    /// offers *its* pattern and cutting again changes the pattern rather than
    /// splitting a split.
    pub fn open_split_tool(&mut self) {
        let targets = self.top_level_selection();
        let Some(&id) = targets.first() else {
            self.status = Status::Warning("Select something to split into pieces".into());
            return;
        };
        if targets.len() > 1 {
            self.status = Status::Warning("Split one object at a time".into());
            return;
        }
        if self.split_job.is_some() {
            self.status = Status::Warning("A split is already running".into());
            return;
        }
        let mesh = simple3d_core::eval::baked_mesh(&self.scene, id);
        let Some(bounds) = mesh.bounds() else {
            self.status = Status::Warning("There is no geometry there to split".into());
            return;
        };
        let tiling = self.scene.node(id).split_tiling().unwrap_or(self.settings.last_split);
        self.split_tool = Some(SplitTool {
            target: id,
            mesh: Arc::new(mesh),
            bounds,
            tiling,
            text: Fields::from(&tiling, self.unit()),
        });
        self.modal = Modal::SplitTool;
    }

    /// Start cutting, and put the window away. The document is not touched until
    /// the pieces arrive.
    pub fn start_split(&mut self) {
        let Some(tool) = self.split_tool.take() else { return };
        self.modal = Modal::None;
        if tool.tiling.refusal(tool.bounds).is_some() || !self.scene.contains(tool.target) {
            return;
        }
        // The shape as it is now, to be compared against the shape as it is when
        // the pieces land: a split applied to something that was edited while it
        // was being cut would be pieces of a shape that no longer exists.
        let Some(before) = self.scene.export_subtree(tool.target) else { return };
        let name = self.scene.node(tool.target).name.clone();
        self.settings.last_split = tool.tiling;
        self.persist();
        let job = SplitJob::spawn(tool.target, self.active, before, tool.mesh, tool.tiling);
        self.status = Status::Info(format!("Splitting {name} into {} cells\u{2026}", job.cells));
        self.split_job = Some(job);
    }

    /// Take the pieces once they are cut, and stand a split where the shape was.
    ///
    /// Everything that could have changed while the cutting ran is checked here
    /// rather than assumed: the document may have been switched, the shape may
    /// have been deleted or edited, and none of those is a reason to change
    /// anything -- the split is dropped and says so.
    pub fn poll_split(&mut self) {
        let Some(job) = &self.split_job else { return };
        let Some(outcome) = job.poll() else { return };
        let job = self.split_job.take().expect("it was there a line ago");
        let Some(pieces) = outcome else {
            self.status = Status::Info("Split stopped; nothing was changed".into());
            return;
        };
        if job.tab != self.active || !self.scene.contains(job.node) {
            self.status = Status::Warning("The split was dropped: the object it was cutting is no longer there".into());
            return;
        }
        if self.scene.export_subtree(job.node).as_ref() != Some(&job.before) {
            self.status = Status::Warning("The split was dropped: the object changed while it was being cut".into());
            return;
        }
        let kind = job.tiling.kind;
        if pieces.len() < 2 {
            self.status = Status::Warning(format!(
                "{} that size leave the shape in one piece -- try a smaller cell",
                kind.label()
            ));
            return;
        }
        self.edit("Split into smaller pieces", None);
        let Some((name, count)) = self.hold_pieces(job.node, pieces, Some(job.tiling)) else {
            self.history.discard_last();
            return;
        };
        self.status =
            Status::Info(format!("Split {name} into {count} {} -- {}", plural_cells(kind, count), self.way_back()));
    }

    pub fn cancel_split_tool(&mut self) {
        self.split_tool = None;
        self.modal = Modal::None;
    }
}

/// What to call a number of pieces of a given cell shape.
fn plural_cells(kind: CellKind, count: usize) -> String {
    if count == 1 {
        kind.singular().to_string()
    } else {
        kind.label().to_lowercase()
    }
}

/// The tool's contents: the pattern down the left, a picture of where the cuts
/// will fall on the right, and what it comes to underneath both.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    if app.split_tool.is_none() {
        ui.label("The object this was opened on is no longer there.");
        return;
    }
    // Two columns measured out here rather than left to a side panel: both
    // widths are decided by one rule -- the numbers first, the picture with what
    // is left -- and a panel would put half that rule in egui's hands.
    let room = ui.available_width();
    let gap = ui.spacing().item_spacing.x;
    let wide = room >= COLUMN_MIN + PICTURE_MIN + gap;
    // Below the width both need, the numbers keep the room: they are what the
    // window is open for, and the picture is what gives way. Above it the
    // numbers keep the width they need and every extra pixel goes to the
    // picture, which is the half worth more the bigger it is.
    let column = if wide { (room - PICTURE_MIN - gap).min(COLUMN_MAX) } else { room };
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(column, ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                controls(app, ui);
                ui.add_space(6.0);
                summary(app, ui);
            },
        );
        if wide {
            picture(app, ui);
        }
    });
}

pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    let ready = app
        .split_tool
        .as_ref()
        .is_some_and(|tool| tool.tiling.refusal(tool.bounds).is_none() && app.scene.contains(tool.target));
    if ui::dialog_button(ui, "Split", ready).clicked() {
        app.start_split();
    }
    if ui::dialog_button(ui, "Cancel", true).clicked() {
        app.cancel_split_tool();
    }
}

/// The narrowest the plan of the cells is worth drawing at.
const PICTURE_MIN: f32 = 220.0;
/// The narrowest the fields are worth laying out at, and the widest they are
/// worth stretching to: a row is a name and a number, and past this the two are
/// pushed apart with nothing between them.
const COLUMN_MIN: f32 = 300.0;
const COLUMN_MAX: f32 = 380.0;

fn controls(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    let Some(tool) = app.split_tool.as_mut() else { return };
    egui::Grid::new("split-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.label("Cells");
        ui.horizontal_wrapped(|ui| {
            for kind in CellKind::ALL {
                if theme::choice(ui, tool.tiling.kind == kind, kind.label())
                    .on_hover_text(kind.size_meaning())
                    .clicked()
                {
                    tool.tiling.kind = kind;
                }
            }
        });
        ui.end_row();

        ui.label("Size");
        length_field(
            ui,
            "split-size",
            &mut tool.text.size,
            &mut tool.tiling.size,
            unit,
            tool.tiling.kind.size_meaning(),
        );
        ui.end_row();

        if tool.tiling.kind.has_depth() {
            ui.label("Depth");
            length_field(
                ui,
                "split-depth",
                &mut tool.text.depth,
                &mut tool.tiling.depth,
                unit,
                "The second side of one rectangle.",
            );
            ui.end_row();
        }

        // The axis is the direction the cells *run in*, not the plane they lie
        // in, which is the way round a cut is thought about: a plate lying flat
        // is cut into columns standing up it, which is Z.
        ui.label("Through");
        ui.horizontal(|ui| {
            for (axis, name) in [(0u8, "X"), (1, "Y"), (2, "Z")] {
                if theme::choice(ui, tool.tiling.axis == axis, name).clicked() {
                    tool.tiling.axis = axis;
                }
            }
        })
        .response
        .on_hover_text("The axis the cells run along. The tiling lies in the plane across it.");
        ui.end_row();

        ui.label("Turn");
        angle_field(ui, "split-angle", &mut tool.text.angle, &mut tool.tiling.angle);
        ui.end_row();

        ui.label("Offset");
        ui.horizontal(|ui| {
            for i in 0..2 {
                let mut value = tool.tiling.offset[i];
                length_field(
                    ui,
                    &format!("split-offset-{i}"),
                    &mut tool.text.offset[i],
                    &mut value,
                    unit,
                    "Move the grid within its plane. The cells are centred on the shape until this says otherwise.",
                );
                tool.tiling.offset[i] = value;
            }
        });
        ui.end_row();

        // "Layer height" rather than "Layers": the number is how tall one layer
        // is, and a row called Layers holding a 4 reads as four of them.
        ui.label("Layer height");
        length_field(
            ui,
            "split-layer",
            &mut tool.text.layer,
            &mut tool.tiling.layer,
            unit,
            "Cut across the cells as well, into layers this tall. Zero cuts straight through.",
        );
        ui.end_row();
    });
}

/// One length field: a text box that is read back only when what is in it
/// parses, so a number half typed is left alone.
fn length_field(ui: &mut egui::Ui, id: &str, text: &mut String, value: &mut f64, unit: Unit, hover: &str) {
    let field = ui.add(egui::TextEdit::singleline(text).id_salt(id).desired_width(90.0)).on_hover_text(hover);
    if field.changed() {
        if let Some(mm) = ui::commit_length(text, unit, *value) {
            *value = mm.max(0.0);
        }
    }
    // Back into the field's own terms once the user has left it, so "1/2" or a
    // number with a unit on it settles into what it meant.
    if field.lost_focus() {
        *text = simple3d_core::unit::format_length(*value, unit);
    }
}

fn angle_field(ui: &mut egui::Ui, id: &str, text: &mut String, value: &mut f64) {
    let field = ui
        .add(egui::TextEdit::singleline(text).id_salt(id).desired_width(90.0))
        .on_hover_text("Turn the whole grid within its plane, in degrees.");
    if field.changed() {
        if let Some(deg) = ui::commit_angle(text, *value) {
            *value = deg;
        }
    }
    if field.lost_focus() {
        *text = simple3d_core::unit::format_angle(*value);
    }
}

/// What the pattern comes to: how many cells it lays over the shape, or why it
/// cannot be cut at all.
fn summary(app: &mut App, ui: &mut egui::Ui) {
    let Some(tool) = app.split_tool.as_ref() else { return };
    match tool.tiling.refusal(tool.bounds) {
        Some(why) => {
            ui.add(egui::Label::new(egui::RichText::new(why).size(theme::font::LABEL).color(theme::token::ACCENT)));
        }
        None => {
            let cells = tiling::planned(&tool.tiling, tool.bounds);
            let size = tool.bounds.1 - tool.bounds.0;
            ui.add(
                egui::Label::new(theme::hint(format!(
                    "Up to {cells} cells over {}. A cell the shape does not reach makes no piece, so there will \
                     usually be fewer pieces than cells.",
                    ui::describe_size(size, app.unit())
                )))
                .selectable(false),
            );
        }
    }
}

/// The shape seen down the axis the cells run along, with the cells drawn over
/// it: where the cuts will fall, which is the one thing a number of millimetres
/// cannot say on its own.
fn picture(app: &mut App, ui: &mut egui::Ui) {
    let Some(tool) = app.split_tool.as_ref() else { return };
    // Space rather than a widget: the plan is painted, not interacted with, and
    // a widget laid over the room would be a widget the numbers beside it are
    // under -- which is a click on a cell shape that goes nowhere.
    let (_, rect) = ui.allocate_space(ui.available_size());
    // Clipped to its own rectangle: a cell of the plan can reach past the shape,
    // and what reaches past the picture belongs to the fields beside it.
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, theme::token::SURFACE_1);

    let outlines = tiling::cell_outlines(&tool.tiling, tool.bounds);
    let (lo, hi) = (tool.tiling.flatten(tool.bounds.0), tool.tiling.flatten(tool.bounds.1));
    // Fitted to the shape *and* the cells over it, not to the shape alone: a
    // cell at the edge reaches past the shape it is cutting, and fitting the
    // shape drew the outermost cells off the side of the picture.
    let (mut plan_lo, mut plan_hi) = (lo, hi);
    for point in outlines.iter().flatten() {
        plan_lo = (plan_lo.0.min(point.0), plan_lo.1.min(point.1));
        plan_hi = (plan_hi.0.max(point.0), plan_hi.1.max(point.1));
    }
    let span = ((plan_hi.0 - plan_lo.0).max(1e-6), (plan_hi.1 - plan_lo.1).max(1e-6));
    let pad = 10.0;
    let scale = ((rect.width() - pad * 2.0) as f64 / span.0).min((rect.height() - pad * 2.0) as f64 / span.1);
    let centre = ((plan_lo.0 + plan_hi.0) / 2.0, (plan_lo.1 + plan_hi.1) / 2.0);
    // Y up, the way the plane's second axis runs, rather than down the screen.
    let at = |(x, y): (f64, f64)| {
        egui::pos2(rect.center().x + ((x - centre.0) * scale) as f32, rect.center().y - ((y - centre.1) * scale) as f32)
    };

    // The shape itself, seen down the axis: its own triangles flattened, which
    // is the true outline rather than the box around it. A mesh too big to draw
    // every frame gets the box instead -- the picture is about the cells.
    let mesh = &tool.mesh;
    if mesh.triangle_count() <= SILHOUETTE_LIMIT {
        let mut shape = egui::epaint::Mesh::default();
        for tri in &mesh.indices {
            let base = shape.vertices.len() as u32;
            for &index in tri {
                let flat = tool.tiling.flatten(mesh.positions[index as usize]);
                shape.vertices.push(egui::epaint::Vertex {
                    pos: at(flat),
                    uv: egui::epaint::WHITE_UV,
                    color: theme::token::SURFACE_3,
                });
            }
            shape.indices.extend([base, base + 1, base + 2]);
        }
        painter.add(egui::Shape::mesh(shape));
    } else {
        painter.rect_filled(egui::Rect::from_two_pos(at(lo), at(hi)), 0.0, theme::token::SURFACE_3);
    }

    // And the cells over it.
    let stroke = egui::Stroke::new(1.0_f32, theme::token::ACCENT);
    for outline in outlines {
        let points: Vec<egui::Pos2> = outline.iter().map(|&p| at(p)).collect();
        if points.iter().all(|p| !rect.expand(4.0).contains(*p)) {
            continue;
        }
        painter.add(egui::Shape::closed_line(points, stroke));
    }
}

/// The most triangles the picture will flatten and draw every frame. Past this
/// the shape is drawn as its bounding box: the cells are what the picture is
/// about, and a preview is not worth a frame rate.
const SILHOUETTE_LIMIT: usize = 20_000;
