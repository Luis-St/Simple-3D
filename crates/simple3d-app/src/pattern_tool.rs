//! The custom pattern kind creation tool (issue 67).
//!
//! The six kinds a pattern ships with are the ones worth having a name for.
//! This is where a user makes their own: a rule built out of *stages*, each one
//! repeating whatever the stages before it made, named and kept on a shelf for
//! any other project.
//!
//! It edits the pattern node itself rather than a draft of one. A rule is only
//! ever judged by what it lays out, so every number typed here moves the copies
//! in the viewport behind the window at once, and every one of them is an
//! ordinary parameter edit -- which is why undo, the project file and the
//! clipboard needed nothing added for any of this.
//!
//! What is saved to the shelf is the *rule*, not the pattern: the stage numbers
//! and nothing else. A node still stores those numbers itself, so a project
//! opened on a machine that has never seen the kind still lays out correctly.

use crate::app::{App, Modal, Status};
use crate::render::{self, Grid, Item, Palette, Style};
use crate::view::View;
use crate::{theme, ui};
use simple3d_core::pattern;
use simple3d_core::pattern_library;
use simple3d_core::primitive::{ParamValue, Params, ParamsExt};
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    /// Open the tool on the selected pattern -- making one out of the selection
    /// first if what is selected is not a pattern yet, which is what makes this
    /// a *creation* tool and not merely an editor of one.
    pub fn open_pattern_tool(&mut self) {
        let existing = self.selection.iter().copied().find(|id| self.scene.get(*id).is_some_and(|n| n.is_pattern()));
        let target = match existing {
            Some(id) => Some(id),
            None => {
                self.make_pattern();
                self.selection.iter().copied().find(|id| self.scene.get(*id).is_some_and(|n| n.is_pattern()))
            }
        };
        let Some(id) = target else {
            self.status = Status::Warning("That selection cannot be made into a pattern".into());
            return;
        };
        // The tool only ever builds a custom rule, so the node is switched to
        // one on the way in. Switching kinds writes no numbers of its own --
        // every kind keeps its own -- so the six fixed kinds are still there,
        // unchanged, if the user picks one again afterwards.
        if self.scene.node(id).params().map(|p| p.int("kind")) != Some(pattern::CUSTOM) {
            self.edit("Custom pattern", None);
            if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
                params.insert("kind".to_string(), ParamValue::Choice(pattern::CUSTOM));
            }
        }
        self.pattern_tool = Some(id);
        self.pattern_tool_name = self.scene.node(id).name.clone();
        self.refresh_pattern_kinds();
        self.reset_pattern_preview();
        self.modal = Modal::PatternKind;
    }

    /// Put the preview back the way it opens: looking at the pattern from where
    /// the viewport is looking at the scene, backed off far enough to hold what
    /// the rule lays out.
    ///
    /// Same angle, so the picture in the window and the one behind it agree
    /// about which way round the shape is; its own camera from there, so
    /// turning one does not turn the other. The framing itself waits for the
    /// next frame, which is where the picture's own shape is known -- forgetting
    /// what it was framed on is what asks for it, in `preview`.
    ///
    /// This is what the Frame button does, and it resets the angle and the zoom
    /// with the rest. The *automatic* reframing, when a rule starts laying its
    /// copies out somewhere else, deliberately does not -- see
    /// `frame_pattern_preview`.
    pub(crate) fn reset_pattern_preview(&mut self) {
        self.pattern_preview_camera = self.scene.camera;
        self.pattern_preview_bounds = None;
    }

    /// What the preview is looking at: the pattern, what the rule lays out while
    /// there is nothing in the pattern to lay out, or the whole scene.
    ///
    /// The middle one is the case a pattern is *built* in. A pattern with
    /// nothing in it has no bounds of its own, and framing on the scene instead
    /// pointed the picture at everything except the thing the window is open
    /// for -- and, with an empty scene behind it, at nothing at all, which is
    /// where the Frame button had nothing to do.
    pub(crate) fn pattern_preview_target(&self) -> Option<(Vec3, Vec3)> {
        self.pattern_tool
            .and_then(|id| self.evaluated.node_world_bounds.get(&id).copied())
            .or_else(|| self.pattern_placement_bounds())
            .or_else(|| self.evaluated.mesh.bounds())
    }

    /// Where the rule would put a copy, in world space. These are the
    /// placements a pattern still has when there is no shape in it to place.
    pub(crate) fn pattern_placements(&self) -> Vec<Vec3> {
        let Some(id) = self.pattern_tool_target() else { return Vec::new() };
        let Some(params) = self.scene.node(id).params() else { return Vec::new() };
        // The instances are laid out in the pattern's own frame; the gizmo's is
        // what carries that frame out into the world, the same way the lay-out
        // grips in the viewport are placed.
        let Some(gizmo) = self.gizmo_for(id) else { return Vec::new() };
        pattern::instances(params).into_iter().map(|copy| gizmo.own.point(copy.xform.t)).collect()
    }

    /// The room the placements take, with air around them -- a rule that lays
    /// out one copy is a single point, and a picture framed on a point is a
    /// grid line filling the window.
    fn pattern_placement_bounds(&self) -> Option<(Vec3, Vec3)> {
        let places = self.pattern_placements();
        let (first, rest) = places.split_first()?;
        let (mut lo, mut hi) = (*first, *first);
        for at in rest {
            lo = lo.min(*at);
            hi = hi.max(*at);
        }
        let pad = self.scene.settings.grid_spacing.max(1.0);
        let pad = Vec3::new(pad, pad, pad);
        Some((lo - pad, hi + pad))
    }

    /// A pattern with the rule but not yet the shape it repeats. Its preview
    /// has no geometry to render, so it is drawn as its placements instead.
    pub(crate) fn pattern_tool_is_empty(&self) -> bool {
        self.pattern_tool_target().is_some_and(|id| !self.evaluated.node_world_bounds.contains_key(&id))
    }

    /// Point the preview camera at the pattern, at the aspect the picture is
    /// actually drawn at, and remember what it was pointed at.
    ///
    /// Only the target and the distance move: which way round the pattern is
    /// seen from is the user's, and the *automatic* framing -- which happens
    /// whenever a rule starts laying its copies out somewhere else -- must not
    /// undo an orbit halfway through one. Asking for it by name, with the Frame
    /// button, is a different thing: see `reset_pattern_preview`.
    pub(crate) fn frame_pattern_preview(&mut self, aspect: f64) {
        self.pattern_preview_bounds = self.pattern_preview_target();
        match self.pattern_preview_bounds {
            Some((lo, hi)) => crate::view::frame_bounds(&mut self.pattern_preview_camera, lo, hi, aspect),
            None => {
                self.pattern_preview_camera.target = Vec3::ZERO;
                self.pattern_preview_camera.distance = 160.0;
            }
        }
    }

    /// Re-read the shelf. Done when the tool opens and after it is written to,
    /// rather than every frame: the shelf is a directory and the dialog draws
    /// sixty times a second.
    pub(crate) fn refresh_pattern_kinds(&mut self) {
        self.pattern_kinds = pattern_library::list(self.config_dir());
    }

    /// The pattern the tool is working on, if it is still there: the outliner
    /// behind the dialog can delete it while the dialog is open.
    fn pattern_tool_target(&self) -> Option<NodeId> {
        self.pattern_tool.filter(|id| self.scene.get(*id).is_some_and(|n| n.is_pattern()))
    }

    fn pattern_tool_params(&self) -> Params {
        self.pattern_tool_target().and_then(|id| self.scene.node(id).params().cloned()).unwrap_or_default()
    }

    /// How many stages the rule uses. Adding one starts it from the defaults it
    /// was born with, so a stage that has just appeared does something visible
    /// rather than sitting at zero and looking broken.
    fn set_stage_count(&mut self, wanted: usize) {
        let Some(id) = self.pattern_tool_target() else { return };
        let wanted = wanted.clamp(1, pattern::MAX_STAGES) as u32;
        self.edit("Pattern stages", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            params.insert("stages".to_string(), ParamValue::Count(wanted));
        }
    }

    /// Put a saved rule on the node the tool is working on.
    pub(crate) fn apply_saved_kind(&mut self, entry: &pattern_library::Entry) {
        let Some(id) = self.pattern_tool_target() else { return };
        let Some(kind) = pattern_library::load(&entry.path) else {
            self.status = Status::Warning(format!("\u{201C}{}\u{201D} could not be read", entry.name));
            return;
        };
        self.edit("Pattern kind", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            pattern_library::apply(params, &kind);
        }
        // The node takes the kind's name, but only while it still carries the
        // one it was given automatically: a pattern the user has named
        // themselves keeps that name.
        if self.scene.node(id).name.starts_with("Pattern") {
            if let Some(node) = self.scene.get_mut(id) {
                node.name = entry.name.clone();
            }
        }
        self.pattern_tool_name = entry.name.clone();
        self.status = Status::Info(format!("Pattern kind \u{201C}{}\u{201D} applied", entry.name));
    }

    /// Keep the rule the node currently holds on the shelf, under the name in
    /// the dialog's name field.
    pub(crate) fn save_current_kind(&mut self) {
        let params = self.pattern_tool_params();
        let name = simple3d_core::library::sanitise(&self.pattern_tool_name);
        match pattern_library::save(self.config_dir(), &name, &params) {
            Ok(_) => {
                self.refresh_pattern_kinds();
                self.status = Status::Info(format!("Pattern kind \u{201C}{name}\u{201D} saved"));
            }
            Err(e) => self.status = Status::Warning(format!("Could not save the pattern kind: {e}")),
        }
    }

    pub(crate) fn delete_saved_kind(&mut self, entry: &pattern_library::Entry) {
        match pattern_library::remove(&entry.path) {
            Ok(()) => {
                self.refresh_pattern_kinds();
                self.status = Status::Info(format!("Pattern kind \u{201C}{}\u{201D} deleted", entry.name));
            }
            Err(e) => self.status = Status::Warning(format!("Could not delete the pattern kind: {e}")),
        }
    }
}

