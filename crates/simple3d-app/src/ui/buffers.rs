//! The text each field is holding while it is being edited.

use super::*;
use std::collections::{HashMap, HashSet};

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
    pub(super) buffers: HashMap<egui::Id, String>,
    pub(super) errors: HashSet<egui::Id>,
    /// Fields the user has clicked into. A field that is not being typed into is
    /// drawn as its own value and carries the scrub gesture instead, which is
    /// what lets one control be both (see `scrub_field`).
    pub(super) editing: HashSet<egui::Id>,
    /// Fields opened on the previous frame, which have to be given the keyboard
    /// once the text field they turn into actually exists. Asking for focus on
    /// the frame of the click would name a widget nothing had drawn.
    pub(super) opening: HashSet<egui::Id>,
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
    ///
    /// It answers to the pointer the way every other control in the application
    /// does -- the fill lifts under it and goes to the accent while it is being
    /// dragged -- because a box that never changes gives a drag no feedback at
    /// all, and because taking the *text* colour from the pressed state while
    /// keeping the resting fill wrote the number in near-black on dark grey for
    /// the length of the gesture.
    pub(super) fn value_box(&self, ui: &mut egui::Ui, grip: egui::Id, text: &str, rejected: bool) -> egui::Response {
        let height = crate::theme::metric::INPUT_ROW;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
        let response = ui.interact(rect, grip, egui::Sense::click_and_drag());
        let visuals = ui.style().interact(&response);
        let (fill, stroke, text_colour) = if rejected {
            // The refused value is never written in the pressed state's colour:
            // that one is chosen to sit on the accent, and on the danger tint it
            // would hide the very number the user has to correct.
            (
                crate::theme::token::DANGER.gamma_multiply(0.16),
                egui::Stroke::new(1.0_f32, crate::theme::token::DANGER),
                crate::theme::token::TEXT_HI,
            )
        } else {
            (visuals.weak_bg_fill, visuals.bg_stroke, visuals.text_color())
        };
        let painter = ui.painter();
        painter.rect(rect, visuals.corner_radius, fill, stroke, egui::StrokeKind::Inside);
        painter.text(
            egui::pos2(rect.right() - 4.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            text,
            egui::FontId::monospace(crate::theme::font::VALUE),
            text_colour,
        );
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        // The box is painted rather than assembled out of egui's own widgets, so
        // nothing would otherwise say what it is: to anything reading the
        // interface it was an unnamed rectangle. It is the same control egui's
        // `DragValue` is, so it says so, and reads out the value it is showing.
        let enabled = ui.is_enabled();
        let shown = text.to_string();
        response.widget_info(|| egui::WidgetInfo {
            enabled,
            current_text_value: Some(shown.clone()),
            ..egui::WidgetInfo::new(egui::WidgetType::DragValue)
        });
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
}
