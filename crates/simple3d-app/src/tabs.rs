//! Several documents open at once, one per tab (issue 61).
//!
//! The application still has exactly one *current* document, and every panel,
//! command and gesture goes on reading it straight off `App` as it always did.
//! What tabs add is the documents that are not current: their state is lifted
//! off `App` into a `Document` and put back when the tab is picked again, so
//! nothing in the rest of the application has to know how many are open.
//!
//! The invariant the switching rests on: `App::tabs` has one entry per open
//! document, and the entry at `App::active` is a stand-in whose contents are
//! stale -- the live state of that document is the one on `App` itself. Nothing
//! outside this module reads a `Document` directly; the tab bar asks the
//! helpers below, which know to answer for the active tab from `App`.

use crate::app::{App, Modal, Status};
use crate::theme::{self, metric, token};
use simple3d_core::eval::Evaluated;
use simple3d_core::scene::{NodeId, Scene};
use simple3d_core::undo::History;
use simple3d_geom::Vec3;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// One open document: everything about the model in a tab, and nothing about
/// the window it is shown in. The camera travels inside `scene`, the tool mode,
/// the dock layout and the settings are the application's and stay put when the
/// tab changes.
pub struct Document {
    pub scene: Scene,
    pub history: History,
    pub path: Option<PathBuf>,
    pub saved_revision: u64,
    pub selection: Vec<NodeId>,
    pub selection_anchor: Option<NodeId>,
    pub collapsed: HashSet<NodeId>,
    pub cursor: Option<Vec3>,
    pub frame_when_evaluated: bool,
    /// The last evaluation of this scene, so coming back to a tab shows the
    /// model at once rather than an empty viewport while it is recomputed.
    pub evaluated: Evaluated,
}

impl Document {
    /// An empty, unsaved document -- what a new tab starts as, and what stands
    /// in for the active tab while its real state lives on `App`.
    pub fn empty() -> Document {
        Document {
            scene: Scene::new(),
            history: History::new(),
            path: None,
            saved_revision: 0,
            selection: Vec::new(),
            selection_anchor: None,
            collapsed: HashSet::new(),
            cursor: None,
            frame_when_evaluated: true,
            evaluated: empty_evaluation(),
        }
    }

    fn unsaved(&self) -> bool {
        self.history.revision() != self.saved_revision
    }

    fn name(&self) -> String {
        document_name(self.path.as_deref())
    }
}

/// The result of evaluating nothing: what a document shows before its first
/// evaluation lands.
pub fn empty_evaluation() -> Evaluated {
    Evaluated {
        mesh: std::sync::Arc::new(simple3d_geom::Mesh::new()),
        node_meshes: BTreeMap::new(),
        group_meshes: BTreeMap::new(),
        node_frames: BTreeMap::new(),
        node_local_bounds: BTreeMap::new(),
        node_world_bounds: BTreeMap::new(),
        errors: Vec::new(),
        cancelled: false,
    }
}

/// What a document is called: its file name, or `Untitled` before it has one.
pub fn document_name(path: Option<&Path>) -> String {
    match path {
        Some(path) => path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        None => "Untitled".to_string(),
    }
}

impl App {
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// What the tab at `index` is called, and whether it has unsaved changes.
    /// The active tab is answered from the live state on `App`, since the entry
    /// in `tabs` is only a stand-in.
    pub fn tab_summary(&self, index: usize) -> (String, bool) {
        if index == self.active {
            (document_name(self.path.as_deref()), self.unsaved())
        } else {
            let doc = &self.tabs[index];
            (doc.name(), doc.unsaved())
        }
    }

    /// True when *any* open document has unsaved changes -- the question quit
    /// has to ask, rather than only about the one on screen.
    pub fn any_unsaved(&self) -> bool {
        self.unsaved() || self.tabs.iter().enumerate().any(|(i, doc)| i != self.active && doc.unsaved())
    }

