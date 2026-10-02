//! The text each field holds while being edited.

use super::*;
use std::collections::{HashMap, HashSet};

/// Per-field in-progress text while focused, so a half-typed `1.` is not parsed, keyed by widget
/// id. Also tracks fields given unreadable input, which keep their text and show a red frame.
#[derive(Default)]
pub struct FieldBuffers {
    pub(super) buffers: HashMap<egui::Id, String>,
    pub(super) errors: HashSet<egui::Id>,
    /// Fields the user has clicked into; others are drawn as their value and carry the scrub
    /// gesture (see `scrub_field`).
    pub(super) editing: HashSet<egui::Id>,
    /// Fields opened last frame, to be focused once their text field exists.
    pub(super) opening: HashSet<egui::Id>,
    /// The value fields in drawing order, this pass and the last, for Tab to walk.
    pub(super) drawn: Vec<egui::Id>,
    pub(super) drawn_before: Vec<egui::Id>,
    pub(super) pass: u64,
    /// The field Tab (or Shift+Tab, `true`) was just pressed in.
    pub(super) tabbed: Option<(egui::Id, bool)>,
}

/// What a scrub gesture did this frame.
#[derive(Clone, Copy, Debug)]
pub struct Scrubbed {
    /// True on the drag's first frame, the only one that snapshots undo.
    pub started: bool,
    /// Change to apply, in the unit the field displays.
    pub delta: f64,
}

/// One frame of a value field: committed text and/or an ongoing scrub, never both.
#[derive(Default)]
pub struct Field {
    pub committed: Option<String>,
    pub scrubbed: Option<Scrubbed>,
}

impl FieldBuffers {
    /// A value field that is its own slider.
    ///
    /// Until clicked it is a drawing of its value that carries the drag; clicking turns it into a
    /// focused text field. This replaced drag grips on labels and axis chips. `grip` names the
    /// gesture after the value, so a mid-drag relayout cannot hand it to another field.
    pub fn scrub_field(
        &mut self,
        ui: &mut egui::Ui,
        id: egui::Id,
        grip: egui::Id,
        current: &str,
        step: f64,
        scrub: &mut Scrub,
    ) -> Field {
        self.drawn_in_order(ui, id);
        let typing = self.editing.contains(&id) || ui.memory(|memory| memory.has_focus(id));
        if typing {
            let opening = self.opening.remove(&id);
            if opening {
                // Focused with the whole value selected before it is drawn, so a key typed this frame
                // (straight after a Tab) replaces the value.
                ui.memory_mut(|memory| memory.request_focus(id));
                select_whole_value(ui, id, self.buffers.get(&id).map_or(current, String::as_str));
            }
            let committed = self.field(ui, id, current);
            if !opening && (committed.is_some() || !ui.memory(|memory| memory.has_focus(id))) {
                self.editing.remove(&id);
            }
            if let Some((_, backwards)) = self.tabbed.take_if(|&mut (tabbed, _)| tabbed == id) {
                self.open_neighbour(id, backwards);
            }
            return Field { committed, scrubbed: None };
        }

        let rejected = self.is_rejected(id);
        let text = self.buffers.get(&id).cloned().unwrap_or_else(|| current.to_string());
        let response = self.value_box(ui, grip, &text, rejected);
        if response.clicked() {
            // Becomes the text field, focused, next frame.
            self.editing.insert(id);
            self.opening.insert(id);
        }
        Field { committed: None, scrubbed: scrub_gesture(ui, &response, scrub, step) }
    }

    /// The field when not being typed into: the same box and figures, with a resize cursor. It
    /// highlights on hover and drag for feedback; the pressed text colour once made it unreadable.
    pub(super) fn value_box(&self, ui: &mut egui::Ui, grip: egui::Id, text: &str, rejected: bool) -> egui::Response {
        let height = crate::theme::metric::INPUT_ROW;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
        let response = ui.interact(rect, grip, egui::Sense::click_and_drag());
        let visuals = ui.style().interact(&response);
        let (fill, stroke, text_colour) = if rejected {
            // Never the pressed colour on the danger tint, which would hide the number to correct.
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
        // A number wider than its box is shrunk to fit rather than drawn over the axis chip.
        let room = (rect.width() - 8.0).max(1.0);
        let mut size = crate::theme::font::VALUE;
        let width = painter.layout_no_wrap(text.to_string(), egui::FontId::monospace(size), text_colour).size().x;
        if width > room {
            size = (size * room / width).max(size * 0.7);
        }
        painter.with_clip_rect(rect.intersect(painter.clip_rect())).text(
            egui::pos2(rect.right() - 4.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            text,
            egui::FontId::monospace(size),
            text_colour,
        );
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        // Painted by hand, so it declares itself as a `DragValue` with its value for accessibility.
        let enabled = ui.is_enabled();
        let shown = text.to_string();
        response.widget_info(|| egui::WidgetInfo {
            enabled,
            current_text_value: Some(shown.clone()),
            ..egui::WidgetInfo::new(egui::WidgetType::DragValue)
        });
        response
    }

    /// A single-line field: returns the committed text on Enter or blur, `None` while typing. When
    /// not being edited it shows the model's value, so edits flow both ways.
    pub fn field(&mut self, ui: &mut egui::Ui, id: egui::Id, current: &str) -> Option<String> {
        let mut text = self.buffers.get(&id).cloned().unwrap_or_else(|| current.to_string());
        let rejected = self.is_rejected(id);
        let response = ui
            .scope(|ui| {
                if rejected {
                    // The refusal is the field's own frame, so it cannot be missed.
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
                        // Tab is ours, not egui's: it moved focus to the next widget, which was a value
                        // box rather than a text field, so the keys typed next reached the keymap.
                        .lock_focus(true)
                        .desired_width(f32::INFINITY)
                        // Tabular figures, so columns line up and digits do not shift during a scrub.
                        .font(egui::TextStyle::Monospace)
                        .horizontal_align(egui::Align::RIGHT),
                )
            })
            .inner;
        // Tab commits like Enter and hands the keyboard on (`FieldBuffers::open_neighbour`).
        // Only in a field focused before this frame, or the Tab that opened it would pass it on again.
        let tab = response.has_focus()
            && ui.memory(|memory| memory.had_focus_last_frame(id))
            && ui.input(|i| i.key_pressed(egui::Key::Tab));
        if tab {
            self.tabbed = Some((id, ui.input(|i| i.modifiers.shift)));
            ui.memory_mut(|memory| memory.surrender_focus(id));
        }
        let entered = tab || response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        // Escape abandons the edit and keeps the model's value; egui drops focus on Escape, which would
        // otherwise commit the text.
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
