//! The tool that cuts a shape into a pattern of smaller pieces (issue 82).
//!
//! It cuts a shape that is in a single piece: into squares, rectangles,
//! triangles or hexagons, running through the shape along an axis and
//! optionally cut into layers across it as well. A cut can be made more than
//! once over -- each cut on its own axis, in its own cell shape, applied to what
//! the last one left -- so a plate can be scored into a grid of blocks in one
//! gesture rather than by splitting a split. It ends in a
//! [`Body::Split`](simple3d_core::scene::Body::Split) standing where the shape
//! stood, holding the pieces and the shape itself -- so it is undone by one
//! Join back together, however long afterwards.
//!
//! The window is where the pattern is chosen, and the cells are drawn **over
//! the model itself** while they are being chosen: the one question a number of
//! millimetres cannot answer on its own is what it looks like against the thing
//! being cut, and the honest answer to that is the thing being cut. They are
//! drawn *by the renderer*, with the depth buffer, so a cut on the far side of
//! the shape is behind it -- a grid that shows through the solid it is drawn on
//! reads as lying in front of it, and which cells are on the face turned
//! towards you is most of what the picture is for. Nothing is
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
use simple3d_core::primitive::ParamKind;
use simple3d_core::scene::NodeId;
use simple3d_core::xform::Xform;
use simple3d_geom::tiling::{CellKind, SplitPlan, Tiling};
use simple3d_geom::{Mesh, Vec3};
use std::hash::{Hash, Hasher};
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
    /// The cuts to make, in order: each one applied to the pieces the last left,
    /// so two of them on different axes make the blocks their grids come to
    /// between them.
    pub plan: SplitPlan,
}

impl SplitTool {
    /// Everything the preview drawn over the model depends on, for the key the
    /// viewport's cached image is rebuilt on. The loops themselves are not
    /// hashed: they are thousands of points, rebuilt from exactly these numbers.
    pub(crate) fn hash_preview<H: Hasher>(&self, hasher: &mut H) {
        self.target.hash(hasher);
        self.generation.hash(hasher);
        for tiling in &self.plan.passes {
            tiling.kind.hash(hasher);
            tiling.axis.hash(hasher);
            for number in [tiling.size, tiling.depth, tiling.angle, tiling.layer, tiling.offset[0], tiling.offset[1]] {
                number.to_bits().hash(hasher);
            }
        }
        for point in [self.bounds.0, self.bounds.1, self.placement.t] {
            for number in [point.x, point.y, point.z] {
                number.to_bits().hash(hasher);
            }
        }
        for row in self.placement.m {
            for number in row {
                number.to_bits().hash(hasher);
            }
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
        let plan = self.scene.node(id).split_plan().cloned().unwrap_or_else(|| self.settings.last_split.clone());
        self.split_tool = Some(SplitTool {
            target: id,
            mesh: Arc::new(mesh),
            bounds,
            placement,
            generation: self.evaluation_generation,
            plan,
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
        if tool.plan.refusal(tool.bounds).is_some() || !self.scene.contains(tool.target) {
            return;
        }
        // The shape as it is now, to be compared against the shape as it is when
        // the pieces land: a split applied to something that was edited while it
        // was being cut would be pieces of a shape that no longer exists.
        let Some(before) = self.scene.export_subtree(tool.target) else { return };
        let name = self.scene.node(tool.target).name.clone();
        self.settings.last_split = tool.plan.clone();
        self.persist();
        let job = SplitJob::spawn(tool.target, self.active, before, tool.mesh, tool.plan);
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
        let kind = job.plan.first().kind;
        if pieces.len() < 2 {
            self.status = Status::Warning(format!(
                "{} that size leave the shape in one piece -- try a smaller cell",
                kind.label()
            ));
            return;
        }
        self.edit("Split into smaller pieces", None);
        let Some((name, count)) = self.hold_pieces(job.node, pieces, Some(job.plan.clone())) else {
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
        // Three cuts is three columns of fields, which is taller than a short
        // viewport: the body scrolls rather than pushing Split and Cancel off
        // the bottom of the screen where nothing can reach them.
        let (area, restore) = theme::list_scroll_area(ui);
        area.auto_shrink([false, true]).max_height(popup::body_room(bounds)).show(ui, |ui| {
            ui.set_style(restore);
            body(app, ui);
        });
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

/// The tool's contents: the cuts to make, and what they come to.
///
/// One column of fields and no picture. The picture is the viewport -- see
/// [`preview_loops`] -- which is the whole reason the window is a popup
/// floating over it rather than a dialog in front of it: a plan drawn small
/// inside the window answers "what shape are the cells", and the model behind
/// it answers "where will they fall", which is the question actually being
/// asked.
///
/// The tool is lifted out of the application for the length of the drawing and
/// put back at the end. The fields are the same control the properties panel's
/// rows are -- dragged to change the number, clicked to type it -- and that
/// control lives on the application, so the two cannot be borrowed from it at
/// once.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut tool) = app.split_tool.take() else {
        ui.label("The object this was opened on is no longer there.");
        return;
    };
    controls(app, ui, &mut tool);
    ui.add_space(6.0);
    summary(app, ui, &tool);
    app.split_tool = Some(tool);
}

pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    let ready = app
        .split_tool
        .as_ref()
        .is_some_and(|tool| tool.plan.refusal(tool.bounds).is_none() && app.scene.contains(tool.target));
    if ui::dialog_button(ui, "Split", ready).clicked() {
        app.start_split();
    }
    // Cancel is at the other end of the row, not beside Split. The row is laid
    // out from the right, so the button that goes through with the command sits
    // under the pointer's own corner; the one that throws the window away is as
    // far from it as the window is wide, which is the distance a press nobody
    // meant has to cross.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            app.cancel_split_tool();
        }
    });
}