    /// Lift the current document off `App`, leaving the fields that belong to a
    /// document empty and the ones that belong to the window alone.
    fn detach(&mut self) -> Document {
        Document {
            scene: std::mem::replace(&mut self.scene, Scene::new()),
            history: std::mem::replace(&mut self.history, History::new()),
            path: self.path.take(),
            saved_revision: self.saved_revision,
            selection: std::mem::take(&mut self.selection),
            selection_anchor: self.selection_anchor.take(),
            collapsed: std::mem::take(&mut self.collapsed),
            cursor: self.cursor.take(),
            frame_when_evaluated: self.frame_when_evaluated,
            evaluated: std::mem::replace(&mut self.evaluated, empty_evaluation()),
        }
    }

    /// Make `doc` the document the application is showing.
    ///
    /// Everything half-done belongs to the document that was on screen -- a
    /// drag, a rename, a deletion waiting to be confirmed, a half-typed field --
    /// so all of it is dropped rather than carried onto a model it was never
    /// about.
    fn attach(&mut self, doc: Document) {
        self.scene = doc.scene;
        self.history = doc.history;
        self.path = doc.path;
        self.saved_revision = doc.saved_revision;
        self.selection = doc.selection;
        self.selection_anchor = doc.selection_anchor;
        self.collapsed = doc.collapsed;
        self.cursor = doc.cursor;
        self.frame_when_evaluated = doc.frame_when_evaluated;
        self.evaluated = doc.evaluated;

        self.drag = None;
        self.grabbed = None;
        self.hover_handle = None;
        self.rename = None;
        self.outliner_drag = None;
        self.drop_target = None;
        self.outliner_last_click = None;
        self.pending_delete = None;
        self.camera_move = None;
        self.cube_spin = None;
        // A measurement is about the model that was on screen, so it does not
        // travel to the next one -- and neither does the tool holding the
        // pointer. Clearing only the span left the crosshair armed over a
        // document the user had just switched to, ready to eat their first
        // click, which is not what putting the tool away means.
        self.measure = crate::app::Measure::default();
        self.fields.clear();
        self.export_preview = None;

        // Nothing cached about the model on screen survives a change of model.
        self.evaluation_generation += 1;
        self.scene_renderable = crate::render::Renderable::prepare(&self.evaluated.mesh);
        self.node_renderables.clear();
        self.renderable_key = u64::MAX;
        self.invalidate_image();

        // Submit here rather than leaving `dirty` for the next frame, and as a
        // supersede rather than an ordinary edit: an evaluation of the tab we
        // just left would otherwise come back and be applied to this one.
        self.worker.supersede(&self.scene);
        self.dirty = false;
    }

    /// Show the tab at `index`, putting the current document away first.
    pub fn activate_tab(&mut self, index: usize) {
        if index >= self.tabs.len() || index == self.active {
            return;
        }
        let current = self.detach();
        self.tabs[self.active] = current;
        let next = std::mem::replace(&mut self.tabs[index], Document::empty());
        self.active = index;
        self.attach(next);
        let (name, _) = self.tab_summary(index);
        self.status = Status::Info(format!("Showing {name}"));
    }

    /// Move `delta` tabs along, wrapping at both ends so one key can walk the
    /// whole row.
    pub fn cycle_tab(&mut self, delta: isize) {
        let count = self.tabs.len() as isize;
        if count < 2 {
            return;
        }
        let next = (self.active as isize + delta).rem_euclid(count) as usize;
        self.activate_tab(next);
    }

    /// Open `doc` in a tab of its own, after the current one, and show it.
    fn open_tab(&mut self, doc: Document) {
        let current = self.detach();
        self.tabs[self.active] = current;
        let at = self.active + 1;
        self.tabs.insert(at, Document::empty());
        self.active = at;
        self.attach(doc);
    }

    /// A new, empty document in a new tab (`Command::New`).
    pub fn new_project(&mut self) {
        self.open_tab(Document::empty());
        self.starter_scene();
        self.status = Status::Info("New project".into());
    }

    /// Whether the current document is one nothing has been done to: an empty,
    /// unsaved, never-saved document is scratch space, and opening a file uses
    /// it rather than leaving an empty tab behind.
    pub(crate) fn active_is_scratch(&self) -> bool {
        self.path.is_none() && !self.unsaved() && self.scene.node(self.scene.root()).children.is_empty()
    }

