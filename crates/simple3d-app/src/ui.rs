//! Shared interface pieces, with the decision-making parts kept as pure
//! functions so they can be tested without an egui context.

use simple3d_core::keymap::{Chord, Command, Keymap};
use simple3d_core::primitive::{ParamKind, ParamValue};
use simple3d_core::unit::{format_angle, format_length, format_number, parse_entry, parse_entry_plain, Unit};
use std::collections::{HashMap, HashSet};

/// What a text field's content should do to the model when the user commits it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Commit {
    /// A new value to store.
    Value(ParamValue),
    /// Unparseable: keep the previous value and mark the field (spec section 4,
    /// acceptance criterion 14). The text the user typed stays where it is --
    /// clearing it would throw away the very thing they need to correct.
    Revert,
}

/// Interpret typed text for a parameter of the given kind. Out-of-range values
/// are clamped rather than rejected -- the user's intent is clear, and refusing a
/// number they can see in the field is more confusing than adjusting it.
///
/// `current` is what the parameter holds now, in stored terms (millimetres for
/// a length, degrees for an angle). It is needed because the field accepts a
/// *delta*: `+2` means "two more than whatever this is", and with several nodes
/// selected that resolves differently for each of them.
pub fn commit_param(text: &str, kind: ParamKind, unit: Unit, current: f64) -> Commit {
    // A length is the only kind a unit suffix means anything to; an angle typed
    // as "4 cm" would otherwise silently become forty degrees.
    let entry = match kind {
        ParamKind::Length { .. } => parse_entry(text, unit),
        _ => parse_entry_plain(text),
    };
    let Some(entry) = entry else { return Commit::Revert };
    let current_shown = match kind {
        ParamKind::Length { .. } => unit.from_mm(current),
        _ => current,
    };
    Commit::Value(value_from_display(kind, unit, entry.resolve(current_shown)))
}

/// Store a number expressed in the field's own display terms as a value of the
/// given kind, with the same clamping `commit_param` applies. The scrub gesture
/// goes through here rather than through text, so a drag and a typed number
/// cannot disagree about what is in range.
pub fn value_from_display(kind: ParamKind, unit: Unit, shown: f64) -> ParamValue {
    match kind {
        ParamKind::Length { min } => ParamValue::Length(unit.to_mm(shown).max(min)),
        ParamKind::Angle { min, max } => ParamValue::Angle(shown.clamp(min, max)),
        ParamKind::Count { min, max } => {
            if shown < 0.0 {
                ParamValue::Count(min)
            } else {
                ParamValue::Count((shown.round() as u32).clamp(min, max))
            }
        }
        ParamKind::Bool => ParamValue::Bool(shown != 0.0),
        ParamKind::Choice { options } => {
            ParamValue::Choice((shown.max(0.0) as u32).min(options.len().saturating_sub(1) as u32))
        }
    }
}

/// The stored number a parameter holds, for resolving a delta against.
pub fn param_number(value: ParamValue) -> f64 {
    match value {
        ParamValue::Length(mm) => mm,
        ParamValue::Angle(deg) => deg,
        ParamValue::Count(n) => n as f64,
        ParamValue::Choice(n) => n as f64,
        ParamValue::Bool(b) => b as u8 as f64,
    }
}

/// A position component, in stored millimetres. Positions may be negative, so
/// there is no minimum; `current` is what the field holds now, so `+2` and
/// `- 5` adjust it.
pub fn commit_length(text: &str, unit: Unit, current: f64) -> Option<f64> {
    let entry = parse_entry(text, unit)?;
    Some(unit.to_mm(entry.resolve(unit.from_mm(current))))
}

/// A rotation component in degrees. Rotations are free, since a node may be
/// turned any way.
pub fn commit_angle(text: &str, current: f64) -> Option<f64> {
    let entry = parse_entry_plain(text)?;
    Some(entry.resolve(current))
}

/// A scale factor. Unitless, and clamped to something that still produces a
/// solid: a factor of zero flattens a shape into a plane and a negative one
/// turns it inside out, and neither is a thing to hand a slicer.
pub fn commit_factor(text: &str, current: f64) -> Option<f64> {
    let entry = parse_entry_plain(text)?;
    Some(entry.resolve(current).max(simple3d_core::scene::Node::MIN_SCALE))
}

/// The em dash a field shows when the nodes it covers do not agree. Typing over
/// it applies to all of them; leaving it alone leaves each as it was.
pub const MIXED: &str = "\u{2014}";