/// The narrowest the stages are worth drawing at. Their rows are the property
/// panel's own, which stack a name above its field rather than beside it once
/// the room runs out, so the numbers stay usable well below the width two
/// columns need.
const STAGES_MIN: f32 = 240.0;
/// The widest the divider will drag them. Past this a stage row is a name, a
/// field and a stretch of nothing between the two, and the picture is paying
/// for it.
const STAGES_MAX: f32 = 560.0;
/// The smallest the preview can be and still be a viewport rather than a stamp.
const PREVIEW_MIN: f32 = 220.0;
/// The longest side the preview's image is rasterized at, whatever size it is
/// drawn at. See `paint_preview`.
const PREVIEW_MAX_PX: f32 = 1280.0;

/// The tool's contents: the rule down the left, and a viewport on what it lays
/// out taking everything else.
///
/// The divider between them is draggable, and the width it is dragged to is the
/// width the stages keep -- through a resize of the window and through the next
/// time the tool is opened, since it is kept with the dock widths. Widening the
/// window therefore widens the picture and nothing else, which is the point: a
/// stage row is a name and a number, and a hundred more pixels only push the
/// two apart, while the viewport is worth more the bigger it is.
///
/// Narrowed past what both need, the stages give width up before the picture
/// disappears, and narrower still the picture is the column that goes: the
/// viewport behind this window is showing the same scene, and the numbers are
/// what the window is open for.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(id) = app.pattern_tool_target() else {
        ui.label("The pattern this was opened on is no longer there.");
        return;
    };
    let room = ui.available_width();
    // What the divider itself costs, so the widest the stages may be still
    // leaves the picture its minimum.
    let rule = ui.spacing().item_spacing.x * 2.0 + 6.0;
    let widest = (room - PREVIEW_MIN - rule).min(STAGES_MAX);

    if widest < STAGES_MIN {
        rule_column(app, ui, id);
        return;
    }
    let wanted = app.settings.pattern_stages_width.clamp(STAGES_MIN, widest);
    let mut width = wanted;
    egui::SidePanel::left("pattern-stages-column")
        .frame(egui::Frame::NONE)
        .resizable(true)
        .default_width(wanted)
        // egui remembers a panel's width itself, so a window narrowed past what
        // the remembered width leaves the picture has to be told the new
        // ceiling -- otherwise the stages keep a width the window no longer has.
        .width_range(STAGES_MIN..=widest)
        .show_inside(ui, |ui| {
            width = ui.available_width();
            // Claim the column's full width up front. egui remembers a panel by
            // the rectangle its *content* filled, and a property row pins its
            // right edge `EDGE_PAD` inside that -- so the column came back eight
            // pixels narrower every frame, and being remembered, kept coming
            // back narrower until it hit its own minimum. The docks have to do
            // exactly this, for exactly this reason.
            ui.expand_to_include_rect(ui.max_rect());
            ui.set_min_width(width);
            rule_column(app, ui, id);
        });
    app.settings.pattern_stages_width = width.clamp(STAGES_MIN, STAGES_MAX);
    preview(app, ui);
}

