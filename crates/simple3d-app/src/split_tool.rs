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
//! The window is where the pattern is chosen, and the cells are drawn **over
//! the model itself** while they are being chosen: the one question a number of
//! millimetres cannot answer on its own is what it looks like against the thing
//! being cut, and the honest answer to that is the thing being cut. Nothing is
//! cut until Split is pressed, and the cutting itself happens on a thread --
//! see [`crate::worker::SplitJob`] -- because a hexagon tiling over a plate is
//! hundreds of booleans and an interface that stops answering is one nobody can
//! tell from a crashed one.
//!
//! It is an [in-place popup](crate::popup) rather than a dialog: it floats over
//! the viewport, is dragged around by its own title bar and rolls up out of the
//! way, and it does not stop the model underneath it being orbited, zoomed or
//! selected. That is what lets the viewport be the modelling area and the
//! preview area at once -- the cells are drawn in it, in world space, and the
//! way to see whether they fall where they should is to orbit the model with
//! the numbers still on screen. What the viewport does *under* the cells while
//! that is happening is the document's own
//! [`PreviewViewport`](simple3d_core::scene::PreviewViewport) setting, because
//! whether the grid, the axes or the rest of the scene is the nuisance depends
//! on what is being cut.
//!
//! Being non-modal also means the shape can change underneath the tool, so it
//! re-bakes what it is cutting whenever the evaluation moves on, and closes
//! itself if the shape goes away.

use crate::app::{App, Status};
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::worker::SplitJob;
use crate::{theme, ui};
use simple3d_core::scene::NodeId;
use simple3d_core::unit::Unit;
use simple3d_core::xform::Xform;
use simple3d_geom::tiling::{self, CellKind, Tiling};
use simple3d_geom::{Mesh, Vec3};
use std::sync::Arc;

/// What the tool is working on while its window is open.
pub struct SplitTool {
    pub target: NodeId,
    /// The shape as it stands, baked in its own frame: what the cells are cut
    /// out of, what the estimate is counted over, and what the picture is drawn
    /// from.
    ///
    /// Re-baked whenever the evaluation moves on, because the window is not
    /// modal: the shape can be edited, hidden or moved while the tool is open,
    /// and a plan drawn over the shape as it *was* is a plan of cuts that will
    /// not fall there.
    pub mesh: Arc<Mesh>,
    pub bounds: (Vec3, Vec3),
    /// Where the baked frame stands in the world, so the cells can be drawn on
    /// the shape rather than beside it. The tiling is worked out in the shape's
    /// own frame -- that is the frame a cell size in millimetres means something
    /// in -- and this is the way back out to the viewport.
    pub placement: Xform,
    /// Which evaluation `mesh` was baked from, so the tool can tell when what
    /// it is drawing has gone stale.
    pub generation: u64,
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
        let (mesh, placement) = self.bake_for_split(id);
        let Some(bounds) = mesh.bounds() else {
            self.status = Status::Warning("There is no geometry there to split".into());
            return;
        };
        let tiling = self.scene.node(id).split_tiling().unwrap_or(self.settings.last_split);
        self.split_tool = Some(SplitTool {
            target: id,
            mesh: Arc::new(mesh),
            bounds,
            placement,
            generation: self.evaluation_generation,
            tiling,
            text: Fields::from(&tiling, self.unit()),
        });
    }

    /// The shape to be cut, in its own frame, and where that frame stands in
    /// the world -- one for the cutting and the numbers, the other for drawing
    /// the cells on the model.
    fn bake_for_split(&self, id: NodeId) -> (Mesh, Xform) {
        let parent = self.evaluated.node_frames.get(&id).copied().unwrap_or(Xform::IDENTITY);
        simple3d_core::eval::baked_mesh_in_place(&self.scene, id, parent)
    }

    /// Keep the open tool honest against a document that can change underneath
    /// it, which a non-modal window can.
    ///
    /// Three things can happen to the shape while the tool is up: it can be
    /// deleted, which closes the tool; it can be edited, which re-bakes what the
    /// picture is drawn from; and it can be left alone, which is the usual case
    /// and costs one integer comparison.
    pub(crate) fn refresh_split_tool(&mut self) {
        let Some(tool) = self.split_tool.as_ref() else { return };
        if !self.scene.contains(tool.target) {
            self.cancel_split_tool();
            self.status = Status::Warning("The object being split is no longer there".into());
            return;
        }
        if tool.generation == self.evaluation_generation {
            return;
        }
        let target = tool.target;
        let (mesh, placement) = self.bake_for_split(target);
        let tool = self.split_tool.as_mut().expect("it was there a line ago");
        tool.generation = self.evaluation_generation;
        // The placement follows the shape whatever happens to the geometry: a
        // shape merely moved is the same cut in a new place, and the cells have
        // to move with it.
        tool.placement = placement;
        // A shape edited down to nothing -- hidden, or emptied of children --
        // leaves the last numbers up rather than blanking the window: they are
        // still worth reading, and Split refuses on its own.
        if let Some(bounds) = mesh.bounds() {
            tool.bounds = bounds;
            tool.mesh = Arc::new(mesh);
        }
    }

    /// Start cutting, and put the window away. The document is not touched until
    /// the pieces arrive.
    pub fn start_split(&mut self) {
        let Some(tool) = self.split_tool.take() else { return };
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
    }
}