/// What a field shows for a set of values: the value when they agree, and an em
/// dash when they do not.
pub fn shared_text(mut values: impl Iterator<Item = String>) -> String {
    let Some(first) = values.next() else { return String::new() };
    if values.all(|v| v == first) {
        first
    } else {
        MIXED.to_string()
    }
}

/// How much one horizontal pixel of a label scrub is worth.
///
/// Shift is fine, Ctrl is coarse -- the same two modifiers the manipulator uses
/// for the same two meanings, so there is one thing to learn rather than two.
pub fn scrub_step(step: f64, fine: bool, coarse: bool) -> f64 {
    let factor = match (fine, coarse) {
        (true, _) => 0.1,
        (false, true) => 10.0,
        _ => 1.0,
    };
    step * factor / PIXELS_PER_STEP
}

/// Pixels of drag per one step of the value. Slow enough that a value can be
/// landed on, fast enough that a field can be crossed.
pub const PIXELS_PER_STEP: f64 = 6.0;

/// How a parameter's current value is shown in its field.
pub fn show_param(value: ParamValue, unit: Unit) -> String {
    match value {
        ParamValue::Length(mm) => format_length(mm, unit),
        ParamValue::Angle(deg) => format_angle(deg),
        ParamValue::Count(n) => n.to_string(),
        ParamValue::Choice(n) => n.to_string(),
        ParamValue::Bool(b) => b.to_string(),
    }
}

/// Text fields keep their own in-progress buffer while focused, so a half-typed
/// `1.` is not parsed as it is typed and does not fight the value the model
/// holds. Keyed by widget id.
///
/// It also remembers which fields were last given something unreadable. A
/// rejected field keeps the text that was typed into it and wears a red frame:
/// the value the model holds is untouched, and the thing the user has to
/// correct is still in front of them.
#[derive(Default)]
pub struct FieldBuffers {
    buffers: HashMap<egui::Id, String>,
    errors: HashSet<egui::Id>,
    /// Fields the user has clicked into. A field that is not being typed into is
    /// drawn as its own value and carries the scrub gesture instead, which is
    /// what lets one control be both (see `scrub_field`).
    editing: HashSet<egui::Id>,
    /// Fields opened on the previous frame, which have to be given the keyboard
    /// once the text field they turn into actually exists. Asking for focus on
    /// the frame of the click would name a widget nothing had drawn.
    opening: HashSet<egui::Id>,
}

/// What a scrub gesture did this frame.
#[derive(Clone, Copy, Debug)]
pub struct Scrubbed {
    /// True on the frame the drag began: the one frame that takes an undo
    /// snapshot, so the whole drag collapses into a single step.
    pub started: bool,
    /// Change to apply, in the unit the field displays.
    pub delta: f64,
}

/// What one frame of a value field produced: text the user committed, and the
/// scrub they are in the middle of. Both can be empty; they are never both set.
#[derive(Default)]
pub struct Field {
    pub committed: Option<String>,
    pub scrubbed: Option<Scrubbed>,
}

impl FieldBuffers {
    /// A value field that is also its own slider.
    ///
    /// Every scrubbable value in the application used to hang its drag on
    /// whatever sat *beside* the field -- the label for a dimension, the
    /// coloured axis chip for a position -- because a text field has to keep
    /// click-to-caret and drag-to-select for itself. The result was one gesture
    /// living in two different places depending on the row, and the grip for the
    /// only three-column rows in the panel being a four-pixel chip.
    ///
    /// So the field is not a text field until it is being typed into. Until
    /// then it is a drawing of its own value, in the same box, and it carries
    /// the drag; clicking it turns it into the text field and gives it the
    /// keyboard. `grip` is the id that gesture is remembered by -- named after
    /// the value rather than taken from the layout, so a panel that relays
    /// itself out mid-drag cannot hand the drag to another field.
    pub fn scrub_field(
        &mut self,
        ui: &mut egui::Ui,
        id: egui::Id,
        grip: egui::Id,
        current: &str,
        step: f64,
        scrub: &mut Scrub,
    ) -> Field {
        let typing = self.editing.contains(&id) || ui.memory(|memory| memory.has_focus(id));
        if typing {
            let committed = self.field(ui, id, current);
            if self.opening.remove(&id) {
                // The text field exists now, so it can be given the keyboard --
                // and the value in it can be put under the caret whole.
                ui.memory_mut(|memory| memory.request_focus(id));
                select_whole_value(ui, id, self.buffers.get(&id).map_or(current, String::as_str));
            } else if committed.is_some() || !ui.memory(|memory| memory.has_focus(id)) {
                self.editing.remove(&id);
            }
            return Field { committed, scrubbed: None };
        }

        let rejected = self.is_rejected(id);
        let text = self.buffers.get(&id).cloned().unwrap_or_else(|| current.to_string());
        let response = self.value_box(ui, grip, &text, rejected);
        if response.clicked() {
            // Into the text field, with the keyboard, on the next frame.
            self.editing.insert(id);
            self.opening.insert(id);
        }
        Field { committed: None, scrubbed: scrub_gesture(ui, &response, scrub, step) }
    }