/// The shelf and the stages, in that order down one column.
fn rule_column(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    if shelf(app, ui) {
        ui.separator();
    }
    stages(app, ui, id);
}

/// The saved kinds, as one row: pick one to put it on the pattern, and the
/// cross beside it takes that one off the shelf for good.
///
/// It was a column of its own, which on an empty shelf was a paragraph of
/// explanation taking a fifth of the window. Nothing saved, nothing drawn:
/// the row appears with the first kind kept, and until then the button that
/// keeps one is the only thing that needs to be there.
///
/// Returns whether it drew anything, so the caller knows whether to rule a line
/// under it.
fn shelf(app: &mut App, ui: &mut egui::Ui) -> bool {
    if app.pattern_kinds.is_empty() {
        return false;
    }
    let mut apply = None;
    let mut delete = None;
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(theme::header_text("Saved kinds")).selectable(false));
        // The rule on the pattern is the one named in the box, when that name is
        // a saved kind's -- which is what `apply_saved_kind` leaves behind, and
        // what the name field is filled with.
        let on_shelf = app.pattern_kinds.iter().find(|entry| entry.name == app.pattern_tool_name).cloned();
        let shown = match &on_shelf {
            Some(entry) => entry.name.clone(),
            None => "Pick one".to_string(),
        };
        egui::ComboBox::from_id_salt("pattern-shelf")
            .selected_text(theme::value(shown))
            .width((ui.available_width() - 40.0).clamp(80.0, 220.0))
            .show_ui(ui, |ui| {
                for entry in &app.pattern_kinds {
                    let chosen = Some(&entry.name) == on_shelf.as_ref().map(|e| &e.name);
                    if ui.selectable_label(chosen, &entry.name).clicked() {
                        apply = Some(entry.clone());
                    }
                }
            });
        if ui
            .add_enabled(on_shelf.is_some(), egui::Button::new("\u{00d7}"))
            .on_hover_text("Delete the saved kind named here")
            .clicked()
        {
            delete = on_shelf;
        }
    });
    if let Some(entry) = apply {
        app.apply_saved_kind(&entry);
    }
    if let Some(entry) = delete {
        app.delete_saved_kind(&entry);
    }
    true
}