    /// The tab `path` is already open in, if it is open at all.
    fn tab_for_path(&self, path: &Path) -> Option<usize> {
        (0..self.tabs.len()).find(|index| {
            let held = if *index == self.active { self.path.as_deref() } else { self.tabs[*index].path.as_deref() };
            held == Some(path)
        })
    }

    /// Open a project file: in the tab it is already open in if it is one, in
    /// the current tab if that is still scratch space, and otherwise in a tab
    /// of its own.
    pub fn open_path(&mut self, path: &Path) {
        if let Some(index) = self.tab_for_path(path) {
            self.activate_tab(index);
            self.status = Status::Info(format!("{} is already open", document_name(Some(path))));
            return;
        }
        if !self.active_is_scratch() {
            self.open_tab(Document::empty());
        }
        self.load_into_active(path);
    }

    /// Close the tab at `index`, asking first if it has changes that would be
    /// lost. The last tab does not close: it is emptied, so there is always a
    /// document to work in.
    pub fn close_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        let (_, unsaved) = self.tab_summary(index);
        if unsaved {
            self.pending_close = Some(index);
            self.modal = Modal::ConfirmCloseTab;
            return;
        }
        self.close_tab_now(index);
    }

    /// Close the tab at `index` whatever state it is in. What the confirmation
    /// calls once the question has been answered.
    pub fn close_tab_now(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        let (name, _) = self.tab_summary(index);
        if self.tabs.len() == 1 {
            self.attach(Document::empty());
            self.starter_scene();
            self.status = Status::Info(format!("Closed {name}"));
            return;
        }
        if index == self.active {
            // Show the tab to the right, or the one to the left if this was the
            // last: the neighbour, either way, rather than jumping to an end.
            let next = if index + 1 < self.tabs.len() { index + 1 } else { index - 1 };
            let doc = std::mem::replace(&mut self.tabs[next], Document::empty());
            self.tabs.remove(index);
            self.active = if next > index { next - 1 } else { next };
            self.attach(doc);
        } else {
            self.tabs.remove(index);
            if index < self.active {
                self.active -= 1;
            }
        }
        self.status = Status::Info(format!("Closed {name}"));
    }

    /// Close the tab the confirmation was asked about, discarding its changes.
    pub fn confirm_close_tab(&mut self) {
        if let Some(index) = self.pending_close.take() {
            self.close_tab_now(index);
        }
        self.modal = Modal::None;
    }

    /// Save the tab the confirmation was asked about, then close it if the save
    /// went through. Only the active tab can be saved -- saving asks for a path
    /// and writes what the editor is showing -- so it is shown first.
    pub fn save_and_close_tab(&mut self) {
        let Some(index) = self.pending_close.take() else {
            self.modal = Modal::None;
            return;
        };
        self.modal = Modal::None;
        self.activate_tab(index);
        self.save();
        if !self.unsaved() {
            self.close_tab_now(self.active);
        }
    }

    pub fn cancel_close_tab(&mut self) {
        self.pending_close = None;
        self.modal = Modal::None;
    }
}

/// The row of open documents: the top of the workspace, between the docks and
/// over the viewport, so a tab sits above the model it holds.
///
/// Drawn by hand rather than out of widgets so a tab can be a shape -- the
/// active one lit along its top edge and joined to the workspace below it --
/// which is what makes the row readable at a glance.
pub fn show(app: &mut App, ctx: &egui::Context) {
    let frame = egui::Frame::NONE.fill(token::SURFACE_1);
    egui::TopBottomPanel::top("tabs").frame(frame).exact_height(metric::TAB_BAR).show(ctx, |ui| {
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.max_rect().bottom() - 0.5,
            egui::Stroke::new(1.0_f32, token::SURFACE_3),
        );
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(1.0, 0.0);
            let mut clicked: Option<usize> = None;
            let mut closed: Option<usize> = None;
            for index in 0..app.tab_count() {
                let (name, unsaved) = app.tab_summary(index);
                match tab(ui, &name, unsaved, index == app.active) {
                    Some(Hit::Pick) => clicked = Some(index),
                    Some(Hit::Close) => closed = Some(index),
                    None => {}
                }
            }
            if plus(ui) {
                app.run(simple3d_core::keymap::Command::New);
            }
            // After the row, so closing a tab cannot renumber the ones still
            // being drawn.
            if let Some(index) = clicked {
                app.activate_tab(index);
            }
            if let Some(index) = closed {
                app.close_tab(index);
            }
        });
    });
}