    /// The field as it looks when it is not being typed into: the same box, the
    /// same right-aligned tabular figures, and the resize cursor that says it
    /// can be dragged.
    fn value_box(&self, ui: &mut egui::Ui, grip: egui::Id, text: &str, rejected: bool) -> egui::Response {
        let height = crate::theme::metric::INPUT_ROW;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
        let response = ui.interact(rect, grip, egui::Sense::click_and_drag());
        let visuals = ui.style().interact(&response);
        let (fill, stroke) = if rejected {
            (crate::theme::token::DANGER.gamma_multiply(0.16), egui::Stroke::new(1.0_f32, crate::theme::token::DANGER))
        } else {
            (ui.visuals().extreme_bg_color, visuals.bg_stroke)
        };
        let painter = ui.painter();
        painter.rect(rect, visuals.corner_radius, fill, stroke, egui::StrokeKind::Inside);
        painter.text(
            egui::pos2(rect.right() - 4.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            text,
            egui::FontId::monospace(crate::theme::font::VALUE),
            visuals.text_color(),
        );
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        response
    }

    /// Draw a single-line field. Returns the committed text when the user
    /// presses Enter or leaves the field, and `None` while they are still typing.
    ///
    /// The value shown comes from the model whenever the field is not being
    /// edited, which is what makes a manipulator drag update the property editor
    /// live and typing move the handles -- one source of truth, both directions.
    pub fn field(&mut self, ui: &mut egui::Ui, id: egui::Id, current: &str) -> Option<String> {
        let mut text = self.buffers.get(&id).cloned().unwrap_or_else(|| current.to_string());
        let rejected = self.is_rejected(id);
        let response = ui
            .scope(|ui| {
                if rejected {
                    // The mark is the field's own frame, so it is impossible to
                    // read the number without also reading that it was refused.
                    let visuals = ui.visuals_mut();
                    visuals.extreme_bg_color = crate::theme::token::DANGER.gamma_multiply(0.16);
                    let danger = egui::Stroke::new(1.0_f32, crate::theme::token::DANGER);
                    visuals.widgets.inactive.bg_stroke = danger;
                    visuals.widgets.hovered.bg_stroke = danger;
                    visuals.widgets.active.bg_stroke = danger;
                    visuals.selection.stroke = danger;
                }
                ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .id(id)
                        .desired_width(f32::INFINITY)
                        // Tabular figures: a column of dimensions has to line up,
                        // and no digit may change width while a value is scrubbed.
                        .font(egui::TextStyle::Monospace)
                        .horizontal_align(egui::Align::RIGHT),
                )
            })
            .inner;
        let entered = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        // Escape is the way out of everything else in this application, and it
        // is the way out of a half-typed value too: the buffer is dropped and
        // the model keeps what it had. egui surrenders focus on Escape, so
        // without this the abandoned text would be committed on the way out.
        let abandoned = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
        if abandoned {
            self.buffers.remove(&id);
            self.errors.remove(&id);
            return None;
        }
        if response.has_focus() && !entered {
            self.buffers.insert(id, text);
            return None;
        }
        if response.lost_focus() || entered {
            let committed = self.buffers.remove(&id).unwrap_or(text);
            if committed != current {
                return Some(committed);
            }
            self.errors.remove(&id);
            return None;
        }
        if response.changed() {
            self.buffers.insert(id, text);
        }
        None
    }

    /// The field took the value: drop any mark it was wearing.
    pub fn accept(&mut self, id: egui::Id) {
        self.errors.remove(&id);
    }

    /// The field's text could not be read. Put it back and mark it -- never
    /// clear it, and never touch the model.
    pub fn reject(&mut self, id: egui::Id, text: String) {
        self.buffers.insert(id, text);
        self.errors.insert(id);
    }

    /// Whether a field is currently wearing the mark.
    pub fn is_rejected(&self, id: egui::Id) -> bool {
        self.errors.contains(&id)
    }

    /// Forget every in-progress edit, for when the selection changes underneath.
    pub fn clear(&mut self) {
        self.buffers.clear();
        self.errors.clear();
        self.editing.clear();
        self.opening.clear();
    }
}