/// Every cut in the plan, and the way to add or drop one.
fn controls(app: &mut App, ui: &mut egui::Ui, tool: &mut SplitTool) {
    let cuts = tool.plan.passes.len();
    let mut drop = None;
    let mut reset = None;
    for (index, tiling) in tool.plan.passes.iter_mut().enumerate() {
        if index > 0 {
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(2.0);
        }
        // The header names the cut only when there is more than one: a window
        // that says "Cut 1" over a single cut is answering a question nobody
        // had. The buttons on the right of it are there either way.
        ui.horizontal(|ui| {
            if cuts > 1 {
                ui.label(
                    egui::RichText::new(format!("Cut {}", index + 1))
                        .size(theme::font::LABEL)
                        .color(theme::token::TEXT_HI),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if cuts > 1 && drop_button(ui, index) {
                    drop = Some(index);
                }
                // Back to the numbers a cut starts with: no turn, no offset, no
                // layers, and the stock cell size. Not literally every number
                // to zero -- a cell of no size is a split that is refused, so
                // the size goes back to the one a new cut opens on -- and not
                // the cell shape or the axis, which are choices rather than
                // numbers and are the two things worth keeping while the
                // numbers are thrown away.
                if ui
                    .button("Reset")
                    .on_hover_text(
                        "Put this cut's numbers back: no turn, no offset, no layers, and the cell size a new \
                         cut opens on. The cell shape and the axis are left as they are.",
                    )
                    .clicked()
                {
                    *tiling = Tiling { kind: tiling.kind, axis: tiling.axis, ..Tiling::default() };
                    reset = Some(index);
                }
            });
        });
        pass(app, ui, index, tiling);
    }
    // A field being typed into holds its own text until it is left, and that
    // text is what would be read back over the numbers this just put right.
    if let Some(index) = reset {
        for part in ["size", "depth", "angle", "layer", "offset-0", "offset-1"] {
            app.fields.forget(egui::Id::new(("split-field", field_name(index, part))));
        }
    }
    if let Some(index) = drop {
        tool.plan.passes.remove(index);
    }
    if tool.plan.passes.len() < simple3d_geom::tiling::MAX_PASSES {
        ui.add_space(6.0);
        // The new cut starts on the next axis round rather than on the one
        // already being cut: two identical tilings on the same axis are one
        // tiling, so the useful second cut is the one across the first.
        let next =
            tool.plan.passes.last().map_or_else(Tiling::default, |last| Tiling { axis: (last.axis + 1) % 3, ..*last });
        if ui
            .button("Add another cut")
            .on_hover_text(
                "Cut the pieces again, on another axis or in another shape. The pieces are what both cuts \
                 leave -- a plate scored into squares and then into slabs comes back as blocks.",
            )
            .clicked()
        {
            tool.plan.passes.push(next);
        }
    }
}