/// The rule itself: how many stages, and each stage's numbers.
///
/// A stage is added by a button that says so and dropped by the cross on the
/// stage itself, rather than by a `+` and a `−` beside the count. A pair of
/// signs is the control for a *number*, and the number here is not the thing
/// being edited: what the buttons did was add and remove whole sections of the
/// form below them, and only the count they sat beside said so.
fn stages(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let params = app.pattern_tool_params();
    let used = pattern::stage_count(&params);
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(theme::header_text("Stages")).selectable(false));
        ui.add(egui::Label::new(theme::value(format!("{used} of {}", pattern::MAX_STAGES))).selectable(false));
    });
    ui.add(egui::Label::new(theme::hint("Each stage repeats what the ones above it made.")).selectable(false));

    let unit = app.unit();
    let room = ui.available_height();
    let mut wanted = used;
    egui::ScrollArea::vertical().max_height(room).id_salt("pattern-stages").show(ui, |ui| {
        for index in 0..used {
            let stage = pattern::stage(&params, index);
            // A rule is read as a stack of stages, so each one is ruled off from
            // the one above it: six pixels of air made the whole column one run
            // of rows, and which numbers belonged to which stage had to be
            // worked out from the names.
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(theme::header_text(pattern::STAGES[index].label)).selectable(false));
                ui.add(egui::Label::new(theme::hint(describe(&stage, unit))).selectable(false));
                // Only the last stage can go: a stage repeats what the ones
                // above it made, so there is no such thing as removing one from
                // the middle -- the stages below it would be repeating something
                // else. The cross is on the one that can, rather than on all of
                // them refusing.
                if index + 1 == used && used > 1 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("\u{00d7}")
                            .on_hover_text("Drop this stage. Only the last one can go: the others are what it repeats.")
                            .clicked()
                        {
                            wanted = used - 1;
                        }
                    });
                }
            });
            for key in pattern::stage_keys(index) {
                let Some(spec) = pattern::PARAMS.iter().find(|p| p.key == key) else { continue };
                if !pattern::param_visible(spec, &params) {
                    continue;
                }
                crate::panel_properties::param_field(
                    app,
                    ui,
                    &[id],
                    id,
                    spec,
                    unit,
                    crate::panel_properties::PATTERN_TOOL_ROW,
                );
            }
        }
        // With the rule full there is nothing to add, so the button goes rather
        // than sitting there greyed out -- and its rule goes with it, or the
        // last stage would be underlined by a line with nothing under it. The
        // count in the header already says the rule is full.
        if used < pattern::MAX_STAGES {
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(4.0);
            if ui.button("Add a stage").on_hover_text("Repeat what the stages above it make").clicked() {
                wanted = used + 1;
            }
        }
    });
    if wanted != used {
        app.set_stage_count(wanted);
    }
}

