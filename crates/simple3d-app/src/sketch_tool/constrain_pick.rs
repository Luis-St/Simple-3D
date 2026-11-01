//! Choosing a tool, and the clicks of a constraint tool. As in other CAD programs a constraint works
//! both ways round: with what it needs selected already, choosing it puts it straight on; otherwise
//! it becomes the tool, each click picks a point or curve, and it goes on once the picks are enough.

use super::constrain::Rule;
use super::*;
use crate::app::Status;
use crate::view::View;

impl SketchTool {
    /// Choose tool `draw` from its tile or its key. A constraint that fits the selection is put on it
    /// at once, the tool in use staying; otherwise the tool is taken up, keeping the selection only
    /// as far as it can be the start of what the tool picks. What to say, if anything.
    pub(crate) fn choose(&mut self, draw: Draw) -> Option<Status> {
        if let Draw::Constrain(rule) = draw {
            if self.kinds_for(rule).is_some() {
                let done = self.apply(rule);
                self.settings.remember(draw);
                return Some(match done {
                    Ok(_) => {
                        self.selection.clear();
                        Status::Info(format!("Added {}", rule.label().to_lowercase()))
                    }
                    Err(why) => Status::Warning(why),
                });
            }
        }
        self.set_draw(draw);
        let keeps = match draw {
            Draw::Constrain(rule) => self.could_fit(rule),
            Draw::Dimension => self.dimension_could_fit(),
            _ => true,
        };
        if !keeps {
            self.selection.clear();
        }
        self.inspect = None;
        None
    }
}

/// A constraint tool's click: the point or curve under the pointer joins the picks, a second click
/// on one drops it again, and once the picks are enough the constraint goes on.
pub(crate) fn constrain_at(tool: &mut SketchTool, view: &View, cursor: egui::Pos2, rule: Rule) -> Status {
    let Some(pick) = tool.pick_at(view, cursor).filter(|p| !matches!(p, Pick::Constraint(_))) else {
        return Status::Info(rule.hint().into());
    };
    if let Some(at) = tool.selection.iter().position(|p| *p == pick) {
        tool.selection.remove(at);
        return Status::Info(rule.next(tool.counts()).into());
    }
    tool.selection.push(pick);
    if !tool.could_fit(rule) {
        // Not something this rule takes along with the rest: it starts the picks afresh.
        tool.selection = vec![pick];
        if !tool.could_fit(rule) {
            tool.selection.clear();
            return Status::Warning(format!("{} needs {}", rule.label(), rule.needs().to_lowercase()));
        }
    }
    if tool.kinds_for(rule).is_none() {
        return Status::Info(rule.next(tool.counts()).into());
    }
    match tool.apply(rule) {
        Ok(_) => {
            tool.selection.clear();
            Status::Info(format!("Added {}; click on for another", rule.label().to_lowercase()))
        }
        Err(why) => {
            tool.selection.clear();
            Status::Warning(why)
        }
    }
}