/// The cross that drops one cut, drawn rather than written.
///
/// The same two strokes the popup's own close cross is, for the same reason: a
/// cross typed as a character is a character the interface font may not have,
/// and the one it puts in its place is an empty box.
fn drop_button(ui: &mut egui::Ui, index: usize) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(14.0), egui::Sense::click());
    let colour = if response.hovered() { theme::token::DANGER } else { theme::token::TEXT_LO };
    let arm = rect.shrink(3.5);
    let stroke = egui::Stroke::new(1.4_f32, colour);
    ui.painter().line_segment([arm.left_top(), arm.right_bottom()], stroke);
    ui.painter().line_segment([arm.right_top(), arm.left_bottom()], stroke);
    // Painted, so nothing would otherwise say what it is: to anything reading
    // the interface it was an unnamed rectangle.
    let label = format!("Drop cut {}", index + 1);
    let name = label.clone();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    response.on_hover_text(&label).clicked()
}

/// One cut of the plan: what shape its cells are, how big, and where they run.
fn pass(app: &mut App, ui: &mut egui::Ui, index: usize, tiling: &mut Tiling) {
    // Every field says what it is measured in, the way the properties panel's
    // rows do: a number in a box is a number in some unit, and which one is not
    // something to work out from the document setting three panels away.
    let length = format!("({})", app.unit().suffix());
    // The cell shapes are a row of their own above the grid rather than a cell
    // in it. Four chips do not fit across the width of a popup, and an
    // `egui::Grid` does not grow its row for a wrapped one: the fourth landed
    // on top of the row below, which is the Size field.
    ui.label(egui::RichText::new("Cells").size(theme::font::LABEL).color(theme::token::TEXT_LO));
    ui.horizontal_wrapped(|ui| {
        for kind in CellKind::ALL {
            if theme::choice(ui, tiling.kind == kind, kind.label()).on_hover_text(kind.size_meaning()).clicked() {
                tiling.kind = kind;
            }
        }
    });
    ui.add_space(6.0);
    egui::Grid::new(("split-grid", index)).num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        label(ui, &format!("Size {length}"), tiling.kind.size_meaning());
        number(app, ui, &field_name(index, "size"), SIZE, &mut tiling.size);
        ui.end_row();

        if tiling.kind.has_depth() {
            label(ui, &format!("Depth {length}"), "The second side of one rectangle.");
            number(app, ui, &field_name(index, "depth"), SIZE, &mut tiling.depth);
            ui.end_row();
        }

        // The axis is the direction the cells *run in*, not the plane they lie
        // in, which is the way round a cut is thought about: a plate lying flat
        // is cut into columns standing up it, which is Z.
        ui.label("Through");
        ui.horizontal(|ui| {
            for (axis, name) in [(0u8, "X"), (1, "Y"), (2, "Z")] {
                if theme::choice(ui, tiling.axis == axis, name).clicked() {
                    tiling.axis = axis;
                }
            }
        })
        .response
        .on_hover_text("The axis the cells run along. The tiling lies in the plane across it.");
        ui.end_row();

        label(ui, "Turn (deg)", "Turn the whole grid within its plane, in degrees.");
        number(app, ui, &field_name(index, "angle"), ParamKind::Angle { min: -360.0, max: 360.0 }, &mut tiling.angle);
        ui.end_row();

        label(
            ui,
            &format!("Offset {length}"),
            "Move the grid within its plane. The cells are centred on the shape until this says otherwise.",
        );
        ui.horizontal(|ui| {
            for i in 0..2 {
                let name = field_name(index, if i == 0 { "offset-0" } else { "offset-1" });
                number(app, ui, &name, ParamKind::Length { min: f64::NEG_INFINITY }, &mut tiling.offset[i]);
            }
        });
        ui.end_row();

        // "Layer height" rather than "Layers": the number is how tall one layer
        // is, and a row called Layers holding a 4 reads as four of them.
        label(
            ui,
            &format!("Layer height {length}"),
            "Cut across the cells as well, into layers this tall. Zero cuts straight through.",
        );
        number(app, ui, &field_name(index, "layer"), ParamKind::Length { min: 0.0 }, &mut tiling.layer);
        ui.end_row();
    });
}

/// A row's name, carrying what the number beside it means.
///
/// The hover is on the label rather than on the field, exactly as the
/// properties panel puts it: the field is dragged and typed into, and a tooltip
/// that appears under the pointer halfway through a drag is a tooltip in the
/// way of the thing it is describing.
fn label(ui: &mut egui::Ui, name: &str, hover: &str) {
    ui.label(name).on_hover_text(hover);
}

/// A cell size: never negative, and never quite zero -- a cell of no size is
/// refused by the tiling itself, and a field that can reach it only wastes the
/// press that finds out.
const SIZE: ParamKind = ParamKind::Length { min: 0.0 };