/// The tool's own window, drawn over the viewport once a frame while it is open
/// (issue 82).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    app.refresh_split_tool();
    if app.split_tool.is_none() {
        return;
    }
    let bounds = app.viewport_rect;
    // The window names what it is cutting. It has to: it is not modal, so the
    // selection can move on to something else while it is open, and a window
    // that only says "Split into smaller pieces" would leave no way to tell
    // which object is about to be cut.
    let title = app
        .split_tool
        .as_ref()
        .and_then(|tool| app.scene.get(tool.target))
        .map_or_else(|| "Split into smaller pieces".to_string(), |node| format!("Split {} into pieces", node.name));
    // Taken out of the map for the duration, so the popup may hold it mutably
    // while its contents hold the application.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event = popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: &title, width: WIDTH }, |ui| {
        body(app, ui);
        popup::action_row(ui, |ui| actions(app, ui));
    });
    app.popups.insert(KEY, placement);
    if event == PopupEvent::Closed {
        app.cancel_split_tool();
    }
}

/// Identifies the popup, and is what remembers where it was dragged to.
const KEY: &str = "split-tool";
/// How wide the window is: enough for a labelled field and a plan of the cells
/// under it, and no wider. A popup lives over the model, so every pixel of it
/// is a pixel of the thing being cut that cannot be seen.
const WIDTH: f32 = 340.0;

/// What to call a number of pieces of a given cell shape.
fn plural_cells(kind: CellKind, count: usize) -> String {
    if count == 1 {
        kind.singular().to_string()
    } else {
        kind.label().to_lowercase()
    }
}

/// The tool's contents: the pattern, and what it comes to.
///
/// One column of fields and no picture. The picture is the viewport -- see
/// [`preview`] -- which is the whole reason the window is a popup floating over
/// it rather than a dialog in front of it: a plan drawn small inside the window
/// answers "what shape are the cells", and the model behind it answers "where
/// will they fall", which is the question actually being asked.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    if app.split_tool.is_none() {
        ui.label("The object this was opened on is no longer there.");
        return;
    }
    controls(app, ui);
    ui.add_space(6.0);
    summary(app, ui);
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

fn controls(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    let Some(tool) = app.split_tool.as_mut() else { return };
    // The cell shapes are a row of their own above the grid rather than a cell
    // in it. Four chips do not fit across the width of a popup, and an
    // `egui::Grid` does not grow its row for a wrapped one: the fourth landed
    // on top of the row below, which is the Size field.
    ui.label(egui::RichText::new("Cells").size(theme::font::LABEL).color(theme::token::TEXT_LO));
    ui.horizontal_wrapped(|ui| {
        for kind in CellKind::ALL {
            if theme::choice(ui, tool.tiling.kind == kind, kind.label()).on_hover_text(kind.size_meaning()).clicked() {
                tool.tiling.kind = kind;
            }
        }
    });
    ui.add_space(6.0);
    egui::Grid::new("split-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
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

/// Where the cuts will fall, drawn over the model in the viewport (issue 82).
///
/// This is the tool's preview, and it is in the viewport rather than in the
/// window on purpose. A plan drawn inside the window can only ever show the
/// tiling seen straight down its own axis; the question that actually stops
/// people -- will this cut fall through the middle of that boss, is the grid
/// turned the way I think it is -- is a question about the *shape*, and it is
/// answered by drawing the cells on the shape and turning the model.
///
/// The cells live in the shape's own frame, so every loop goes out through the
/// tool's `placement` before it is projected. Loops entirely behind the camera
/// are dropped; a loop crossing the eye plane is dropped too rather than drawn
/// through infinity, which is what projecting a point behind the eye would
/// otherwise do to it.
pub(crate) fn preview(app: &App, painter: &egui::Painter, view: &crate::view::View) {
    let Some(tool) = app.split_tool.as_ref() else { return };
    if tool.tiling.refusal(tool.bounds).is_some() {
        return;
    }
    let stroke = egui::Stroke::new(1.0_f32, theme::token::ACCENT);
    for loop_ in tiling::preview_loops(&tool.tiling, tool.bounds, PREVIEW_LOOPS) {
        let mut points = Vec::with_capacity(loop_.len());
        let mut whole = true;
        for point in loop_ {
            match view.project(tool.placement.point(point)) {
                Some((at, _)) => points.push(at),
                None => {
                    whole = false;
                    break;
                }
            }
        }
        if !whole || points.len() < 2 {
            continue;
        }
        // Cheap rejection before the shape is queued: at a close zoom most of
        // the grid is off the edges, and a closed line egui has to clip is still
        // a closed line egui has to hold.
        let clip = painter.clip_rect().expand(8.0);
        if points.iter().all(|p| !clip.contains(*p)) {
            continue;
        }
        painter.add(egui::Shape::closed_line(points, stroke));
    }
}

/// The most cell outlines the preview will draw in a frame.
///
/// A split may ask for ten thousand cells, and the preview draws them at both
/// ends of the run and at every layer between -- which is a number of line
/// loops that costs more per frame than the picture is worth. Past this the
/// preview is the part of the grid that was laid down first, which is the far
/// end and then the near one.
const PREVIEW_LOOPS: usize = 3_000;