/// Put the caret across the whole of a value field that has just been opened, so
/// that what is typed next replaces the number rather than being added to the end
/// of it.
///
/// Clicking a field that read 20 and typing 12 gave 2012, and on a count with a
/// maximum it gave whatever the maximum was -- 7 and "12" came out as 512. That
/// is not what clicking a number and typing means anywhere, and it is not what
/// the field asks for either: it is opened by a *click*, one gesture on the value
/// as a whole, and never by a caret placed anywhere in particular.
///
/// The state exists by now because the text field has already been drawn this
/// frame; if it somehow has not, the field opens with the caret at the end, which
/// is what it did before.
fn select_whole_value(ui: &egui::Ui, id: egui::Id, text: &str) {
    let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) else { return };
    let whole =
        egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
    state.cursor.set_char_range(Some(whole));
    egui::TextEdit::store_state(ui.ctx(), id, state);
}

/// A drag on a field's *label*, which scrubs the value.
///
/// The whole gesture is one undo step: the snapshot is taken when the drag
/// starts, and every frame after it only touches the scene. Forty snapshots for
/// one drag would make undo useless exactly where it is needed most. The id is
/// held so a pointer that runs off one label and over another cannot hand the
/// gesture to a field the user never grabbed.
#[derive(Clone, Copy, Debug, Default)]
pub struct Scrub {
    pub id: Option<egui::Id>,
    /// The part of the drag a whole-numbered field could not take yet.
    ///
    /// A count is stored as an integer and read back out of the model on every
    /// frame, so the fraction of a copy each frame of the drag is worth was
    /// rounded away sixty times a second rather than added up. A hand moving a
    /// pixel a frame rounded to nothing and the field never moved at all; a hand
    /// moving four rounded *up* on every frame and the field ran away from the
    /// pointer. Kept here instead, and spent on the frame it comes to a whole
    /// one, which is what makes a count follow the pointer at the same six
    /// pixels a step every other field does.
    pub carry: f64,
}

/// One frame of a scrub on a value box: which field owns the gesture, and how
/// far it has moved.
///
/// The id is held for the length of the drag so a pointer that runs off one
/// field and over another cannot hand the gesture to a field the user never
/// grabbed -- and any real edit does run off, six pixels to the millimetre.
pub fn scrub_gesture(ui: &mut egui::Ui, response: &egui::Response, scrub: &mut Scrub, step: f64) -> Option<Scrubbed> {
    let id = response.id;
    if response.drag_started() {
        scrub.id = Some(id);
        // Nothing is owed at the start of a gesture: a fraction left over from
        // the last one would be spent on this field's first frame.
        scrub.carry = 0.0;
    }
    if scrub.id != Some(id) {
        return None;
    }
    if !response.dragged() {
        scrub.id = None;
        scrub.carry = 0.0;
        return None;
    }
    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    let (fine, coarse) = ui.input(|i| (i.modifiers.shift, i.modifiers.command));
    Some(Scrubbed { started: response.drag_started(), delta: scrub_delta(response.drag_delta().x, step, fine, coarse) })
}

/// This frame's pointer movement as a change in the field's own units.
pub fn scrub_delta(dx: f32, step: f64, fine: bool, coarse: bool) -> f64 {
    dx as f64 * scrub_step(step, fine, coarse)
}

/// The scrub increment for a field of a given kind, in the unit the field
/// shows. One millimetre in a millimetre document, one degree, one segment.
pub fn scrub_increment(kind: ParamKind, unit: Unit) -> f64 {
    match kind {
        ParamKind::Length { .. } => unit.from_mm(1.0),
        ParamKind::Angle { .. } => 1.0,
        _ => 1.0,
    }
}

/// The label to put on a menu entry: the command's name plus its *current*
/// binding, never a hardcoded one (spec section 8.2). The two halves are kept
/// apart by a tab, and `menu_entry` is what draws them.
pub fn menu_label(keymap: &Keymap, command: Command) -> String {
    match keymap.binding(command) {
        Some(chord) => format!("{}\t{}", command.label(), chord),
        None => command.label().to_string(),
    }
}

/// Split a menu label into the action and its key binding.
pub fn split_menu_label(label: &str) -> (&str, &str) {
    match label.split_once('\t') {
        Some((action, shortcut)) => (action, shortcut.trim()),
        None => (label, ""),
    }
}