/// A line of English saying what one stage does, so the numbers above it can be
/// read without working them out.
fn describe(stage: &pattern::Stage, unit: simple3d_core::unit::Unit) -> String {
    if stage.mirror {
        return format!("mirrored across {}", ["X", "Y", "Z"][stage.axis.min(2)]);
    }
    let mut what: Vec<String> = Vec::new();
    let run = stage.step.length();
    if run > 1e-9 {
        what.push(format!("{} {} apart", simple3d_core::unit::format_length(run, unit), unit.suffix()));
    }
    if stage.turn.abs() > 1e-9 {
        what.push(format!(
            "turning {} deg about {}",
            simple3d_core::unit::format_number(stage.turn, 1),
            ["X", "Y", "Z"][stage.axis.min(2)]
        ));
    }
    if stage.radius.abs() > 1e-9 || stage.growth.abs() > 1e-9 {
        what.push(format!("at radius {} {}", simple3d_core::unit::format_length(stage.radius, unit), unit.suffix()));
    }
    if what.is_empty() {
        what.push("in place".to_string());
    }
    format!("{} copies, {}", stage.copies(), what.join(", "))
}

/// The id the preview's picture answers to. Named rather than taken from the
/// layout, so a test can ask the context where the picture was drawn and check
/// what is in it -- the same bargain every other grip in the application makes.
pub(crate) fn preview_id() -> egui::Id {
    egui::Id::new("pattern-preview")
}

