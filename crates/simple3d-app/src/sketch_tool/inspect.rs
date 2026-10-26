//! The popup beside a clicked curve, point or constraint: its sizes and places typed or dragged,
//! its switches, and Delete, so one thing is changed where it is drawn rather than in the panel.

use super::*;
use crate::app::{App, Status};
use crate::popup::{PopupEvent, PopupSpec};
use crate::ui;
use simple3d_sketch::ConstraintKind;

/// The popup's key in the placement store.
pub(crate) const INSPECT_KEY: &str = "sketch-inspect";
/// Its width: a name column and a field with a lock beside it.
const INSPECT_WIDTH: f32 = 260.0;
/// How far from the click it opens, so it does not cover what was clicked.
const OFFSET: egui::Vec2 = egui::vec2(18.0, 14.0);

/// Draw the popup while something is inspected.
pub(crate) fn show_inspect(app: &mut App, ctx: &egui::Context) {
    let Some(tool) = app.sketch_tool.as_mut() else { return };
    let Some(pick) = tool.inspect else { return };
    if let Some(at) = tool.inspect_at.take() {
        let mut placement = crate::popup::Placement::default();
        placement.pos = Some(at + OFFSET);
        app.popups.insert(INSPECT_KEY, placement);
    }
    let title = title(&tool.sketch, pick);
    let spec = PopupSpec { key: INSPECT_KEY, title: &title, width: INSPECT_WIDTH };
    if app.tool_popup(ctx, spec, body, actions) == PopupEvent::Closed {
        if let Some(tool) = app.sketch_tool.as_mut() {
            tool.inspect = None;
        }
    }
}

/// What the popup is about, as its title.
fn title(sketch: &Sketch, pick: Pick) -> String {
    match pick {
        Pick::Point(id) => match sketch.point(id) {
            Some(point) if point.fixed => "Copied point".into(),
            _ => "Point".into(),
        },
        Pick::Curve(id) => match sketch.curve(id) {
            Some(curve) => {
                let label = curve.shape.label();
                let word = if curve.reference {
                    format!("Copied {label}")
                } else if curve.construction {
                    format!("Helper {label}")
                } else {
                    let mut chars = label.chars();
                    chars.next().map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
                };
                word
            }
            None => String::new(),
        },
        Pick::Constraint(id) => sketch.constraint(id).map_or(String::new(), |c| c.kind.label().to_string()),
    }
}

/// The tool is lifted out of the app while drawing, since the fields also borrow app state.
fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut tool) = app.sketch_tool.take() else { return };
    if let Some(pick) = tool.inspect {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
        let message = match pick {
            Pick::Curve(id) => super::inspect_fields::curve(app, ui, &mut tool, id),
            Pick::Point(id) => super::inspect_fields::point(app, ui, &mut tool, id),
            Pick::Constraint(id) => super::inspect_fields::constraint(app, ui, &mut tool, id),
        };
        if let Some(message) = message {
            app.status = message;
        }
    }
    app.sketch_tool = Some(tool);
}

fn actions(app: &mut App, ui: &mut egui::Ui) {
    let Some(tool) = app.sketch_tool.as_mut() else { return };
    if ui::dialog_button(ui, "Done", true).clicked() {
        tool.inspect = None;
        return;
    }
    if ui::dialog_button(ui, "Delete", true).clicked() {
        if let Some(pick) = tool.inspect.take() {
            tool.selection = vec![pick];
            tool.delete_selection();
            app.status = Status::Info("Deleted".into());
        }
    }
}

impl SketchTool {
    /// The first constraint `holds` matches.
    pub(crate) fn holds(&self, holds: impl Fn(&ConstraintKind) -> bool) -> Option<ConstraintId> {
        self.sketch.constraints.iter().find(|c| holds(&c.kind)).map(|c| c.id)
    }

    /// Switch a geometric constraint on or off: off when `existing` names it, else `kind` added.
    pub(crate) fn toggle(&mut self, existing: Option<ConstraintId>, kind: ConstraintKind) -> Option<Status> {
        match existing {
            Some(id) => {
                self.change(|sketch| sketch.remove_constraint(id));
                None
            }
            None => {
                let before = self.sketch.clone();
                match self.sketch.constrain(kind) {
                    Ok(_) => {
                        self.remember(before);
                        self.refresh();
                        None
                    }
                    Err(refusal) => Some(Status::Warning(refusal.message())),
                }
            }
        }
    }

    /// Hold every point of `points` with a fix, or free them all if they all are held.
    pub(crate) fn toggle_fix(&mut self, points: &[PointId]) {
        let fixes: Vec<ConstraintId> = self
            .sketch
            .constraints
            .iter()
            .filter(|c| matches!(c.kind, ConstraintKind::Fix { point, .. } if points.contains(&point)))
            .map(|c| c.id)
            .collect();
        let held =
            |p: &PointId| fixes.iter().any(|&f| self.sketch.constraint(f).is_some_and(|c| c.kind.points() == [*p]));
        let all = points.iter().all(held);
        let free: Vec<PointId> = points.iter().copied().filter(|p| !held(p)).collect();
        self.change(|sketch| {
            if all {
                fixes.iter().for_each(|&id| sketch.remove_constraint(id));
            } else {
                for point in free {
                    let at = sketch.at(point);
                    sketch.push_constraint(ConstraintKind::Fix { point, at });
                }
            }
        });
    }

    /// Whether every point of `points` is held by a fix.
    pub(crate) fn all_fixed(&self, points: &[PointId]) -> bool {
        !points.is_empty()
            && points.iter().all(|&p| {
                self.sketch.point(p).is_some_and(|p| p.fixed)
                    || self
                        .sketch
                        .constraints
                        .iter()
                        .any(|c| matches!(c.kind, ConstraintKind::Fix { point, .. } if point == p))
            })
    }

    /// Move points to `moves` as an edit of popup field `field`; why not, when the constraints hold
    /// them where they are.
    pub(crate) fn move_points(&mut self, field: &str, moves: &[(PointId, [f64; 2])]) -> Option<Status> {
        let before = self.sketch.clone();
        if !self.sketch.drag(moves) {
            return Some(Status::Warning("The constraints hold it where it is".into()));
        }
        if self.sketch != before {
            self.remember_field(before, field);
            self.refresh();
        }
        None
    }

    /// Set dimension `id` to `value` as an edit of popup field `field`.
    pub(crate) fn retype(&mut self, field: &str, id: ConstraintId, value: f64) -> Option<Status> {
        let before = self.sketch.clone();
        match self.sketch.set_value(id, value) {
            Ok(()) => {
                self.remember_field(before, field);
                self.refresh();
                None
            }
            Err(refusal) => Some(Status::Warning(refusal.message())),
        }
    }
}