/// What one field is called. The name is what the drag gesture is remembered
/// by, so it carries the cut it belongs to: two cuts have two Size fields, and
/// a gesture handed from one to the other would edit the wrong one.
fn field_name(index: usize, part: &str) -> String {
    format!("split-{index}-{part}")
}

/// A number that is dragged to change it and clicked to type into it -- the
/// same control the properties panel's rows are, through the same buffers, so
/// the two answer the pointer identically (issue 82).
///
/// The tiling it edits is not a document parameter, so there is no undo step to
/// take and nothing to mark for re-evaluation: the value goes straight into the
/// plan the window holds, and the model is not touched until Split is pressed.
fn number(app: &mut App, ui: &mut egui::Ui, name: &str, kind: ParamKind, value: &mut f64) {
    let unit = app.unit();
    let shown = match kind {
        ParamKind::Length { .. } => simple3d_core::unit::format_length(*value, unit),
        _ => simple3d_core::unit::format_angle(*value),
    };
    let id = egui::Id::new(("split-field", name));
    let step = ui::scrub_increment(kind, unit);
    // The scrub state is lifted out and put back so the field can borrow the
    // buffers mutably without borrowing the whole application twice.
    let mut scrub = app.scrub;
    let outcome = ui
        .scope(|ui| {
            ui.set_max_width(FIELD_WIDTH);
            app.fields.scrub_field(ui, id, crate::panel_properties::grip_id(name), &shown, step, &mut scrub)
        })
        .inner;
    app.scrub = scrub;
    if let Some(scrubbed) = outcome.scrubbed {
        let displayed = match kind {
            ParamKind::Length { .. } => unit.from_mm(*value),
            _ => *value,
        };
        *value = ui::param_number(ui::value_from_display(kind, unit, displayed + scrubbed.delta));
    }
    if let Some(text) = outcome.committed {
        match ui::commit_param(&text, kind, unit, *value) {
            ui::Commit::Value(committed) => {
                app.fields.accept(id);
                *value = ui::param_number(committed);
            }
            ui::Commit::Revert => {
                app.fields.reject(id, text.clone());
                app.status = Status::Info(format!("\"{text}\" is not a number this field can take"));
            }
        }
    }
}

/// How wide a number field is: enough for a length with its unit on it, and no
/// wider -- a popup lives over the model, and every pixel of it is a pixel of
/// the thing being cut that cannot be seen.
const FIELD_WIDTH: f32 = 90.0;

/// What the plan comes to: how many cells it lays over the shape, or why it
/// cannot be cut at all.
fn summary(app: &mut App, ui: &mut egui::Ui, tool: &SplitTool) {
    match tool.plan.refusal(tool.bounds) {
        Some(why) => {
            ui.add(egui::Label::new(egui::RichText::new(why).size(theme::font::LABEL).color(theme::token::ACCENT)));
        }
        None => {
            let cells = tool.plan.planned(tool.bounds);
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

/// Where the cuts will fall, in world space, for the renderer to draw over the
/// model (issue 82).
///
/// This is the tool's preview, and it is in the viewport rather than in the
/// window on purpose. A plan drawn inside the window can only ever show the
/// tiling seen straight down its own axis; the question that actually stops
/// people -- will this cut fall through the middle of that boss, is the grid
/// turned the way I think it is -- is a question about the *shape*, and it is
/// answered by drawing the cells on the shape and turning the model.
///
/// The loops go to the renderer rather than to the 2D painter so that the depth
/// buffer can have them: a cell on the far side of the solid is behind it, and
/// a grid drawn through the shape reads as floating in front of it. The cells
/// live in the shape's own frame, so every loop goes out through the tool's
/// `placement` on the way.
pub(crate) fn preview_loops(app: &App) -> Vec<Vec<Vec3>> {
    let Some(tool) = app.split_tool.as_ref() else { return Vec::new() };
    if tool.plan.refusal(tool.bounds).is_some() {
        return Vec::new();
    }
    tool.plan
        .preview_loops(tool.bounds, PREVIEW_LOOPS)
        .into_iter()
        .map(|loop_| loop_.into_iter().map(|point| tool.placement.point(point)).collect())
        .collect()
}

/// The most cell outlines the preview will draw in a frame.
///
/// A split may ask for ten thousand cells, and the preview draws them at both
/// ends of the run and at every layer between -- which is a number of line
/// loops that costs more per frame than the picture is worth. Past this the
/// preview is the part of the grid that was laid down first, which is the far
/// end and then the near one.
const PREVIEW_LOOPS: usize = 3_000;
