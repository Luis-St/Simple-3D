//! The resize and scale tools' in-place popup (issue 120): per axis, the size to type, and for every
//! axis at once, what stays put while a handle is pulled. It lives here beside the tool rather than in
//! the document settings.

use crate::app::{App, Status};
use crate::gizmo::{Drag, Fixed, Handle, Mode, MIN_EXTENT};
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::{theme, ui};
use simple3d_core::config::SizeAnchor;
use simple3d_core::primitive::ParamKind;
use simple3d_core::scene::NodeId;

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "size-tool";

/// The window width: an axis chip and a length field, under the anchor chips.
const WIDTH: f32 = 260.0;

/// A size field's width: enough for a length with its unit.
const FIELD_WIDTH: f32 = 90.0;

/// The popup over the viewport while resize or scale is the tool and something is selected.
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if !matches!(app.mode, Mode::Resize | Mode::Scale) || app.size_tool_hidden || app.primary().is_none() {
        return;
    }
    let title = if app.mode == Mode::Resize { "Resize" } else { "Scale" };
    let spec = PopupSpec { key: KEY, title, width: WIDTH };
    if app.tool_popup(ctx, spec, body, actions) == PopupEvent::Closed {
        app.size_tool_hidden = true;
    }
}

fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(primary) = app.primary() else { return };
    let gizmo = app.gizmo_for(primary);
    let unit = app.unit();
    ui.horizontal(|ui| {
        popup::label(ui, "Stays put", "What holds still while a handle is pulled, on every axis.");
        for anchor in SizeAnchor::ALL {
            let chosen = app.settings.size_anchor == anchor;
            if theme::choice(ui, chosen, anchor.label()).on_hover_text(anchor.description()).clicked() {
                app.settings.size_anchor = anchor;
            }
        }
    });
    ui.add_space(6.0);
    egui::Grid::new("size-tool").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
        ui.label("");
        popup::label(ui, &format!("Size ({})", unit.suffix()), "The object's extent along its own axis.");
        ui.end_row();
        for axis in 0..3 {
            theme::axis_chip(ui, ui.id().with(("size-axis", axis)), axis);
            let sizeable = gizmo.as_ref().is_some_and(|gizmo| app.mode == Mode::Scale || gizmo.drivers[axis].is_some());
            match gizmo.as_ref().filter(|_| sizeable) {
                Some(gizmo) => {
                    let start = (gizmo.local_hi.get(axis) - gizmo.local_lo.get(axis)) * gizmo.axis_scale[axis];
                    let start = app.shown_size(axis, start);
                    let mut extent = start;
                    let field = ("size-tool-field", &*format!("size-{axis}"));
                    popup::number(app, ui, field, FIELD_WIDTH, ParamKind::Length { min: MIN_EXTENT }, &mut extent);
                    if (extent - start).abs() > 1e-9 {
                        app.set_size(axis, extent);
                    }
                }
                None => {
                    ui.add(egui::Label::new(theme::hint("no dimension")).selectable(false))
                        .on_hover_text("No parameter governs this axis; the scale tool can still stretch it.");
                }
            }
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    let hint = "Hold Ctrl while pulling a face to hold the centre. A typed size grows about the centre.";
    ui.add(egui::Label::new(theme::hint(hint)).selectable(false));
    if app.top_level_selection().len() > 1 {
        let hint = "A typed size applies to every selected object.";
        ui.add(egui::Label::new(theme::hint(hint)).selectable(false));
    }
}

fn actions(app: &mut App, ui: &mut egui::Ui) {
    if ui::dialog_button(ui, "Close", true).clicked() {
        app.size_tool_hidden = true;
    }
}

/// A run of size changes on one axis, measured from where it began like a handle drag: the
/// evaluation lags behind a scrub, and re-measuring each step from it would shift twice.
pub(crate) struct SizeRun {
    axis: usize,
    drags: Vec<Drag>,
    /// What the run last wrote to each node, to tell whether anything else has moved it since.
    wrote: Vec<(NodeId, Written)>,
    /// The extent last asked for, shown until the evaluation catches up.
    last: f64,
}

type Written = (simple3d_geom::Vec3, simple3d_geom::Vec3, Option<simple3d_core::primitive::Params>);

fn written(app: &App, id: NodeId) -> Option<Written> {
    let node = app.scene.get(id)?;
    Some((node.position, node.scale, node.params().cloned()))
}

impl App {
    /// Whether nothing but the run itself has changed its nodes since it last wrote them.
    fn size_run_current(&self, run: &SizeRun, targets: &[NodeId]) -> bool {
        run.wrote.len() == run.drags.len()
            && run.wrote.iter().all(|(id, was)| targets.contains(id) && written(self, *id).as_ref() == Some(was))
    }

    /// The extent the popup shows on `axis`: the one last typed while that run is still current, as
    /// the evaluation may not have measured it yet.
    fn shown_size(&self, axis: usize, measured: f64) -> f64 {
        let targets = self.top_level_selection();
        match self.size_run.as_ref() {
            Some(run) if run.axis == axis && self.size_run_current(run, &targets) => run.last,
            _ => measured,
        }
    }

    /// Give every selected object `extent` world millimetres along its own `axis`, about its centre,
    /// since a typed size pulls no side; one undo step per axis while a field is scrubbed.
    pub(crate) fn set_size(&mut self, axis: usize, extent: f64) {
        let label = if self.mode == Mode::Scale { "Scale" } else { "Resize" };
        let targets: Vec<NodeId> = self.top_level_selection();
        let current = self.size_run.take().filter(|run| run.axis == axis && self.size_run_current(run, &targets));
        let run = match current {
            Some(run) => run,
            None => {
                let drags = targets
                    .iter()
                    .filter_map(|&id| {
                        Drag::carried(&self.scene, &self.gizmo_for(id)?, id, Handle::ResizeFace(axis, true))
                    })
                    .collect();
                SizeRun { axis, drags, wrote: Vec::new(), last: extent }
            }
        };
        self.edit(label, Some(&format!("size:{axis}:{targets:?}")));
        let mut sized = 0;
        for drag in &run.drags {
            let outward = (extent - drag.start_extent(axis)) / Fixed::Centre.growth();
            if drag.size_axis(&mut self.scene, axis, outward, true, true).is_some() {
                sized += 1;
            }
        }
        if sized == 0 {
            self.history.discard_last();
            self.status = Status::Info("Nothing selected can be sized on that axis".into());
            return;
        }
        let wrote = run.drags.iter().filter_map(|drag| Some((drag.node, written(self, drag.node)?))).collect();
        self.size_run = Some(SizeRun { wrote, last: extent, ..run });
        self.fields.clear();
    }
}