/// What the rule lays out, as a viewport.
///
/// It was a flat scatter of dots, one per copy, because a rule is a handful of
/// numbers and nobody reads a helix out of six of them. Dots answer "how many
/// and roughly where" and nothing else, though, and the question a rule is
/// actually judged by -- what the shape looks like repeated -- needs the shape.
/// So this is the same render the viewport behind the window is drawing, from a
/// camera of its own that the pointer can turn.
///
/// Everything but the camera comes from the main window: the display mode, the
/// grid, the axes, the plane marks. An axis switched off out there is switched
/// off in here, because there is one set of view settings in the application
/// and this is not a second one.
fn preview(app: &mut App, ui: &mut egui::Ui) {
    let params = app.pattern_tool_params();
    ui.horizontal(|ui| {
        // Clear of the divider, the way a panel header stands clear of its dock's
        // edge. The picture below is left flush with it: a viewport against a rule
        // reads as a viewport, and a *word* against one reads as crowding.
        ui.add_space(theme::metric::PANEL_PAD);
        ui.add(egui::Label::new(theme::header_text("Lays out")).selectable(false));
        let (wanted, made) = pattern::instance_count(&params);
        let note = if wanted > made {
            format!("{made} copies -- {wanted} were asked for, which is more than can be drawn")
        } else if app.pattern_tool_is_empty() {
            // Otherwise the dots are a picture with no caption: the rule works,
            // and what is missing is the shape it has nothing to repeat.
            format!("{made} copies, marked -- put a shape in the pattern to see it repeated")
        } else {
            format!("{made} copies")
        };
        ui.add(egui::Label::new(theme::hint(note)).selectable(false));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("Frame")
                .on_hover_text("Put the preview back: the viewport's angle, framed on the pattern")
                .clicked()
            {
                app.reset_pattern_preview();
            }
        });
    });
    ui.add_space(4.0);

    // Whatever is left of the column, which is what makes this a viewport and
    // not a stamp: it grows with the window.
    let rect = ui.available_rect_before_wrap();
    if rect.width() < 32.0 || rect.height() < 32.0 {
        return;
    }
    // A rule that lays out more, or further, is a different thing to look at, so
    // the picture is framed on it again.
    //
    // Without this the camera was pointed once, when the tool opened, and stayed
    // there: adding a stage grew the pattern out past the edges of the preview
    // and what the new stage did was off the picture. Only the target and the
    // distance move, so an orbit survives it.
    if app.pattern_preview_bounds != app.pattern_preview_target() {
        app.frame_pattern_preview((rect.width() / rect.height().max(1.0)) as f64);
    }
    ui.allocate_rect(rect, egui::Sense::hover());
    let response = ui.interact(rect, preview_id(), egui::Sense::click_and_drag());
    paint_preview(app, ui, rect);
    if app.pattern_tool_is_empty() {
        paint_placements(app, ui, rect);
    }

    // The same navigation bindings the viewport uses, read from the keymap on
    // every frame, so a rebinding applies here as immediately as it does there.
    let nav = app.keymap.nav;
    let (ctrl, shift, alt) = ui.input(|i| (i.modifiers.command, i.modifiers.shift, i.modifiers.alt));
    let held = [
        response.dragged_by(egui::PointerButton::Primary),
        response.dragged_by(egui::PointerButton::Middle),
        response.dragged_by(egui::PointerButton::Secondary),
    ];
    if let Some(gesture) = crate::panel_viewport::nav_gesture(&nav, held, ctrl, shift, alt) {
        let view = View::new(app.pattern_preview_camera, rect);
        crate::panel_viewport::apply_gesture(&mut app.pattern_preview_camera, gesture, response.drag_delta(), &view);
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        crate::panel_viewport::apply_zoom(&mut app.pattern_preview_camera, &nav, scroll);
    }
    // No cursor of its own. The viewport this is a copy of leaves the pointer
    // alone while it is orbited, and a preview that swapped it for a hand said
    // the picture was something to pick up.
}

/// Orange dots where the rule would put a copy.
///
/// A pattern is *built* empty: the tool makes one out of the selection, and a
/// selection of nothing makes a pattern with nothing in it. That pattern has no
/// geometry, so the render behind this has nothing to draw and the picture is
/// the bare grid -- which reads as a preview that does not work rather than as
/// a pattern with nothing in it yet. The rule still has placements, and while
/// the shape is missing they are the whole of what the window has to show.
///
/// Drawn over the render rather than into it, in the picture's own camera, so
/// the dots sit on the grid exactly where the copies will and turn with an
/// orbit like everything else. The original is the one every other copy is a
/// copy *of*, so it is the one drawn brightest.
fn paint_placements(app: &App, ui: &egui::Ui, rect: egui::Rect) {
    let view = View::new(app.pattern_preview_camera, rect);
    let painter = ui.painter_at(rect);
    for (index, at) in app.pattern_placements().iter().enumerate() {
        // Orthographic, so there is no behind-the-camera to test for: every
        // placement lands somewhere, and the clip takes the ones off the
        // picture.
        let Some((screen, _)) = view.project(*at) else { continue };
        let (radius, colour) =
            if index == 0 { (5.0, theme::token::ACCENT) } else { (3.5, theme::token::ACCENT.gamma_multiply(0.7)) };
        painter.circle_filled(screen, radius, colour);
    }
}