/// One menu entry: the action on the left, its key binding boxed at the right
/// edge of the menu.
///
/// A tab between the two put them one word apart, wherever that happened to
/// land, so a column of entries had its bindings scattered down the middle and
/// nothing said which text was a key and which was the name of the command.
/// The binding is now pushed to the right by a growing spacer -- so every
/// entry's binding lines up with every other's -- and drawn inside a keycap
/// outline, so it reads as a key rather than as more of the sentence.
pub fn menu_entry(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let (action, shortcut) = split_menu_label(label);
    let font = egui::FontId::monospace(crate::theme::font::SMALL);
    let mut button = egui::Button::new(action);
    if !shortcut.is_empty() {
        button = button.shortcut_text(egui::RichText::new(shortcut).font(font.clone()));
    }
    let response = ui.add_enabled(enabled, button);
    if !shortcut.is_empty() {
        // The outline is painted after the button, so it can only ever be a
        // stroke: a filled box here would cover the text already drawn under it.
        let width = ui.fonts(|fonts| fonts.layout_no_wrap(shortcut.to_string(), font, egui::Color32::WHITE).size().x);
        let right = response.rect.right() - ui.spacing().button_padding.x;
        let box_rect = egui::Rect::from_center_size(
            egui::pos2(right - width / 2.0, response.rect.center().y),
            egui::vec2(width, crate::theme::font::SMALL + 2.0),
        )
        .expand2(egui::vec2(4.0, 2.0));
        let colour = if enabled { crate::theme::token::SURFACE_3 } else { crate::theme::token::SURFACE_2 };
        ui.painter().rect_stroke(box_rect, 3.0, egui::Stroke::new(1.0_f32, colour), egui::StrokeKind::Inside);
    }
    response
}

/// Translate an egui key press into the chord form the keymap stores. Uses
/// egui's own key names, so there is no translation table to drift out of date.
/// A button in a dialog's action row: every one the same size, so a row of them
/// is a row and not a ragged line (issue 63).
pub fn dialog_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let size = egui::vec2(crate::theme::metric::DIALOG_BUTTON_WIDTH, crate::theme::metric::DIALOG_BUTTON);
    ui.add_enabled(enabled, egui::Button::new(label).min_size(size))
}

/// The toolkit key a name stands for, the inverse of `Key::name`. Used to ask
/// whether a bound key is being held right now -- for the snap-while-held mode
/// (issue 68), where a binding is a key to hold rather than a press to react to.
pub fn key_from_name(name: &str) -> Option<egui::Key> {
    egui::Key::ALL.iter().copied().find(|k| k.name() == name)
}

/// Watches what is being held so that whatever a hand holds down together can be
/// a binding: a modifier on its own (issue 77), an ordinary combination like
/// `Q+W+E`, or the two mixed.
///
/// The toolkit never reports Ctrl, Shift or Alt as key events -- they only ever
/// arrive as the modifier state of some *other* key -- so a modifier press has
/// to be recognised from that state changing. Ordinary keys have the same
/// problem in reverse: a press cannot be told from the start of a combination
/// until the hand comes off. So one rule covers both, and it is the one a hand
/// already performs: everything held down together is the chord, and the chord
/// is complete when the last of it is released.
///
/// The widest set held during one press is what comes out, so pressing Q, adding
/// W and E and letting all three go is `Q+W+E` rather than whichever happened to
/// be released last.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChordHold {
    held: Option<Chord>,
    /// The pointer was used while this was down, so the hold is part of a mouse
    /// gesture and not a binding of its own.
    interrupted: bool,
}

impl ChordHold {
    /// Feed one frame's state. Returns the chord if this frame completed one:
    /// everything is up again and the pointer was not used along the way.
    ///
    /// `keys_down` is every ordinary key currently held, and `interrupted` is a
    /// mouse button being used -- Ctrl+click picks a second object and must not
    /// also fire what Ctrl alone is bound to.
    pub fn update<'a>(
        &mut self,
        modifiers: egui::Modifiers,
        keys_down: impl IntoIterator<Item = &'a str>,
        interrupted: bool,
    ) -> Option<Chord> {
        let mut now = Chord {
            keys: keys_down.into_iter().map(str::to_string).collect(),
            ctrl: modifiers.command,
            shift: modifiers.shift,
            alt: modifiers.alt,
        };
        if now.keys.is_empty() && !(now.ctrl || now.shift || now.alt) {
            let released = self.held.take();
            let clean = !self.interrupted;
            self.interrupted = false;
            return released.filter(|_| clean);
        }
        if let Some(held) = &self.held {
            now.keys.extend(held.keys.iter().cloned());
            now.ctrl |= held.ctrl;
            now.shift |= held.shift;
            now.alt |= held.alt;
        }
        now.keys.sort();
        now.keys.dedup();
        self.held = Some(now);
        self.interrupted |= interrupted;
        None
    }

    /// Forget a hold in progress: the keyboard has gone somewhere else -- a
    /// dialog, a text field -- and the release will never be seen here.
    pub fn reset(&mut self) {
        *self = ChordHold::default();
    }
}

