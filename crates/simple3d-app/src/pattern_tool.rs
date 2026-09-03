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
        self.modal = Modal::PatternKind;
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

/// The tool's contents: the shelf of saved kinds down the left, the stages down
/// the middle, and what they currently lay out drawn beside them.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(id) = app.pattern_tool_target() else {
        ui.label("The pattern this was opened on is no longer there.");
        return;
    };
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(170.0);
            shelf(app, ui);
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.set_width(300.0);
            stages(app, ui, id);
        });
        ui.separator();
        ui.vertical(|ui| preview(app, ui));
    });
}

/// The saved kinds. Clicking one puts it on the pattern; the cross beside it
/// takes it off the shelf for good.
fn shelf(app: &mut App, ui: &mut egui::Ui) {
    ui.add(egui::Label::new(theme::header_text("Saved kinds")).selectable(false));
    ui.add_space(4.0);
    if app.pattern_kinds.is_empty() {
        ui.add(
            egui::Label::new(theme::hint("Nothing saved yet. Build a rule beside this and give it a name to keep it."))
                .selectable(false),
        );
    }
    let mut apply = None;
    let mut delete = None;
    let room = ui.available_height();
    egui::ScrollArea::vertical().max_height(room).show(ui, |ui| {
        for entry in &app.pattern_kinds {
            ui.horizontal(|ui| {
                if ui.selectable_label(false, &entry.name).on_hover_text("Put this rule on the pattern").clicked() {
                    apply = Some(entry.clone());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("\u{00d7}").on_hover_text("Delete this saved kind").clicked() {
                        delete = Some(entry.clone());
                    }
                });
            });
        }
    });
    if let Some(entry) = apply {
        app.apply_saved_kind(&entry);
    }
    if let Some(entry) = delete {
        app.delete_saved_kind(&entry);
    }
}

/// The rule itself: how many stages, and each stage's numbers.
fn stages(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let params = app.pattern_tool_params();
    let used = pattern::stage_count(&params);
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(theme::header_text("Stages")).selectable(false));
        ui.add(egui::Label::new(theme::value(format!("{used} of {}", pattern::MAX_STAGES))).selectable(false));
        if ui.small_button("+").on_hover_text("Repeat what the stages so far make").clicked() {
            app.set_stage_count(used + 1);
        }
        if ui.small_button("\u{2212}").on_hover_text("Drop the last stage").clicked() {
            app.set_stage_count(used.saturating_sub(1));
        }
    });
    ui.add(egui::Label::new(theme::hint("Each stage repeats what the ones above it made.")).selectable(false));
    ui.add_space(4.0);

    let unit = app.unit();
    let room = ui.available_height();
    egui::ScrollArea::vertical().max_height(room).id_salt("pattern-stages").show(ui, |ui| {
        for index in 0..used {
            let stage = pattern::stage(&params, index);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(theme::header_text(pattern::STAGES[index].label)).selectable(false));
                ui.add(egui::Label::new(theme::hint(describe(&stage, unit))).selectable(false));
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
    });
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

/// Where the copies land, drawn as one dot each.
///
/// A rule is a handful of numbers and nobody reads a helix out of six of them.
/// The viewport behind the dialog shows the real thing, but it is behind the
/// dialog: this is the picture that is *in front of* the numbers being typed,
/// and it costs one call to the same `instances` the renderer uses.
fn preview(app: &mut App, ui: &mut egui::Ui) {
    let params = app.pattern_tool_params();
    let copies = pattern::instances(&params);
    ui.add(egui::Label::new(theme::header_text("Lays out")).selectable(false));
    ui.add_space(4.0);
    let side = ui.available_width().min(ui.available_height() - 24.0).clamp(160.0, 420.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, theme::token::SURFACE_0);

    // Straight isometric, the view the viewport opens in, so a helix reads as
    // one and a grid does not collapse onto a line.
    let flat = |p: Vec3| egui::vec2(((p.x - p.y) * 0.866) as f32, ((p.x + p.y) * 0.5 - p.z) as f32);
    let points: Vec<egui::Vec2> = copies.iter().map(|c| flat(c.xform.t)).collect();
    let (mut lo, mut hi) = (egui::vec2(0.0, 0.0), egui::vec2(0.0, 0.0));
    for p in &points {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    let span = (hi - lo).max(egui::vec2(1.0, 1.0));
    let scale = ((rect.width() - 32.0) / span.x).min((rect.height() - 32.0) / span.y);
    let centre = (lo + hi) * 0.5;
    for (index, p) in points.iter().enumerate() {
        let at = rect.center() + (*p - centre) * scale;
        // The original is the one every other copy is a copy *of*, so it is the
        // one drawn brightest.
        let colour = if index == 0 { theme::token::ACCENT } else { theme::token::TEXT_LO };
        painter.circle_filled(at, if index == 0 { 4.0 } else { 2.5 }, colour);
    }
    ui.add_space(4.0);
    let (wanted, made) = pattern::instance_count(&params);
    let note = if wanted > made {
        format!("{made} copies -- {wanted} were asked for, which is more than can be drawn")
    } else {
        format!("{made} copies")
    };
    ui.add(egui::Label::new(theme::hint(note)).selectable(false));
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
    ui.add(egui::TextEdit::singleline(&mut app.pattern_tool_name).desired_width(180.0).hint_text("Bolt ring"));
    ui.add(egui::Label::new("Name").selectable(false));
}