/// Rasterize the preview, reusing the last image while nothing that affects it
/// has changed -- the same bargain the viewport's own image strikes, and the
/// reason a dialog that redraws sixty times a second can hold a render at all.
///
/// Always the software rasterizer: the GPU renderer draws into a texture owned
/// by the main window's context, and this is a second native window.
fn paint_preview(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect) {
    // Capped, unlike the viewport's own image: this one is rasterized in
    // software on every frame of an orbit, and on a large screen the dialog's
    // half of it is a million pixels. Past the cap the picture is drawn scaled,
    // which costs a little sharpness and keeps the drag smooth.
    let pixels_per_point = ui.ctx().pixels_per_point().min(PREVIEW_MAX_PX / rect.width().max(rect.height()));
    let size = [
        (rect.width() * pixels_per_point).round().max(1.0) as usize,
        (rect.height() * pixels_per_point).round().max(1.0) as usize,
    ];
    let dark = ui.visuals().dark_mode;
    let key = preview_key(app, size, dark);
    if key != app.pattern_preview_key || app.pattern_preview_texture.is_none() {
        // Scoped so the borrow of the renderables ends before the texture,
        // which lives on the same application, is written to.
        let image = {
            // The framebuffer's own space, not the dialog's position on screen.
            let render_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size[0] as f32, size[1] as f32));
            let mut items = vec![Item { renderable: &app.scene_renderable, style: Style::Solid }];
            // The pattern itself wears the selection outline, so which copies
            // the rule is laying out is never in doubt.
            if let Some(renderable) = app.pattern_tool.and_then(|id| app.node_renderables.get(&id)) {
                items.push(Item { renderable, style: Style::Selected });
            }
            let request = render::Request {
                view: View::new(app.pattern_preview_camera, render_rect),
                size,
                mode: app.settings.display_mode,
                palette: Palette::for_dark_mode(dark),
                grid: Grid {
                    visible: app.scene.settings.grid_visible,
                    spacing: app.scene.settings.grid_spacing,
                    axes: app.scene.settings.axes_visible,
                    style: app.scene.settings.axis_style,
                    plane_marks: app.scene.settings.plane_marks,
                },
                items,
            };
            let prepared = render::prepare_frame(&request);
            render::render_prepared(&request, &prepared).to_color_image()
        };
        match &mut app.pattern_preview_texture {
            Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
            None => {
                app.pattern_preview_texture =
                    Some(ui.ctx().load_texture("pattern-preview", image, egui::TextureOptions::LINEAR));
            }
        }
        app.pattern_preview_key = key;
    }
    if let Some(texture) = &app.pattern_preview_texture {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
}

/// Everything the preview's image is drawn from. Unchanged, and the last one is
/// still what should be on screen.
fn preview_key(app: &App, size: [usize; 2], dark: bool) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    size.hash(&mut hasher);
    dark.hash(&mut hasher);
    app.evaluation_generation.hash(&mut hasher);
    app.renderable_key.hash(&mut hasher);
    (app.settings.display_mode as u8).hash(&mut hasher);
    app.scene.settings.grid_visible.hash(&mut hasher);
    app.scene.settings.grid_spacing.to_bits().hash(&mut hasher);
    app.scene.settings.axes_visible.hash(&mut hasher);
    app.scene.settings.axis_style.hash(&mut hasher);
    app.scene.settings.plane_marks.hash(&mut hasher);
    let camera = app.pattern_preview_camera;
    for value in
        [camera.target.x, camera.target.y, camera.target.z, camera.distance, camera.yaw, camera.pitch, camera.fov_deg]
    {
        value.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

/// The dialog's buttons: name the rule and keep it, or close.
pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    // The action row lays itself out from the right, so these are added in the
    // order they are read backwards: Close ends up against the right edge and
    // the name field's label ends up in front of the field it names.
    let named = !simple3d_core::library::sanitise(&app.pattern_tool_name).is_empty();
    if ui::dialog_button(ui, "Close", true).clicked() {
        app.modal = Modal::None;
    }
    if ui::dialog_button(ui, "Save kind", named).clicked() {
        app.save_current_kind();
    }
    // What is left of the row once the two buttons have taken theirs, less the
    // width of the word in front of the field. A fixed 180 was wider than a
    // narrow window has to give, and the name field is the one thing here that
    // can honestly be any width at all.
    let field = (ui.available_width() - 52.0).clamp(60.0, 220.0);
    ui.add(egui::TextEdit::singleline(&mut app.pattern_tool_name).desired_width(field).hint_text("Bolt ring"));
    // The word in front of the field is the first thing to go when the row runs
    // out: the field's own hint already says what belongs in it, and a label half
    // off the edge of the window says less than no label at all.
    if ui.available_width() >= 44.0 {
        ui.add(egui::Label::new("Name").selectable(false));
    }
}