/// Every ordinary key held down right now, by the name the keymap stores.
pub fn keys_down(input: &egui::InputState) -> Vec<String> {
    let mut names: Vec<String> = input.keys_down.iter().map(|k| k.name().to_string()).collect();
    names.sort();
    names
}

/// A short description of a bounding box, for the overlay that answers "will
/// this fit" (spec section 6.1).
pub fn describe_size(size: simple3d_geom::Vec3, unit: Unit) -> String {
    format!(
        "{} x {} x {} {}",
        format_length(size.x, unit),
        format_length(size.y, unit),
        format_length(size.z, unit),
        unit.suffix()
    )
}

/// A point, in the display unit, for a message that names one.
pub fn describe_point(p: simple3d_geom::Vec3, unit: Unit) -> String {
    format!(
        "{}, {}, {} {}",
        format_length(p.x, unit),
        format_length(p.y, unit),
        format_length(p.z, unit),
        unit.suffix()
    )
}

/// The status bar's triangle and node counts. `nodes` is what the document
/// holds -- the scene itself is not one of them, and counting it made an empty
/// document report one node with nothing in it.
pub fn describe_counts(nodes: usize, triangles: usize) -> String {
    format!("{nodes} node{}  {triangles} triangle{}", plural(nodes), plural(triangles))
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Format a duration for the status bar without pretending to more precision
/// than is meaningful.
pub fn describe_elapsed(elapsed: std::time::Duration) -> String {
    let millis = elapsed.as_secs_f64() * 1000.0;
    // Rounded to whole milliseconds, an evaluation that takes a fifth of one --
    // which most of them do -- reads as "0 ms", and a readout that says zero
    // reads as a readout that is not working. Two decimals under 10 ms, one
    // under 100, none above: always three significant figures of something
    // that actually happened.
    if millis < 10.0 {
        format!("{} ms", format_number(millis, 2))
    } else if millis < 100.0 {
        format!("{} ms", format_number(millis, 1))
    } else if millis < 1000.0 {
        format!("{} ms", format_number(millis, 0))
    } else {
        format!("{} s", format_number(millis / 1000.0, 2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_length_is_read_in_the_display_unit() {
        assert_eq!(
            commit_param("1.8", ParamKind::Length { min: 0.0 }, Unit::Metre, 0.0),
            Commit::Value(ParamValue::Length(1800.0))
        );
        assert_eq!(
            commit_param("1,8", ParamKind::Length { min: 0.0 }, Unit::Millimetre, 0.0),
            Commit::Value(ParamValue::Length(1.8))
        );
    }

    #[test]
    fn garbage_reverts_rather_than_raising_a_dialog() {
        // Spec acceptance criterion 14.
        for bad in ["", "  ", "abc", "1.2.3", "--4", "NaN", "inf", "12mmm"] {
            assert_eq!(
                commit_param(bad, ParamKind::Length { min: 0.0 }, Unit::Millimetre, 10.0),
                Commit::Revert,
                "{bad:?}"
            );
            assert_eq!(commit_length(bad, Unit::Millimetre, 10.0), None, "{bad:?}");
            assert_eq!(commit_angle(bad, 10.0), None, "{bad:?}");
        }
    }

    #[test]
    fn a_dimension_below_its_minimum_is_clamped_not_rejected() {
        let kind = ParamKind::Length { min: 1e-3 };
        assert_eq!(commit_param("0", kind, Unit::Millimetre, 4.0), Commit::Value(ParamValue::Length(1e-3)));
        assert_eq!(commit_param("-5", kind, Unit::Millimetre, 4.0), Commit::Value(ParamValue::Length(1e-3)));
        assert_eq!(commit_param("5", kind, Unit::Millimetre, 4.0), Commit::Value(ParamValue::Length(5.0)));
    }

    #[test]
    fn a_count_is_rounded_and_clamped_to_its_range() {
        let kind = ParamKind::Count { min: 3, max: 128 };
        assert_eq!(commit_param("6", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(6)));
        assert_eq!(commit_param("6.7", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(7)));
        assert_eq!(commit_param("1", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(3)));
        assert_eq!(commit_param("999", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(128)));
        assert_eq!(commit_param("-4", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(3)));
    }

    #[test]
    fn an_angle_is_clamped_to_its_range_and_stays_in_degrees() {
        let kind = ParamKind::Angle { min: 1.0, max: 360.0 };
        // Angles are always degrees regardless of the length unit.
        assert_eq!(commit_param("90", kind, Unit::Metre, 45.0), Commit::Value(ParamValue::Angle(90.0)));
        assert_eq!(commit_param("999", kind, Unit::Millimetre, 45.0), Commit::Value(ParamValue::Angle(360.0)));
        assert_eq!(commit_param("0", kind, Unit::Millimetre, 45.0), Commit::Value(ParamValue::Angle(1.0)));
    }

    #[test]
    fn a_choice_index_cannot_run_off_the_end() {
        let kind = ParamKind::Choice { options: &["Across corners", "Across flats"] };
        assert_eq!(commit_param("1", kind, Unit::Millimetre, 0.0), Commit::Value(ParamValue::Choice(1)));
        assert_eq!(commit_param("7", kind, Unit::Millimetre, 0.0), Commit::Value(ParamValue::Choice(1)));
        assert_eq!(commit_param("-1", kind, Unit::Millimetre, 0.0), Commit::Value(ParamValue::Choice(0)));
    }

    #[test]
    fn what_a_field_shows_round_trips_back_through_what_it_accepts() {
        for (value, unit) in [
            (ParamValue::Length(40.0), Unit::Millimetre),
            (ParamValue::Length(4.0), Unit::Metre),
            (ParamValue::Length(0.5), Unit::Centimetre),
            (ParamValue::Angle(180.0), Unit::Millimetre),
            (ParamValue::Count(6), Unit::Millimetre),
        ] {
            let shown = show_param(value, unit);
            let kind = match value {
                ParamValue::Length(_) => ParamKind::Length { min: 0.0 },
                ParamValue::Angle(_) => ParamKind::Angle { min: -360.0, max: 360.0 },
                ParamValue::Count(_) => ParamKind::Count { min: 0, max: 1000 },
                ParamValue::Choice(_) => ParamKind::Choice { options: &["a", "b"] },
                ParamValue::Bool(_) => ParamKind::Bool,
            };
            assert_eq!(
                commit_param(&shown, kind, unit, param_number(value)),
                Commit::Value(value),
                "{shown:?} in {unit:?}"
            );
        }
    }

    #[test]
    fn shown_values_never_carry_floating_point_noise() {
        assert_eq!(show_param(ParamValue::Length(4.0), Unit::Metre), "0.004");
        assert_eq!(show_param(ParamValue::Length(0.1 + 0.2), Unit::Millimetre), "0.3");
        assert_eq!(show_param(ParamValue::Length(1800.0), Unit::Metre), "1.8");
    }

    #[test]
    fn menu_labels_show_the_current_binding() {
        let mut keymap = Keymap::default();
        assert_eq!(menu_label(&keymap, Command::Save), "Save\tCtrl+S");
        keymap.set(Command::Save, Chord::key("F9"), true).unwrap();
        assert_eq!(menu_label(&keymap, Command::Save), "Save\tF9");
        keymap.unbind(Command::Save);
        assert_eq!(menu_label(&keymap, Command::Save), "Save");
    }

    #[test]
    fn an_egui_key_press_resolves_to_its_command() {
        // The names the toolkit gives its keys are the names the keymap stores,
        // which is what lets a press be looked up at all.
        let keymap = Keymap::default();
        let press = |key: egui::Key, modifiers: egui::Modifiers| {
            let name = key.name();
            keymap.command_for_press(name, |k| k == name, modifiers.command, modifiers.shift, modifiers.alt)
        };
        assert_eq!(press(egui::Key::S, egui::Modifiers::COMMAND), Some(Command::Save));
        assert_eq!(press(egui::Key::S, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT), Some(Command::SaveAs));
        assert_eq!(press(egui::Key::Delete, egui::Modifiers::NONE), Some(Command::Delete));
        assert_eq!(press(egui::Key::ArrowUp, egui::Modifiers::NONE), Some(Command::NudgeUp));
    }

    /// A frame with nothing held: the state a hold is completed by.
    const UP: [&str; 0] = [];

    #[test]
    fn a_modifier_released_on_its_own_records_as_a_chord() {
        // Issue 77: the toolkit reports no key event for Ctrl, so a modifier
        // press is recognised from the state going down and coming back up with
        // nothing under it.
        let ctrl = egui::Modifiers::COMMAND;
        let none = egui::Modifiers::NONE;
        let mut hold = ChordHold::default();
        assert_eq!(hold.update(ctrl, UP, false), None, "the press alone is not yet a chord");
        assert_eq!(hold.update(ctrl, UP, false), None, "still held");
        assert_eq!(hold.update(none, UP, false), Some(Chord::modifiers(true, false, false)));
        // The hold is spent: an idle frame afterwards fires nothing.
        assert_eq!(hold.update(none, UP, false), None);
    }

    #[test]
    fn a_modifier_held_under_another_key_records_as_the_combination() {
        // Ctrl+S has to come out as Ctrl+S, not as Ctrl and then S.
        let ctrl = egui::Modifiers::COMMAND;
        let none = egui::Modifiers::NONE;
        let mut hold = ChordHold::default();
        hold.update(ctrl, UP, false);
        hold.update(ctrl, ["S"], false);
        assert_eq!(hold.update(none, UP, false), Some(Chord::ctrl("S")));
    }

    #[test]
    fn several_ordinary_keys_held_together_are_one_chord() {
        // Q, then W, then E, all let go: one binding, whatever order they came
        // off in. The keys are a set, so the same three in another order are the
        // same chord and cannot be bound twice.
        let none = egui::Modifiers::NONE;
        let mut hold = ChordHold::default();
        hold.update(none, ["Q"], false);
        hold.update(none, ["Q", "W"], false);
        hold.update(none, ["Q", "W", "E"], false);
        hold.update(none, ["W", "E"], false);
        let chord = hold.update(none, UP, false).expect("the release completes the chord");
        assert_eq!(chord, Chord::combo(["E", "Q", "W"]));
        assert_eq!(chord.to_string(), "E+Q+W");
        assert_eq!(chord, Chord::combo(["W", "E", "Q"]), "the order keys were pressed in made a different chord");
    }

    #[test]
    fn a_hold_over_a_mouse_gesture_is_the_gesture_not_a_chord() {
        // Ctrl+click picks a second object, and must not also fire whatever Ctrl
        // alone is bound to.
        let mut hold = ChordHold::default();
        hold.update(egui::Modifiers::COMMAND, UP, false);
        hold.update(egui::Modifiers::COMMAND, UP, true);
        assert_eq!(hold.update(egui::Modifiers::NONE, UP, false), None, "a Ctrl+click fired the Ctrl binding");
    }

    #[test]
    fn a_hold_the_keyboard_left_mid_way_is_forgotten() {
        let mut hold = ChordHold::default();
        hold.update(egui::Modifiers::ALT, UP, false);
        hold.reset();
        assert_eq!(hold.update(egui::Modifiers::NONE, UP, false), None, "a dropped hold fired on the release");
    }

    #[test]
    fn every_default_binding_can_be_produced_by_a_real_key_press() {
        // A binding nobody can type would be a silent dead end, so check every
        // preset's key names against the toolkit's own key list.
        let names: Vec<&'static str> = egui::Key::ALL.iter().map(|k| k.name()).collect();
        for preset in simple3d_core::keymap::Preset::ALL {
            let keymap = Keymap::from_preset(preset);
            for command in Command::ALL {
                let chord = keymap.binding(*command).unwrap();
                // A chord that is modifiers alone names no key, and is typed by
                // holding and releasing the modifier (issue 77).
                if chord.is_modifier_only() {
                    continue;
                }
                assert!(
                    chord.keys.iter().all(|k| names.contains(&k.as_str())),
                    "{preset:?}: {:?} is bound to {:?}, which is not a key that exists",
                    command,
                    chord.keys
                );
            }
        }
    }

    #[test]
    fn a_bounding_box_reads_as_three_dimensions_in_the_display_unit() {
        let size = simple3d_geom::Vec3::new(40.0, 20.0, 4.0);
        assert_eq!(describe_size(size, Unit::Millimetre), "40 x 20 x 4 mm");
        assert_eq!(describe_size(size, Unit::Metre), "0.04 x 0.02 x 0.004 m");
    }

    #[test]
    fn counts_and_durations_read_naturally() {
        assert_eq!(describe_counts(1, 1), "1 node  1 triangle");
        assert_eq!(describe_counts(3, 240), "3 nodes  240 triangles");
        assert_eq!(describe_elapsed(std::time::Duration::from_millis(12)), "12 ms");
        // Issue 38: the readout used to say "0 ms" for every evaluation that
        // took less than half a millisecond, which is most of them.
        assert_eq!(describe_elapsed(std::time::Duration::from_micros(240)), "0.24 ms");
        assert_eq!(describe_elapsed(std::time::Duration::from_millis(2500)), "2.5 s");
    }
}