/// What a click on a tab was.
enum Hit {
    Pick,
    Close,
}

/// One tab. Returns what was clicked on it, if anything.
fn tab(ui: &mut egui::Ui, name: &str, unsaved: bool, active: bool) -> Option<Hit> {
    const MIN: f32 = 96.0;
    const MAX: f32 = 220.0;
    const CLOSE: f32 = 16.0;

    let font = egui::FontId::proportional(theme::font::VALUE);
    let label = if unsaved { format!("{name} \u{2022}") } else { name.to_string() };
    let text_width = ui.fonts(|fonts| fonts.layout_no_wrap(label.clone(), font.clone(), token::TEXT_HI).size().x);
    let width = (text_width + CLOSE + 24.0).clamp(MIN, MAX.min(ui.available_width().max(MIN)));
    let height = ui.available_height();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    let fill = if active {
        token::SURFACE_0B
    } else if response.hovered() {
        token::SURFACE_2
    } else {
        token::SURFACE_1
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    if active {
        // The lit top edge, and no rule along the bottom: the active tab is the
        // workspace's own top, not a button sitting above it.
        ui.painter().hline(rect.x_range(), rect.top() + 1.0, egui::Stroke::new(2.0_f32, token::ACCENT));
    } else {
        ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0_f32, token::SURFACE_3));
    }
    ui.painter().vline(rect.right() - 0.5, rect.y_range(), egui::Stroke::new(1.0_f32, token::SURFACE_0));

    let close_rect =
        egui::Rect::from_center_size(egui::pos2(rect.right() - 13.0, rect.center().y), egui::Vec2::splat(CLOSE));
    let text_colour = if active { token::TEXT_HI } else { token::TEXT_LO };
    let mut job = egui::text::LayoutJob::simple_singleline(label, font, text_colour);
    job.wrap.max_width = (close_rect.left() - rect.left() - 16.0).max(8.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    ui.painter().galley(egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y * 0.5), galley, text_colour);

    let close = ui.interact(close_rect, response.id.with("close"), egui::Sense::click());
    if close.hovered() {
        ui.painter().rect_filled(close_rect, 3.0, token::SURFACE_3);
    }
    cross(ui.painter(), close_rect.center(), if close.hovered() { token::TEXT_HI } else { token::TEXT_LO });
    let response =
        response.on_hover_text(if unsaved { format!("{name} \u{2022} unsaved changes") } else { name.to_string() });

    if close.clicked() || response.middle_clicked() {
        return Some(Hit::Close);
    }
    if response.clicked() {
        return Some(Hit::Pick);
    }
    None
}

/// The button at the end of the row: another document.
fn plus(ui: &mut egui::Ui) -> bool {
    let size = egui::vec2(28.0, ui.available_height());
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let colour = if response.hovered() { token::TEXT_HI } else { token::TEXT_LO };
    if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, token::SURFACE_2);
    }
    let centre = rect.center();
    let stroke = egui::Stroke::new(1.4_f32, colour);
    ui.painter().hline(centre.x - 5.0..=centre.x + 5.0, centre.y, stroke);
    ui.painter().vline(centre.x, centre.y - 5.0..=centre.y + 5.0, stroke);
    response.on_hover_text("New document").clicked()
}

/// The close cross, drawn rather than typed: a glyph would depend on the font
/// having it, and would sit off centre in most that do.
fn cross(painter: &egui::Painter, centre: egui::Pos2, colour: egui::Color32) {
    let r = 3.5;
    let stroke = egui::Stroke::new(1.3_f32, colour);
    painter.line_segment([centre + egui::vec2(-r, -r), centre + egui::vec2(r, r)], stroke);
    painter.line_segment([centre + egui::vec2(r, -r), centre + egui::vec2(-r, r)], stroke);
}
