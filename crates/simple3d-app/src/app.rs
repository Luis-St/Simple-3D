//! The application: window layout, command dispatch, file handling and the
//! glue that keeps the outliner, property editor and viewport in step
//! (spec section 7).

use crate::gizmo::{self, Drag, Gizmo, Handle, Mode};
use crate::panel_viewport;
use crate::render::Renderable;
use crate::ui::FieldBuffers;
use crate::view::{frame_bounds, CameraMove, ViewPreset};
use crate::worker::{EvalWorker, ExportJob};
use simple3d_core::clipboard::{self, Clip};
use simple3d_core::config::{self, AppSettings, DisplayMode, HandleFrame, Placement, Side};
use simple3d_core::eval::Evaluated;
use simple3d_core::keymap::{Chord, Command, Keymap};
use simple3d_core::library;
use simple3d_core::project;
use simple3d_core::scene::{GroupOp, NodeId, Scene};
use simple3d_core::undo::History;
use simple3d_core::unit::Unit;
use simple3d_export::Format;
use simple3d_geom::Vec3;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const APP_NAME: &str = "Simple 3D";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PROJECT_EXTENSION: &str = "simple3d";
/// An export that has not finished by now has gone wrong; better a clear message
/// than an indefinite hang (spec section 9).
pub const EXPORT_LIMIT: Duration = Duration::from_secs(120);

/// A message for the status bar, and how it should read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    Info(String),
    Warning(String),
}

impl Status {
    pub fn text(&self) -> &str {
        match self {
            Status::Idle => "Ready",
            Status::Info(text) | Status::Warning(text) => text,
        }
    }
}

/// Which of the modal windows is open. Only one at a time, so the state cannot
/// contradict itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Modal {
    #[default]
    None,
    Export,
    Keymap,
    SceneSettings,
    About,
    /// A failure worth stopping for, shown in a scrollable, copyable window.
    Error,
    /// Naming a group, or a whole project, to keep on the palette.
    SavePrimitive,
    /// Quitting with unsaved changes.
    ConfirmQuit,
}

pub struct App {
    pub scene: Scene,
    pub history: History,
    pub settings: AppSettings,
    pub keymap: Keymap,

    /// The current selection, in click order. The last entry is the primary one
    /// the property editor and the manipulator act on.
    pub selection: Vec<NodeId>,
    pub clipboard: Option<Clip>,

    pub worker: EvalWorker,
    pub evaluated: Evaluated,
    /// Bumped whenever a new evaluation lands, so cached images and renderables
    /// know to rebuild.
    pub evaluation_generation: u64,
    pub(crate) dirty: bool,

    pub scene_renderable: Renderable,
    pub node_renderables: BTreeMap<NodeId, Renderable>,
    pub(crate) renderable_key: u64,

    pub mode: Mode,
    pub drag: Option<Drag>,
    pub hover_handle: Option<Handle>,
    /// The handle that was under the pointer when the button went down. What
    /// starts a drag, rather than whatever the pointer has since slipped onto.
    pub grabbed: Option<Handle>,
    pub viewport_rect: egui::Rect,
    pub texture: Option<egui::TextureHandle>,
    pub image_key: u64,

    pub path: Option<PathBuf>,
    saved_revision: u64,

    pub status: Status,
    /// When the current message was set, so a message that has been read can
    /// fade out instead of sitting there looking current.
    pub status_at: std::time::Instant,
    pub fields: FieldBuffers,
    /// Which field label is being dragged, if any. Held on the app rather than
    /// in widget state so the gesture survives the panel being relaid out.
    pub scrub: crate::ui::Scrub,
    pub rename: Option<(NodeId, String)>,
    pub outliner_drag: Option<NodeId>,
    pub drop_target: Option<DropTarget>,

    /// The 3D cursor: where a new shape lands. `None` means the origin, which
    /// is also where it goes back to.
    pub cursor: Option<Vec3>,
    /// A deletion waiting on the outliner's confirmation strip: which nodes,
    /// with the question of what happens to their children still open.
    pub pending_delete: Option<Vec<NodeId>>,
    /// A view change in flight. The camera is the scene's, so the move writes
    /// into it every frame rather than holding a second copy of the truth.
    pub camera_move: Option<CameraMove>,
    /// Which panel header is being dragged between docks, and where to.
    pub dock_drag: crate::dock::DockDrag,
    /// Header centres and the outer rectangle of each dock, collected while the
    /// docks draw and consumed by the drag resolution after them.
    pub dock_headers: Vec<(Side, Vec<f32>)>,
    pub dock_rects: Vec<(Side, egui::Rect)>,

    pub export_job: Option<ExportJob>,
    pub export_format: Format,
    pub export_scale: String,
    pub export_selection_only: bool,

    pub modal: Modal,
    pub error_title: String,
    pub error_detail: String,

    /// The subtree waiting to be saved to the library, and what to call it.
    pub primitive_clip: Option<Clip>,
    pub primitive_name: String,
    /// The library, as it was last read off disk. Re-read when it changes rather
    /// than on every frame -- the palette draws sixty times a second and the
    /// library lives in a directory.
    pub library: Vec<library::Entry>,

    pub keymap_search: String,
    pub recording: Option<Command>,
    pub keymap_conflict: Option<(Command, Chord, Command)>,

    /// Where settings and the keymap are read from and written back to. Held
    /// rather than looked up at each call site so a test can point an `App` at a
    /// temp directory, and so a running application cannot start reading one
    /// directory and writing another.
    config_dir: PathBuf,

    /// The message the fade clock is running for, so any assignment to `status`
    /// anywhere restarts it without having to remember to.
    last_status: Status,
    /// True while a run of held-down nudge keys is coalescing into one undo step.
    nudging: bool,
    /// Set once a quit has been confirmed, so the event loop can close the window.
    quit_now: bool,
}

/// Where an outliner drag would drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropTarget {
    pub parent: NodeId,
    pub index: usize,
    /// Set when the drop is *into* a group rather than between two siblings, so
    /// the indicator can differ.
    pub into: Option<NodeId>,
}

impl App {
    pub fn new(ctx: &egui::Context, open: Option<PathBuf>) -> App {
        App::with_config_dir(ctx, open, config::config_dir())
    }

    /// `new`, but reading and writing settings and the keymap in `config_dir`
    /// rather than the user's own. Tests use this so their result cannot depend
    /// on what happens to be in the developer's config directory, and so they
    /// cannot write to it.
    pub fn with_config_dir(ctx: &egui::Context, open: Option<PathBuf>, config_dir: PathBuf) -> App {
        crate::theme::apply(ctx);
        let settings = config::load_settings_from(&config_dir);
        let keymap = config::load_keymap_from(&config_dir);
        let mut app = App {
            scene: Scene::new(),
            history: History::new(),
            settings,
            keymap,
            selection: Vec::new(),
            clipboard: None,
            worker: EvalWorker::spawn(),
            evaluated: Evaluated {
                mesh: std::sync::Arc::new(simple3d_geom::Mesh::new()),
                node_meshes: BTreeMap::new(),
                node_frames: BTreeMap::new(),
                node_local_bounds: BTreeMap::new(),
                node_world_bounds: BTreeMap::new(),
                errors: Vec::new(),
                cancelled: false,
            },
            evaluation_generation: 0,
            dirty: true,
            scene_renderable: Renderable::empty(),
            node_renderables: BTreeMap::new(),
            renderable_key: u64::MAX,
            mode: Mode::Move,
            drag: None,
            hover_handle: None,
            grabbed: None,
            viewport_rect: egui::Rect::NOTHING,
            texture: None,
            image_key: u64::MAX,
            path: None,
            saved_revision: 0,
            status: Status::Idle,
            status_at: std::time::Instant::now(),
            fields: FieldBuffers::default(),
            scrub: crate::ui::Scrub::default(),
            rename: None,
            outliner_drag: None,
            drop_target: None,
            cursor: None,
            pending_delete: None,
            camera_move: None,
            dock_drag: crate::dock::DockDrag::default(),
            dock_headers: Vec::new(),
            dock_rects: Vec::new(),
            export_job: None,
            export_format: Format::ThreeMf,
            export_scale: "1".to_string(),
            export_selection_only: false,
            modal: Modal::None,
            error_title: String::new(),
            error_detail: String::new(),
            primitive_clip: None,
            primitive_name: String::new(),
            library: Vec::new(),
            keymap_search: String::new(),
            recording: None,
            keymap_conflict: None,
            config_dir,
            last_status: Status::Idle,
            nudging: false,
            quit_now: false,
        };
        app.export_format = Format::from_id(&app.settings.last_export_format).unwrap_or(Format::ThreeMf);
        app.export_scale = simple3d_core::unit::format_number(app.settings.last_export_scale, 4);
        app.refresh_library();
        match open {
            // Opening a project by passing its path on the command line, so file
            // associations work on both platforms (spec section 10).
            Some(path) => app.open_path(&path),
            None => app.starter_scene(),
        }
        app
    }

    /// An empty document. Nothing is added for the user: a shape they did not
    /// ask for is a shape they have to notice and delete, and the palette is
    /// one click away.
    fn starter_scene(&mut self) {
        self.history.clear();
        self.saved_revision = self.history.revision();
        self.frame_all();
        self.dirty = true;
    }

    // -- selection ----------------------------------------------------------

    pub fn primary(&self) -> Option<NodeId> {
        self.selection.iter().rev().find(|id| self.scene.contains(**id)).copied()
    }

    pub fn is_selected(&self, id: NodeId) -> bool {
        self.selection.contains(&id)
    }

    pub fn select_only(&mut self, id: NodeId) {
        self.selection = vec![id];
        self.on_selection_changed();
    }

    pub fn toggle_selected(&mut self, id: NodeId) {
        if let Some(at) = self.selection.iter().position(|&x| x == id) {
            self.selection.remove(at);
        } else {
            self.selection.push(id);
        }
        self.on_selection_changed();
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.on_selection_changed();
    }

    fn on_selection_changed(&mut self) {
        // A half-typed field belongs to the node it was opened on.
        self.fields.clear();
        self.history.close();
        self.rename = None;
    }

    // -- edits --------------------------------------------------------------

    /// Take an undo snapshot and mark the scene for re-evaluation. Every
    /// model-mutating path in the application goes through here.
    pub fn edit(&mut self, label: &str, coalesce: Option<&str>) {
        self.history.record(&self.scene, label, coalesce);
        self.dirty = true;
    }

    /// Mark the scene for re-evaluation without taking a snapshot, for the
    /// frames *during* a drag -- the snapshot was taken when the drag began, so
    /// the whole drag is one undo step.
    pub fn touch(&mut self) {
        self.dirty = true;
    }

    pub fn unsaved(&self) -> bool {
        self.history.revision() != self.saved_revision
    }

    pub fn status_text(&self) -> String {
        self.status.text().to_string()
    }

    /// Force the viewport image to be rebuilt on the next frame.
    pub fn invalidate_image(&mut self) {
        self.image_key = u64::MAX;
    }

    pub fn unit(&self) -> Unit {
        self.scene.settings.unit
    }

    /// The move and resize snap increment, in millimetres (spec section 6.2).
    pub fn move_snap(&self) -> f64 {
        self.scene.settings.snap_step.max(1e-6)
    }

    // -- files --------------------------------------------------------------

    pub fn title(&self) -> String {
        let name = match &self.path {
            Some(path) => path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            None => "Untitled".to_string(),
        };
        format!("{}{name} - {APP_NAME}", if self.unsaved() { "*" } else { "" })
    }

    pub fn new_project(&mut self) {
        self.scene = Scene::new();
        self.selection.clear();
        self.history.clear();
        self.path = None;
        self.starter_scene();
        self.status = Status::Info("New project".into());
    }

    pub fn open_dialog(&mut self) {
        let mut dialog = rfd::FileDialog::new().add_filter("Simple 3D project", &[PROJECT_EXTENSION]);
        if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.open_path(&path);
        }
    }

    pub fn open_path(&mut self, path: &Path) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                self.settings.forget_recent(path);
                return self.fail("Could not open the project", &format!("{}\n\n{e}", path.display()));
            }
        };
        match project::from_str(&text) {
            Ok(scene) => {
                self.scene = scene;
                self.selection.clear();
                self.history.clear();
                self.saved_revision = self.history.revision();
                self.path = Some(path.to_path_buf());
                self.settings.remember_recent(path);
                self.fields.clear();
                self.dirty = true;
                self.status = Status::Info(format!("Opened {}", path.display()));
            }
            Err(e) => {
                self.settings.forget_recent(path);
                self.fail("Could not read the project", &format!("{}\n\n{e}", path.display()));
            }
        }
    }

    pub fn save(&mut self) {
        match self.path.clone() {
            Some(path) => self.save_to(&path),
            None => self.save_as(),
        }
    }

    pub fn save_as(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Simple 3D project", &[PROJECT_EXTENSION])
            .set_file_name(format!("model.{PROJECT_EXTENSION}"));
        if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(mut path) = dialog.save_file() {
            if path.extension().is_none() {
                path.set_extension(PROJECT_EXTENSION);
            }
            self.save_to(&path);
        }
    }

    fn save_to(&mut self, path: &Path) {
        let text = project::to_string(&self.scene);
        match std::fs::write(path, text) {
            Ok(()) => {
                self.path = Some(path.to_path_buf());
                self.saved_revision = self.history.revision();
                self.settings.remember_recent(path);
                self.status = Status::Info(format!("Saved {}", path.display()));
            }
            Err(e) => self.fail("Could not save the project", &format!("{}\n\n{e}", path.display())),
        }
    }

    pub fn fail(&mut self, title: &str, detail: &str) {
        self.error_title = title.to_string();
        self.error_detail = detail.to_string();
        self.modal = Modal::Error;
        self.status = Status::Warning(title.to_string());
    }

    // -- camera -------------------------------------------------------------

    fn aspect(&self) -> f64 {
        let size = self.viewport_rect.size();
        if size.y > 1.0 {
            (size.x / size.y) as f64
        } else {
            1.0
        }
    }

    pub fn frame_all(&mut self) {
        match self.evaluated.mesh.bounds().or_else(|| self.selection_bounds()) {
            Some((lo, hi)) => {
                let aspect = self.aspect();
                frame_bounds(&mut self.scene.camera, lo, hi, aspect);
            }
            None => {
                self.scene.camera.target = Vec3::ZERO;
                self.scene.camera.distance = 160.0;
            }
        }
    }

    pub fn frame_selection(&mut self) {
        match self.selection_bounds() {
            Some((lo, hi)) => {
                let aspect = self.aspect();
                frame_bounds(&mut self.scene.camera, lo, hi, aspect);
            }
            None => self.frame_all(),
        }
    }

    pub fn selection_bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut result: Option<(Vec3, Vec3)> = None;
        for id in &self.selection {
            for node in std::iter::once(*id).chain(self.scene.descendants(*id)) {
                let Some(mesh) = self.evaluated.node_meshes.get(&node) else { continue };
                let Some((lo, hi)) = mesh.bounds() else { continue };
                result = Some(match result {
                    None => (lo, hi),
                    Some((a, b)) => (a.min(lo), b.max(hi)),
                });
            }
        }
        result
    }

    pub fn set_view(&mut self, preset: ViewPreset) {
        let (yaw, pitch) = preset.angles();
        self.turn_camera_to(yaw, pitch);
        self.status = Status::Info(format!("View: {}", preset.label()));
    }

    /// Turn the camera to face a given way, over the design's 200 ms, taking
    /// the short way round. Under a reduced-motion preference it simply arrives:
    /// the transition is there to show that this is the same camera moving, and
    /// someone who does not want things moving does not need to be shown that.
    pub fn turn_camera_to(&mut self, yaw: f64, pitch: f64) {
        let from = (self.scene.camera.yaw, self.scene.camera.pitch);
        let to = (from.0 + crate::view::shortest_turn(from.0, yaw), pitch);
        if self.settings.reduce_motion {
            self.scene.camera.yaw = to.0;
            self.scene.camera.pitch = to.1;
            self.camera_move = None;
            return;
        }
        self.camera_move = Some(CameraMove { from, to, started: std::time::Instant::now() });
    }

    /// Advance a view change. Called once a frame; does nothing when none is in
    /// flight.
    pub fn advance_camera(&mut self) {
        let Some(move_) = self.camera_move else { return };
        let ((yaw, pitch), done) = move_.at(std::time::Instant::now());
        self.scene.camera.yaw = yaw;
        self.scene.camera.pitch = pitch;
        if done {
            self.camera_move = None;
        }
    }

    // -- commands -----------------------------------------------------------

    pub fn run(&mut self, command: Command) {
        use Command::*;
        match command {
            New => self.new_project(),
            Open => self.open_dialog(),
            Save => self.save(),
            SaveAs => self.save_as(),
            Export => self.modal = Modal::Export,
            Quit => self.request_quit(),

            Undo => match {
                let label = self.history.undo(&mut self.scene);
                label
            } {
                Some(label) => {
                    self.after_history(&format!("Undid {label}"));
                }
                None => self.status = Status::Info("Nothing to undo".into()),
            },
            Redo => match {
                let label = self.history.redo(&mut self.scene);
                label
            } {
                Some(label) => self.after_history(&format!("Redid {label}")),
                None => self.status = Status::Info("Nothing to redo".into()),
            },
            Copy => self.copy_selection(false),
            Cut => self.copy_selection(true),
            Paste => self.paste(),
            Duplicate => self.duplicate(),
            Delete => self.delete_selection(),
            Group => self.group_selection(),
            Rename => {
                if let Some(id) = self.primary() {
                    self.rename = Some((id, self.scene.node(id).name.clone()));
                }
            }
            ToggleVisibility => self.toggle_visibility(),
            MoveUp => self.reorder(-1),
            MoveDown => self.reorder(1),

            FrameSelection => self.frame_selection(),
            FrameAll => self.frame_all(),
            ToggleProjection => {
                self.scene.camera.orthographic = !self.scene.camera.orthographic;
                let which = if self.scene.camera.orthographic { "Orthographic" } else { "Perspective" };
                self.status = Status::Info(which.into());
            }
            ViewTop => self.set_view(ViewPreset::Top),
            ViewBottom => self.set_view(ViewPreset::Bottom),
            ViewFront => self.set_view(ViewPreset::Front),
            ViewBack => self.set_view(ViewPreset::Back),
            ViewLeft => self.set_view(ViewPreset::Left),
            ViewRight => self.set_view(ViewPreset::Right),
            ViewIsometric => self.set_view(ViewPreset::Isometric),
            ToggleGrid => self.scene.settings.grid_visible = !self.scene.settings.grid_visible,
            ToggleAxisX => self.toggle_axis(0),
            ToggleAxisY => self.toggle_axis(1),
            ToggleAxisZ => self.toggle_axis(2),
            DisplayShaded => self.settings.display_mode = DisplayMode::Shaded,
            DisplayShadedEdges => self.settings.display_mode = DisplayMode::ShadedWithEdges,
            DisplayWireframe => self.settings.display_mode = DisplayMode::Wireframe,
            ToggleBoundingBox => self.settings.show_bounding_box = !self.settings.show_bounding_box,
            ToggleGhosts => self.settings.show_ghosts = !self.settings.show_ghosts,
            ToggleDocks => {
                self.settings.layout.docks_hidden = !self.settings.layout.docks_hidden;
                self.status = Status::Info(
                    if self.settings.layout.docks_hidden {
                        "Docks hidden; press it again to bring them back exactly as they were"
                    } else {
                        "Docks restored"
                    }
                    .into(),
                );
            }
            ResetLayout => {
                crate::dock::reset(self);
                self.status = Status::Info("Panel layout reset".into());
            }

            ModeMove => self.mode = Mode::Move,
            ModeRotate => self.mode = Mode::Rotate,
            ModeResize => self.mode = Mode::Resize,
            ToggleHandleFrame => {
                self.settings.handle_frame = self.settings.handle_frame.toggled();
                self.status = Status::Info(format!("Handles: {} frame", self.settings.handle_frame.label()));
            }
            NudgeLeft | NudgeRight | NudgeUp | NudgeDown | NudgeAway | NudgeToward => self.nudge(command),
        }
    }

    fn toggle_axis(&mut self, axis: usize) {
        let on = !self.scene.settings.axes_visible[axis];
        self.scene.settings.axes_visible[axis] = on;
        let name = ["X", "Y", "Z"][axis];
        self.status = Status::Info(format!("{name} axis {}", if on { "shown" } else { "hidden" }));
    }

    fn after_history(&mut self, message: &str) {
        self.selection.retain(|id| self.scene.contains(*id));
        self.fields.clear();
        self.rename = None;
        self.dirty = true;
        self.status = Status::Info(message.into());
    }

    fn copy_selection(&mut self, cut: bool) {
        let Some(clip) = clipboard::copy(&self.scene, &self.selection) else {
            self.status = Status::Info("Nothing to copy".into());
            return;
        };
        let count = clip.nodes.len();
        self.clipboard = Some(clip);
        if cut {
            // Cut is an undoable step of its own, so it cannot lose work even if
            // the user never pastes (spec section 8.1).
            self.edit("Cut", None);
            let doomed: Vec<NodeId> = self.top_level_selection();
            for id in doomed {
                self.scene.remove(id);
            }
            self.clear_selection();
            self.status = Status::Info(format!("Cut {count} node{}", if count == 1 { "" } else { "s" }));
        } else {
            self.status = Status::Info(format!("Copied {count} node{}", if count == 1 { "" } else { "s" }));
        }
    }

    fn paste(&mut self) {
        let Some(clip) = self.clipboard.clone() else {
            self.status = Status::Info("The clipboard is empty".into());
            return;
        };
        self.edit("Paste", None);
        let target = self.primary();
        let created = clipboard::paste(&mut self.scene, &clip, target);
        if created.is_empty() {
            self.status = Status::Warning("Nothing could be pasted".into());
            return;
        }
        // Left selected, so a nudge or a drag can follow immediately.
        self.selection = created;
        self.on_selection_changed();
        self.status = Status::Info("Pasted".into());
    }

    fn duplicate(&mut self) {
        let targets = self.top_level_selection();
        if targets.is_empty() {
            self.status = Status::Info("Nothing to duplicate".into());
            return;
        }
        // A separate action from copy and paste: it does not disturb the
        // clipboard (spec section 8.1).
        self.edit("Duplicate", None);
        let mut created = Vec::new();
        for id in targets {
            if let Some(copy) = self.scene.duplicate(id) {
                created.push(copy);
            }
        }
        if !created.is_empty() {
            self.selection = created;
            self.on_selection_changed();
        }
        self.status = Status::Info("Duplicated".into());
    }

    fn delete_selection(&mut self) {
        let targets: Vec<NodeId> =
            self.top_level_selection().into_iter().filter(|id| *id != self.scene.root()).collect();
        if targets.is_empty() {
            self.status = Status::Info("Nothing to delete".into());
            return;
        }
        // Deleting a group is two different actions wearing one word: the
        // children can go with it, or stay. Rather than guess, or open a dialog
        // over the model, the outliner asks in place.
        if targets.iter().any(|id| self.scene.node(*id).is_group() && !self.scene.node(*id).children.is_empty()) {
            self.pending_delete = Some(targets);
            return;
        }
        self.delete_now(&targets, false);
    }

    /// How many nodes the pending deletion would take with it, if the children
    /// go too.
    pub fn pending_delete_count(&self) -> usize {
        let Some(targets) = &self.pending_delete else { return 0 };
        targets.iter().map(|id| 1 + self.scene.descendants(*id).len()).sum()
    }

    /// Carry out the deletion the outliner asked about. `promote` keeps the
    /// children by moving them up into the group's own place first.
    pub fn confirm_delete(&mut self, promote: bool) {
        let Some(targets) = self.pending_delete.take() else { return };
        self.delete_now(&targets, promote);
    }

    pub fn cancel_delete(&mut self) {
        if self.pending_delete.take().is_some() {
            self.status = Status::Info("Nothing deleted".into());
        }
    }

    fn delete_now(&mut self, targets: &[NodeId], promote: bool) {
        self.edit("Delete", None);
        let mut promoted = 0;
        for id in targets {
            if promote {
                // Into the group's own slot, in order, so the tree reads the
                // same afterwards minus one level of nesting.
                let node = self.scene.node(*id);
                let children = node.children.clone();
                if let Some(parent) = node.parent {
                    let at = self.scene.node(parent).children.iter().position(|c| c == id).unwrap_or(0);
                    for (offset, child) in children.iter().enumerate() {
                        if self.scene.reparent(*child, parent, at + offset).is_ok() {
                            promoted += 1;
                        }
                    }
                }
            }
            self.scene.remove(*id);
        }
        self.clear_selection();
        let removed = targets.len();
        self.status = Status::Info(if promote {
            format!("Deleted {removed} group{}, kept {promoted} child{}", plural(removed), children_plural(promoted))
        } else {
            format!("Deleted {removed} node{}", plural(removed))
        });
    }

    fn group_selection(&mut self) {
        if self.selection.is_empty() {
            self.status = Status::Info("Select something to group".into());
            return;
        }
        self.edit("Group", None);
        match self.scene.group_selection(&self.selection.clone()) {
            Some(group) => {
                self.select_only(group);
                self.status = Status::Info("Grouped".into());
            }
            None => self.status = Status::Warning("That selection cannot be grouped".into()),
        }
    }

    fn toggle_visibility(&mut self) {
        let targets = self.top_level_selection();
        if targets.is_empty() {
            return;
        }
        self.edit("Toggle visibility", None);
        // Everything follows the primary node, so a mixed selection ends up
        // consistent rather than inverted node by node.
        let target_state = self.primary().map(|id| !self.scene.node(id).visible).unwrap_or(false);
        for id in targets {
            if let Some(node) = self.scene.get_mut(id) {
                node.visible = target_state;
            }
        }
    }

    fn reorder(&mut self, delta: isize) {
        let Some(id) = self.primary() else { return };
        self.edit("Reorder", None);
        if !self.scene.reorder(id, delta) {
            self.status = Status::Info("Already at the end".into());
        }
    }

    /// Nudge the selection along the two axes most closely aligned with the
    /// screen. In rotate and resize modes the same keys rotate and resize
    /// instead (spec section 6.2).
    fn nudge(&mut self, command: Command) {
        let Some(id) = self.primary() else { return };
        let Some(gizmo) = self.gizmo_for(id) else { return };
        let view = self.current_view();
        let snap = self.move_snap();
        let rotate_snap = self.settings.rotate_snap_deg;
        // Records the undo step under a key stable across a held run, so the
        // whole run coalesces into one.
        let step =
            gizmo::apply_nudge(&mut self.history, &mut self.scene, &gizmo, &view, id, command, snap, rotate_snap);
        let Some(step) = step else { return };
        self.touch();
        self.nudging = true;

        if let gizmo::Nudge::NoDimension { axis } = step {
            self.status = Status::Info(format!(
                "{} has no dimension on the {} axis",
                self.scene.node(id).name,
                gizmo::axis_name(axis)
            ));
        }
        self.fields.clear();
    }

    /// Carry out one frame of a manipulator drag. `panel_viewport::manipulate`
    /// reads the pointer off an `egui::Response`, works out the phase with
    /// `gizmo::drag_phase`, and hands the result here; nothing about this depends
    /// on a running frame, so a whole gesture can be driven from a test.
    ///
    /// The undo record happens on `Begin` and nowhere else -- that is what makes
    /// a completed drag one undo step (spec acceptance criterion 23). The frames
    /// in between call `touch`, which marks the scene dirty without snapshotting.
    pub fn manipulate_step(
        &mut self,
        gizmo: &Gizmo,
        view: &crate::view::View,
        id: NodeId,
        phase: gizmo::DragPhase,
        handle: Option<Handle>,
        cursor: Option<egui::Pos2>,
        mods: gizmo::Mods,
    ) {
        match phase {
            gizmo::DragPhase::Cancel => {
                if let Some(drag) = self.drag.take() {
                    drag.cancel(&mut self.scene);
                    // The snapshot taken at Begin describes exactly the state the
                    // cancel just restored, so keeping it would leave a dead undo
                    // step behind a drag the user explicitly abandoned.
                    self.history.discard_last();
                    self.touch();
                    self.fields.clear();
                    self.status = Status::Info("Drag cancelled".into());
                }
            }
            gizmo::DragPhase::Finish => {
                self.drag = None;
                self.history.close();
                self.fields.clear();
            }
            gizmo::DragPhase::Continue => {
                let snap = self.move_snap();
                let (Some(drag), Some(cursor)) = (self.drag.as_mut(), cursor) else { return };
                let rotate_snap = self.settings.rotate_snap_deg;
                let unit = self.scene.settings.unit;
                drag.update(&mut self.scene, view, cursor, mods, snap, rotate_snap, unit);
                // The property editor tracks the handle live, and the preview follows.
                self.fields.clear();
                self.touch();
            }
            gizmo::DragPhase::Begin => {
                let (Some(cursor), Some(handle)) = (cursor, handle) else { return };
                // One snapshot for the whole drag, so it undoes in a single step.
                self.edit(
                    match self.mode {
                        Mode::Move => "Move",
                        Mode::Rotate => "Rotate",
                        Mode::Resize => "Resize",
                    },
                    None,
                );
                self.drag = Drag::begin(&self.scene, gizmo, id, handle, view, cursor);
            }
            gizmo::DragPhase::Idle => {}
        }
    }

    /// The topmost selected nodes: selecting a group and one of its children acts
    /// on the group only.
    pub fn top_level_selection(&self) -> Vec<NodeId> {
        let order = self.scene.depth_first();
        let mut tops: Vec<NodeId> = self
            .selection
            .iter()
            .copied()
            .filter(|id| self.scene.contains(*id))
            .filter(|&id| !self.selection.iter().any(|&other| other != id && self.scene.is_ancestor_of(other, id)))
            .collect();
        tops.sort_by_key(|id| order.iter().position(|o| o == id).unwrap_or(usize::MAX));
        tops.dedup();
        tops
    }

    pub fn add_node(&mut self, type_id: Option<&str>, op: GroupOp) {
        self.edit("Add", None);
        let (parent, index) = self.scene.insertion_point(self.primary());
        let at = self.insertion_point_world();
        let created = match type_id {
            Some(type_id) => self.scene.add_primitive(type_id, parent, index),
            None => Some(self.scene.add_group(op, parent, index)),
        };
        match created {
            Some(id) => {
                // Where a new shape lands is the user's choice; the palette's
                // hint line says which choice is in force.
                if let Some(node) = self.scene.get_mut(id) {
                    node.position = at;
                }
                self.select_only(id);
                self.status = Status::Info(format!("Added {}", self.scene.node(id).name));
            }
            None => self.status = Status::Warning("Unknown primitive type".into()),
        }
    }

    /// Where the next shape goes, in world millimetres, under the current
    /// placement choice.
    ///
    /// World rather than parent-frame: adding into a rotated group would
    /// otherwise put the shape somewhere else entirely. `Node::position` is in
    /// the parent's coordinates, so the answer is carried back through the
    /// parent's frame before it is written.
    pub fn insertion_point_world(&self) -> Vec3 {
        let world = match self.settings.placement {
            Placement::Origin => Vec3::ZERO,
            Placement::Cursor => self.cursor.unwrap_or(Vec3::ZERO),
            Placement::ViewCentre => {
                let step = self.move_snap();
                let t = self.scene.camera.target;
                Vec3::new((t.x / step).round() * step, (t.y / step).round() * step, (t.z / step).round() * step)
            }
            Placement::BesideSelection => match self.selection_bounds() {
                // Clear of the selection along +X with one step of air, so the
                // new shape is next to what is selected rather than inside it.
                Some((lo, hi)) => Vec3::new(hi.x + self.move_snap(), (lo.y + hi.y) * 0.5, (lo.z + hi.z) * 0.5),
                None => Vec3::ZERO,
            },
        };
        let (parent, _) = self.scene.insertion_point(self.primary());
        match self.evaluated.node_frames.get(&parent) {
            // The frame stored for a node is its *parent's*; a child of `parent`
            // is placed in `parent`'s own frame, which is that composed with its
            // transform.
            Some(frame) => {
                let node = self.scene.node(parent);
                frame
                    .compose(&simple3d_core::xform::Xform::from_pos_rot(node.position, node.rotation))
                    .inverse()
                    .point(world)
            }
            None => world,
        }
    }

    // -- the saved-primitive library ----------------------------------------

    pub fn refresh_library(&mut self) {
        self.library = library::list(&self.config_dir);
    }

    /// Open the naming window for the current selection.
    pub fn save_selection_as_primitive(&mut self) {
        match clipboard::copy(&self.scene, &self.selection) {
            Some(clip) => {
                let name = self.primary().map(|id| self.scene.node(id).name.clone()).unwrap_or_default();
                self.begin_save_primitive(clip, name);
            }
            None => self.status = Status::Warning("Select something to save as a primitive".into()),
        }
    }

    /// The same, for everything in the document. A project *is* a primitive as
    /// far as another project is concerned.
    pub fn save_project_as_primitive(&mut self) {
        let top: Vec<NodeId> = self.scene.node(self.scene.root()).children.clone();
        match clipboard::copy(&self.scene, &top) {
            Some(clip) => {
                let name = self
                    .path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Untitled".to_string());
                self.begin_save_primitive(clip, name);
            }
            None => self.status = Status::Warning("There is nothing in this project to save".into()),
        }
    }

    fn begin_save_primitive(&mut self, clip: Clip, name: String) {
        self.primitive_name = name;
        self.primitive_clip = Some(clip);
        self.modal = Modal::SavePrimitive;
    }

    /// Write the pending subtree to the library under the typed name.
    pub fn confirm_save_primitive(&mut self) {
        let Some(clip) = self.primitive_clip.clone() else { return };
        let name = self.primitive_name.clone();
        match library::save(&self.config_dir, &name, &clip) {
            Ok(_) => {
                self.primitive_clip = None;
                self.modal = Modal::None;
                self.refresh_library();
                self.status =
                    Status::Info(format!("Saved \u{201C}{}\u{201D} to the palette", library::sanitise(&name)));
            }
            Err(e) => self.fail("Could not save the primitive", &format!("{name}\n\n{e}")),
        }
    }

    pub fn cancel_save_primitive(&mut self) {
        self.primitive_clip = None;
        self.modal = Modal::None;
    }

    /// Drop a saved primitive into the scene, at the current placement.
    pub fn add_library_entry(&mut self, entry: &library::Entry) {
        let Some(clip) = library::load(&entry.path) else {
            return self.fail(
                "Could not read that saved primitive",
                &format!("{}\n\nIt may have been written by a newer version, or edited by hand.", entry.path.display()),
            );
        };
        self.edit("Add", None);
        let at = self.insertion_point_world();
        let target = self.primary();
        let created = clipboard::insert(&mut self.scene, &clip, target, false);
        if created.is_empty() {
            self.status = Status::Warning(format!("\u{201C}{}\u{201D} has nothing in it", entry.name));
            return;
        }
        // The saved subtree keeps its own internal arrangement; what moves is
        // where the whole thing sits.
        let anchor = self.scene.node(created[0]).position;
        for id in &created {
            if let Some(node) = self.scene.get_mut(*id) {
                node.position = node.position - anchor + at;
            }
        }
        self.selection = created;
        self.on_selection_changed();
        self.status = Status::Info(format!("Added {}", entry.name));
    }

    pub fn delete_library_entry(&mut self, entry: &library::Entry) {
        match library::remove(&entry.path) {
            Ok(()) => {
                self.refresh_library();
                self.status = Status::Info(format!("Removed {} from the palette", entry.name));
            }
            Err(e) => self.fail("Could not remove that saved primitive", &format!("{}\n\n{e}", entry.path.display())),
        }
    }

    pub fn gizmo_for(&self, id: NodeId) -> Option<Gizmo> {
        Gizmo::build(&self.scene, &self.evaluated, id, self.mode, self.settings.handle_frame == HandleFrame::World)
    }

    pub fn current_view(&self) -> crate::view::View {
        crate::view::View::new(self.scene.camera, self.viewport_rect)
    }

    // -- export -------------------------------------------------------------

    pub fn start_export(&mut self) {
        let scale = simple3d_core::unit::parse_number(&self.export_scale).unwrap_or(1.0);
        if scale <= 0.0 {
            self.fail("The export scale must be greater than zero", "Enter a positive scale factor.");
            return;
        }
        // A boolean the kernel could not evaluate means the mesh is not
        // trustworthy; refuse with the specific reason and name the node.
        if !self.evaluated.errors.is_empty() {
            let detail = self
                .evaluated
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.name, e.message))
                .collect::<Vec<_>>()
                .join("\n");
            self.fail("Export refused: the scene has unevaluated geometry", &detail);
            return;
        }
        let mesh = if self.export_selection_only {
            let mut merged = simple3d_geom::Mesh::new();
            for id in self.top_level_selection() {
                for node in std::iter::once(id).chain(self.scene.descendants(id)) {
                    if let Some(part) = self.evaluated.node_meshes.get(&node) {
                        if self.scene.node(node).visible {
                            merged.append(part);
                        }
                    }
                }
            }
            std::sync::Arc::new(merged)
        } else {
            self.evaluated.mesh.clone()
        };
        if mesh.triangle_count() == 0 {
            self.fail("There is nothing to export", "The scene, or the selection, has no visible geometry.");
            return;
        }

        let default_name = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "model".to_string());
        let mut dialog = rfd::FileDialog::new()
            .add_filter(self.export_format.label(), &[self.export_format.extension()])
            .set_file_name(format!("{default_name}.{}", self.export_format.extension()));
        if let Some(dir) = self
            .settings
            .last_export_dir
            .clone()
            .or_else(|| self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())))
        {
            dialog = dialog.set_directory(dir);
        }
        let Some(mut path) = dialog.save_file() else { return };
        if path.extension().is_none() {
            path.set_extension(self.export_format.extension());
        }

        self.settings.last_export_dir = path.parent().map(|p| p.to_path_buf());
        self.settings.last_export_format = self.export_format.id().to_string();
        self.settings.last_export_scale = scale;

        let options = simple3d_export::Options {
            format: self.export_format,
            scale,
            unit: simple3d_export::Unit3mf::Millimeter,
            allow_invalid: false,
        };
        self.export_job = Some(ExportJob::spawn(path, mesh, options, EXPORT_LIMIT));
        self.modal = Modal::None;
    }

    fn poll_export(&mut self) {
        let Some(job) = &self.export_job else { return };
        let Some(outcome) = job.poll() else { return };
        let path = job.path.clone();
        let label = job.format_label.clone();
        self.export_job = None;
        match outcome {
            Ok(()) => self.status = Status::Info(format!("Exported {label} to {}", path.display())),
            Err(simple3d_export::ExportError::Cancelled) => {
                self.status = Status::Info("Export cancelled; no file was written".into())
            }
            Err(e) => self.fail("Export failed", &e.to_string()),
        }
    }

    // -- shutdown -----------------------------------------------------------

    pub fn request_quit(&mut self) {
        if self.unsaved() {
            self.modal = Modal::ConfirmQuit;
        } else {
            self.modal = Modal::None;
            self.quit_now = true;
        }
    }

    pub fn confirm_quit(&mut self) {
        self.quit_now = true;
    }

    /// Where this application reads and writes its settings and keymap.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn persist(&mut self) {
        let _ = config::save_settings_to(&self.config_dir, &self.settings);
        self.persist_keymap();
    }

    /// Write the keymap out now, so a rebinding survives even a hard kill --
    /// this is the half of acceptance criterion 28 that happens before the
    /// restart. Called from every place the keymap editor changes something.
    pub fn persist_keymap(&self) {
        let _ = config::save_keymap_to(&self.config_dir, &self.keymap);
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn children_plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "ren"
    }
}

/// Where the next shape will land, in words. The palette says this in its hint
/// line and in every tile's tooltip, so the two can never disagree about it.
pub fn insertion_hint(app: &App) -> String {
    let unit = app.unit();
    let at = app.insertion_point_world();
    let where_ = format!(
        "{}, {}, {} {}",
        simple3d_core::unit::format_length(at.x, unit),
        simple3d_core::unit::format_length(at.y, unit),
        simple3d_core::unit::format_length(at.z, unit),
        unit.suffix()
    );
    match app.settings.placement {
        Placement::Origin => format!("Lands at the origin: {where_}."),
        Placement::Cursor if app.cursor.is_some() => format!("Lands at the 3D cursor: {where_}."),
        Placement::Cursor => {
            format!("Lands at {where_}. Shift+right-click in the viewport to put the 3D cursor somewhere else.")
        }
        Placement::ViewCentre => format!("Lands at what the camera is looking at: {where_}."),
        Placement::BesideSelection => format!("Lands beside the selection: {where_}."),
    }
}

/// How long a message stays at full strength before fading out. Long enough to
/// read twice, short enough that the bar is not still reporting an export that
/// finished ten minutes ago.
pub const STATUS_LIFETIME: Duration = Duration::from_secs(6);

/// How readable a message is now: 1 while it is current, falling to 0 over the
/// second after its lifetime. `Idle` never fades -- "Ready" is a state, not news.
pub fn status_opacity(status: &Status, age: Duration) -> f32 {
    if matches!(status, Status::Idle) {
        return 1.0;
    }
    let over = age.as_secs_f32() - STATUS_LIFETIME.as_secs_f32();
    if over <= 0.0 {
        1.0
    } else {
        (1.0 - over).clamp(0.0, 1.0)
    }
}

impl App {
    /// Everything the window is, in the order it stacks: the bars, the tool
    /// rail, the two docks, the viewport, the drag that may be crossing between
    /// them, and whatever modal is open over the lot.
    ///
    /// Separate from `update` so a test can drive a real frame -- the same
    /// panels in the same order -- against a headless context and replay a
    /// pointer over it. A gesture that is only ever performed by hand is a
    /// gesture nothing checks.
    pub fn ui(&mut self, ctx: &egui::Context) {
        self.menu_bar(ctx);
        self.status_bar(ctx);
        crate::panel_toolrail::show(self, ctx);
        crate::dock::show(self, ctx, Side::Left);
        crate::dock::show(self, ctx, Side::Right);
        panel_viewport::show(self, ctx);
        crate::dock::resolve_drag(self, ctx);
        self.modals(ctx);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // A new message restarts its clock. Watching the value rather than
        // stamping it at every assignment means no `status = ...` anywhere in
        // the application can forget to.
        if self.status != self.last_status {
            self.last_status = self.status.clone();
            self.status_at = std::time::Instant::now();
        }
        if let Some(result) = self.worker.poll() {
            let first = self.evaluation_generation == 0;
            self.evaluated = result;
            self.evaluation_generation += 1;
            if first {
                // The starting scene could not be framed before it had been
                // evaluated, since framing needs its bounds.
                self.frame_all();
            }
            self.scene_renderable = Renderable::prepare(&self.evaluated.mesh);
            self.renderable_key = u64::MAX;
            self.image_key = u64::MAX;
        }
        if self.dirty {
            self.worker.submit(&self.scene);
            self.dirty = false;
        }
        self.poll_export();
        self.advance_camera();
        self.refresh_node_renderables();

        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title()));
        self.handle_shortcuts(ctx);

        self.ui(ctx);

        // Confirmation on quit (spec section 7.4): intercept the window's own
        // close button as well as the Quit command.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit_now {
            if self.unsaved() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.modal = Modal::ConfirmQuit;
            } else {
                self.quit_now = true;
            }
        }
        if self.quit_now {
            self.persist();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // Keep animating while work is in flight, so progress and the preview
        // update without the user having to move the mouse.
        if self.worker.is_busy()
            || self.export_job.is_some()
            || self.drag.is_some()
            || self.camera_move.is_some()
            || status_opacity(&self.status, self.status_at.elapsed()) > 0.0 && self.status != Status::Idle
        {
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simple3d_core::eval::{Cancel, Evaluator};

    /// An `App` on a headless `egui::Context`, which needs no window and no
    /// graphics -- so the command dispatch itself can be driven from a test.
    ///
    /// `App::new` reads the *user's real* settings and keymap, so both are put
    /// back to their defaults here; a test must not change its answer because of
    /// what is in the developer's config directory.
    fn temp_config_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "simple3d-app-test-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// An app with one plate in it. The application itself now opens on an
    /// empty document, so the tests below -- which are about what happens *to*
    /// a shape -- put the shape there themselves.
    fn headless_app() -> App {
        let mut app = app_in(temp_config_dir("headless"));
        let root = app.scene.root();
        let id = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
        app.selection = vec![id];
        app.history.clear();
        app.saved_revision = app.history.revision();
        app.reevaluate_for_test();
        app
    }

    fn app_in(config_dir: PathBuf) -> App {
        let ctx = egui::Context::default();
        let mut app = App::with_config_dir(&ctx, None, config_dir);
        // The gizmo needs a viewport to work out which axes face the screen, and
        // an evaluation to know where the node is.
        app.viewport_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
        app.reevaluate_for_test();
        app
    }

    /// Draw one entire frame of the interface into a headless context. A panel
    /// that panics, or a layout that divides by a width it does not have, fails
    /// here rather than in front of someone.
    fn draw_one_frame(app: &mut App) {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        // A real window size: the default raw input has an effectively infinite
        // screen rect, and the viewport would ask for a texture larger than any
        // GPU allows.
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 880.0))),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    }

    #[test]
    fn every_panel_draws_with_a_selection_and_with_none() {
        // The two states put different panels on screen: with nothing selected
        // the right dock swaps the property panels for the document's own
        // settings, and that path has no other test that ever runs it.
        let mut app = headless_app();
        let id = app.scene.depth_first().into_iter().find(|&id| id != app.scene.root()).unwrap();
        app.select_only(id);
        draw_one_frame(&mut app);
        app.clear_selection();
        draw_one_frame(&mut app);

        // A group selection reaches the boolean panel, which is a third layout
        // again.
        app.select_only(id);
        app.run(Command::Group);
        draw_one_frame(&mut app);
    }

    #[test]
    fn every_modal_draws_and_so_does_a_palette_with_saved_primitives_on_it() {
        // A window that panics, or a layout that divides by a width it does not
        // have, fails here rather than in front of someone.
        let dir = temp_config_dir("modals");
        let mut app = app_in(dir);
        let root = app.scene.root();
        let plate = app.scene.add_primitive("plate", root, 0).unwrap();
        app.select_only(plate);

        // Saving one gives the palette a Saved section to draw, and the naming
        // window something to name.
        app.save_selection_as_primitive();
        draw_one_frame(&mut app);
        app.confirm_save_primitive();
        assert_eq!(app.library.len(), 1);
        draw_one_frame(&mut app);

        for modal in [
            Modal::Export,
            Modal::Keymap,
            Modal::SceneSettings,
            Modal::About,
            Modal::Error,
            Modal::ConfirmQuit,
            Modal::SavePrimitive,
        ] {
            app.modal = modal;
            draw_one_frame(&mut app);
        }
        app.modal = Modal::None;
    }

    #[test]
    fn an_empty_scene_still_draws_every_panel() {
        let mut app = headless_app();
        for id in app.scene.depth_first() {
            if id != app.scene.root() {
                app.scene.remove(id);
            }
        }
        app.clear_selection();
        app.reevaluate_for_test();
        draw_one_frame(&mut app);
    }

    /// Spec acceptance criterion 28, the last link: `App` startup itself picks up
    /// a rebinding written by a previous run.
    ///
    /// `config::a_rebinding_survives_a_restart_and_the_menus_follow` covers the
    /// file round trip; this covers `App::new` actually consulting it, which is
    /// what makes a restart show the new binding.
    #[test]
    fn a_restarted_app_starts_on_the_keymap_the_last_one_saved() {
        let dir = temp_config_dir("restart");

        // First run: rebind something and persist, exactly as the keymap editor does.
        let mut first = app_in(dir.clone());
        assert_eq!(first.keymap, Keymap::default(), "a fresh config dir did not give the default keymap");
        first.keymap.set(Command::Group, Chord::ctrl_shift("J"), true).unwrap();
        first.settings.rotate_snap_deg = 7.5;
        first.persist();
        drop(first);

        // Second run: a new App over the same directory, knowing nothing else.
        let second = app_in(dir.clone());
        assert_eq!(second.keymap.binding(Command::Group), Some(&Chord::ctrl_shift("J")), "the rebinding was lost");
        assert_eq!(second.keymap.command_for(&Chord::ctrl_shift("J")), Some(Command::Group));
        assert_eq!(second.settings.rotate_snap_deg, 7.5, "settings did not persist either");
        // What the menus render is the saved binding, not the default.
        assert_ne!(
            second.keymap.shortcut_text(Command::Group),
            Keymap::default().shortcut_text(Command::Group),
            "the menus would still show the default binding"
        );

        // And nothing was written outside the directory we handed it.
        assert_eq!(second.config_dir(), dir.as_path());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_new_document_is_empty_and_unmodified() {
        // A shape nobody asked for is a shape they have to notice and delete,
        // and the palette is one click away. What matters as much: an untouched
        // new document has nothing to save, so quitting it asks no question.
        let mut app = app_in(temp_config_dir("empty-start"));
        assert_eq!(app.scene.depth_first(), vec![app.scene.root()], "something was added to the new document");
        assert!(app.primary().is_none());
        assert!(!app.unsaved(), "an untouched new document already counts as modified");

        // And File > New, from a document that does have something in it, gets
        // back to exactly that.
        let root = app.scene.root();
        app.scene.add_primitive("box", root, 0).unwrap();
        app.run(Command::New);
        assert_eq!(app.scene.depth_first(), vec![app.scene.root()]);
        assert!(!app.unsaved());
    }

    #[test]
    fn the_step_governs_a_nudge_and_is_not_the_grid_spacing() {
        // The two used to be one setting, so a step of 1 mm meant a 1 mm ground
        // grid -- which is a solid block of lines. They are separate now, and
        // it is the step that a nudge moves by.
        let mut app = headless_app();
        let id = app.primary().unwrap();
        assert_eq!(app.move_snap(), 1.0, "the default step is one millimetre");

        app.scene.settings.snap_step = 2.5;
        app.scene.settings.grid_spacing = 10.0;
        let before = app.scene.node(id).position;
        app.run(Command::NudgeRight);
        let moved = app.scene.node(id).position - before;
        assert!((moved.length() - 2.5).abs() < 1e-9, "a nudge went {} rather than the 2.5 mm step", moved.length());
        assert_eq!(app.scene.settings.grid_spacing, 10.0, "the step changed the grid spacing with it");
    }

    #[test]
    fn each_origin_axis_has_its_own_switch() {
        let mut app = headless_app();
        assert_eq!(app.scene.settings.axes_visible, [true; 3], "the axes start shown");
        app.run(Command::ToggleAxisY);
        assert_eq!(app.scene.settings.axes_visible, [true, false, true], "toggling Y touched another axis");
        app.run(Command::ToggleAxisY);
        assert_eq!(app.scene.settings.axes_visible, [true; 3]);
    }

    #[test]
    fn a_group_saved_as_a_primitive_comes_back_into_a_fresh_document() {
        // The library is per user, not per project: what is saved out of one
        // document is on the palette of the next one.
        let dir = temp_config_dir("library");
        let mut app = app_in(dir.clone());
        let root = app.scene.root();
        let plate = app.scene.add_primitive("plate", root, 0).unwrap();
        app.select_only(plate);
        app.run(Command::Group);
        let group = app.primary().unwrap();
        if let Some(node) = app.scene.get_mut(group) {
            node.name = "Bracket".into();
        }

        app.save_selection_as_primitive();
        assert_eq!(app.modal, Modal::SavePrimitive, "saving did not ask for a name");
        assert_eq!(app.primitive_name, "Bracket", "the name was not offered from the node");
        app.confirm_save_primitive();
        assert_eq!(app.modal, Modal::None);
        assert_eq!(app.library.len(), 1, "the palette did not pick the new entry up");

        // A second application on the same config directory -- which is what
        // opening the program again is -- has it on the palette.
        let mut fresh = app_in(dir);
        assert_eq!(fresh.library.len(), 1);
        let entry = fresh.library[0].clone();
        assert_eq!(entry.name, "Bracket");

        fresh.settings.placement = Placement::Origin;
        fresh.add_library_entry(&entry);
        let added = fresh.primary().expect("nothing was added");
        assert_eq!(fresh.scene.node(added).name, "Bracket", "it arrived under some other name");
        assert!(fresh.scene.node(added).is_group());
        assert_eq!(fresh.scene.node(added).children.len(), 1, "the group arrived without its child");
        assert_eq!(fresh.scene.node(added).position, Vec3::ZERO);

        // And it can be taken off the palette again.
        fresh.delete_library_entry(&entry);
        assert!(fresh.library.is_empty());
    }

    #[test]
    fn a_saved_primitive_lands_at_the_placement_rather_than_where_it_was_saved_from() {
        let dir = temp_config_dir("library-placement");
        let mut app = app_in(dir.clone());
        let root = app.scene.root();
        let plate = app.scene.add_primitive("plate", root, 0).unwrap();
        app.scene.get_mut(plate).unwrap().position = Vec3::new(70.0, 0.0, 0.0);
        app.select_only(plate);
        app.save_selection_as_primitive();
        app.confirm_save_primitive();

        let mut fresh = app_in(dir);
        fresh.settings.placement = Placement::Cursor;
        fresh.cursor = Some(Vec3::new(-5.0, 8.0, 0.0));
        let entry = fresh.library[0].clone();
        fresh.add_library_entry(&entry);
        assert_eq!(fresh.scene.node(fresh.primary().unwrap()).position, Vec3::new(-5.0, 8.0, 0.0));
    }

    /// The default `App::new` still points at the user's real config directory --
    /// the test seam must not have changed where a shipped binary looks.
    #[test]
    fn the_default_config_directory_is_the_users_own() {
        let ctx = egui::Context::default();
        let app = App::new(&ctx, None);
        assert_eq!(app.config_dir(), config::config_dir().as_path());
    }

    impl App {
        fn reevaluate_for_test(&mut self) {
            self.evaluated = Evaluator::new().evaluate(&self.scene, &Cancel::new());
        }
    }

    #[test]
    fn deleting_a_group_asks_what_should_happen_to_its_children() {
        // Two readings of one word, so it is asked rather than guessed.
        let mut app = headless_app();
        let plate = app.scene.depth_first().into_iter().find(|&id| id != app.scene.root()).unwrap();
        app.select_only(plate);
        app.run(Command::Group);
        let group = app.primary().unwrap();
        assert!(app.scene.node(group).is_group());

        app.run(Command::Delete);
        assert!(app.pending_delete.is_some(), "a group with children was deleted without asking");
        assert!(app.scene.contains(group), "the model changed before the question was answered");
        assert_eq!(app.pending_delete_count(), 2, "the group and its one child");

        // Cancelling leaves everything alone.
        app.cancel_delete();
        assert!(app.pending_delete.is_none());
        assert!(app.scene.contains(group) && app.scene.contains(plate));

        // Keeping the children promotes them into the group's own place.
        app.select_only(group);
        app.run(Command::Delete);
        app.confirm_delete(true);
        assert!(!app.scene.contains(group), "the group survived");
        assert!(app.scene.contains(plate), "the child was deleted despite being kept");
        assert_eq!(app.scene.node(plate).parent, Some(app.scene.root()), "the child was not promoted");

        // And taking the children takes them.
        app.select_only(plate);
        app.run(Command::Group);
        let group = app.primary().unwrap();
        app.run(Command::Delete);
        app.confirm_delete(false);
        assert!(!app.scene.contains(group) && !app.scene.contains(plate));

        // A shape on its own is not a question, so it just goes.
        let root = app.scene.root();
        let lone = app.scene.add_primitive("sphere", root, 0).unwrap();
        app.select_only(lone);
        app.run(Command::Delete);
        assert!(app.pending_delete.is_none(), "deleting one shape asked a question it did not need to");
        assert!(!app.scene.contains(lone));
    }

    #[test]
    fn a_new_shape_lands_where_the_placement_says_and_the_hint_agrees() {
        let mut app = headless_app();
        assert_eq!(app.settings.placement, Placement::Origin);
        assert!(insertion_hint(&app).contains("origin"), "{}", insertion_hint(&app));
        app.add_node(Some("sphere"), GroupOp::Union);
        assert_eq!(app.scene.node(app.primary().unwrap()).position, Vec3::ZERO);

        app.settings.placement = Placement::Cursor;
        app.cursor = Some(Vec3::new(30.0, -10.0, 5.0));
        let hint = insertion_hint(&app);
        assert!(hint.contains("cursor") && hint.contains("30"), "{hint}");
        app.add_node(Some("sphere"), GroupOp::Union);
        assert_eq!(app.scene.node(app.primary().unwrap()).position, Vec3::new(30.0, -10.0, 5.0));

        // What the camera is looking at, rounded to the step.
        app.settings.placement = Placement::ViewCentre;
        app.scene.settings.snap_step = 5.0;
        app.scene.camera.target = Vec3::new(12.0, -3.0, 0.0);
        assert_eq!(app.insertion_point_world(), Vec3::new(10.0, -5.0, 0.0));
        assert!(insertion_hint(&app).contains("camera"), "{}", insertion_hint(&app));

        // Beside the selection: clear of it, not inside it.
        app.clear_selection();
        let plate = app.scene.depth_first().into_iter().find(|&id| id != app.scene.root()).unwrap();
        app.select_only(plate);
        app.reevaluate_for_test();
        app.settings.placement = Placement::BesideSelection;
        let (_, hi) = app.selection_bounds().expect("the plate has bounds");
        assert_eq!(app.insertion_point_world().x, hi.x + app.move_snap());
    }

    #[test]
    fn a_shape_added_into_a_rotated_group_still_lands_where_the_placement_says() {
        // `Node::position` is in the parent's coordinates. Writing a world point
        // into it unchanged would put the shape somewhere else entirely as soon
        // as the group it goes into is turned or moved.
        let mut app = headless_app();
        let plate = app.primary().unwrap();
        app.select_only(plate);
        app.run(Command::Group);
        let group = app.primary().unwrap();
        if let Some(node) = app.scene.get_mut(group) {
            node.position = Vec3::new(50.0, 0.0, 0.0);
            node.rotation = Vec3::new(0.0, 0.0, 90.0);
        }
        app.reevaluate_for_test();

        app.settings.placement = Placement::Cursor;
        app.cursor = Some(Vec3::new(10.0, 0.0, 0.0));
        app.select_only(group);
        app.add_node(Some("sphere"), GroupOp::Union);
        let added = app.primary().unwrap();
        assert_eq!(app.scene.node(added).parent, Some(group), "the sphere did not go into the group");

        app.reevaluate_for_test();
        let world = app.evaluated.node_meshes[&added].bounds().expect("the sphere has bounds");
        let centre = (world.0 + world.1) * 0.5;
        assert!((centre - Vec3::new(10.0, 0.0, 0.0)).length() < 1e-6, "the sphere landed at {centre:?}");
    }

    #[test]
    fn a_message_fades_once_it_has_been_read_and_ready_never_does() {
        let message = Status::Info("Saved".into());
        assert_eq!(status_opacity(&message, Duration::from_secs(0)), 1.0);
        assert_eq!(status_opacity(&message, STATUS_LIFETIME), 1.0);
        assert!(status_opacity(&message, STATUS_LIFETIME + Duration::from_millis(500)) < 1.0);
        assert_eq!(status_opacity(&message, STATUS_LIFETIME + Duration::from_secs(2)), 0.0);
        // "Ready" is the state of the application, not news about it.
        assert_eq!(status_opacity(&Status::Idle, Duration::from_secs(600)), 1.0);
    }

    #[test]
    fn the_view_cube_and_the_view_menu_reach_the_same_camera() {
        let mut app = headless_app();
        app.settings.reduce_motion = true;
        for (normal, preset, label) in crate::view::CUBE_FACES {
            app.run(Command::ViewIsometric);
            app.set_view(preset);
            // The angles may differ by a whole turn -- the camera takes the
            // short way round, so "left" can arrive at -180 rather than 180 --
            // so compare where the eye ends up, which is the thing that matters.
            let looking = app.current_view().offset_dir();
            let face = Vec3::new(normal[0] as f64, normal[1] as f64, normal[2] as f64);
            assert!(looking.dot(face) > 0.99, "{label}: the camera ended up at {looking:?}");
        }

        // With motion allowed the camera is on its way rather than already
        // there, and it gets there.
        app.settings.reduce_motion = false;
        app.run(Command::ViewIsometric);
        app.advance_camera();
        assert!(app.camera_move.is_some(), "the transition never started");
        let target = app.camera_move.unwrap().to;
        app.camera_move = Some(CameraMove {
            started: std::time::Instant::now() - crate::view::TRANSITION,
            ..app.camera_move.unwrap()
        });
        app.advance_camera();
        assert!(app.camera_move.is_none(), "the transition never finished");
        assert!((app.scene.camera.yaw - target.0).abs() < 1e-9);
    }

    #[test]
    fn every_panel_still_draws_wherever_it_has_been_docked() {
        // Panels are movable, so the layouts that used to be impossible -- a
        // dock with nothing in it, three panels in one column, everything rolled
        // up -- are now reachable and have to draw.
        use simple3d_core::config::{Panel, Side};
        let mut app = headless_app();
        for panel in Panel::ALL {
            app.settings.layout.move_to(panel, Side::Right, 0);
        }
        draw_one_frame(&mut app);
        for panel in Panel::ALL {
            app.settings.layout.toggle_collapsed(panel);
        }
        draw_one_frame(&mut app);
        app.settings.layout.docks_hidden = true;
        draw_one_frame(&mut app);
        app.run(Command::ResetLayout);
        assert_eq!(app.settings.layout, simple3d_core::config::Layout::default());
        draw_one_frame(&mut app);
    }

    /// Spec acceptance criterion 19: the application starts and stays usable on a
    /// machine with no accelerated graphics.
    ///
    /// There is none here -- no GPU, no window, no display -- and this is the
    /// whole of `App::new`: settings, the evaluation worker, the starter scene and
    /// the first frame's worth of state. `raster.rs`'s tests cover the drawing
    /// that follows being done on the CPU; this covers the starting.
    #[test]
    fn the_application_starts_with_no_graphics_at_all() {
        let mut app = headless_app();
        assert_eq!(app.status, Status::Idle);
        assert!(app.evaluated.errors.is_empty(), "{:?}", app.evaluated.errors);

        // And it stays usable: a command runs and takes effect.
        app.run(Command::Duplicate);
        assert_eq!(app.scene.depth_first().len(), 3, "root, plate and its duplicate");
        app.run(Command::Undo);
        assert_eq!(app.scene.depth_first().len(), 2);
    }

    /// Drive a whole manipulator gesture the way `panel_viewport::manipulate`
    /// does: one `Begin`, `frames` × `Continue` along the way, then `Finish`.
    /// Returns the cursor positions used, so a test can aim the drag.
    fn drag_gesture(app: &mut App, id: NodeId, handle: Handle, to: Vec3, frames: usize) {
        let view = app.current_view();
        let start_gizmo = app.gizmo_for(id).expect("a gizmo for the dragged node");
        let from = view.project(start_gizmo.handle_point(handle, &view)).unwrap().0;
        let target = view.project(to).unwrap().0;

        app.hover_handle = Some(handle);
        let pointer = gizmo::PointerState { started: true, on_handle: true, have_cursor: true, ..Default::default() };
        let phase = gizmo::drag_phase(false, pointer);
        assert_eq!(phase, gizmo::DragPhase::Begin);
        app.manipulate_step(&start_gizmo, &view, id, phase, Some(handle), Some(from), gizmo::Mods::default());

        for frame in 1..=frames {
            let t = frame as f32 / frames as f32;
            let at = from + (target - from) * t;
            let pointer = gizmo::PointerState { have_cursor: true, ..Default::default() };
            let phase = gizmo::drag_phase(true, pointer);
            assert_eq!(phase, gizmo::DragPhase::Continue);
            // Rebuilt every frame, exactly as `panel_viewport::manipulate` does.
            // A drag that measured against this rather than the frame it began
            // in would chase its own tail; see the note on `gizmo::Drag::gizmo`.
            let live = app.gizmo_for(id).expect("a gizmo mid-drag");
            app.manipulate_step(&live, &view, id, phase, Some(handle), Some(at), gizmo::Mods::default());
        }

        let live = app.gizmo_for(id).expect("a gizmo at the end of the drag");
        let pointer = gizmo::PointerState { released: true, have_cursor: true, ..Default::default() };
        let phase = gizmo::drag_phase(true, pointer);
        assert_eq!(phase, gizmo::DragPhase::Finish);
        app.manipulate_step(&live, &view, id, phase, Some(handle), Some(target), gizmo::Mods::default());
    }

    /// A drag must land where the cursor left it, however many frames it took --
    /// and land in the *same* place whether that number is odd or even.
    ///
    /// This is the regression test for a bug found by driving the running app:
    /// `Drag::update` measured the cursor against the live gizmo, which is
    /// rebuilt each frame from the node the drag is moving. Once the node had
    /// moved, the reference had moved with it, and the next frame wrote the node
    /// back to its starting point -- so the position flipped between the two on
    /// alternate frames and the drag finished wherever the button happened to
    /// come up. A single-frame drag, which is all the older tests did, cannot see
    /// it.
    #[test]
    fn a_drag_lands_in_the_same_place_however_many_frames_it_took() {
        // Where each drag aims, taken from a scene in its starting state.
        let (target, corner) = {
            let app = headless_app();
            let id = app.primary().unwrap();
            let gizmo = app.gizmo_for(id).unwrap();
            (gizmo.origin + Vec3::new(30.0, 0.0, 0.0), gizmo.origin + Vec3::new(25.0, 15.0, 0.0))
        };

        // Every handle kind, since the frozen frame is what all of them measure
        // against now.
        for (handle, to) in [
            (Handle::MoveAxis(0), target),
            (Handle::MovePlane(2), corner),
            (Handle::RotateRing(2), corner),
            (Handle::ResizeFace(0, true), target),
            (Handle::ResizeCorner([true, true, true]), corner),
        ] {
            let mut landed = Vec::new();
            for frames in [1usize, 2, 3, 4, 5, 20, 21] {
                let mut app = headless_app();
                let id = app.primary().unwrap();
                drag_gesture(&mut app, id, handle, to, frames);
                app.reevaluate_for_test();
                let node = app.scene.node(id);
                landed.push((frames, node.position, node.rotation, node.params().cloned()));
            }
            let first = &landed[0];
            for entry in &landed {
                assert_eq!(
                    (entry.1, entry.2, &entry.3),
                    (first.1, first.2, &first.3),
                    "{handle:?}: a {}-frame drag landed somewhere a {}-frame drag did not",
                    entry.0,
                    first.0
                );
            }
        }
    }

    /// Spec acceptance criterion 23's last clause: a completed drag undoes in one
    /// step, however many frames it took.
    ///
    /// This is the clause that had no test, because the bookkeeping lived inside
    /// a function driven entirely by an `egui::Response`. The gesture below is
    /// twenty frames long and must leave exactly one undo step behind.
    #[test]
    fn a_completed_drag_undoes_in_one_step_however_many_frames_it_took() {
        let mut app = headless_app();
        let id = app.primary().unwrap();
        let start = app.scene.node(id).position;

        // A recorded edit before the drag, so "one step" is not "the stack
        // emptied". `Rename` only opens the editor, so it is not one.
        app.edit("Before", None);
        let before = app.history.revision();

        let handle = Handle::MoveAxis(0);
        let origin = app.gizmo_for(id).unwrap().origin;
        drag_gesture(&mut app, id, handle, origin + Vec3::new(30.0, 0.0, 0.0), 20);
        app.reevaluate_for_test();

        let moved = app.scene.node(id).position;
        assert_ne!(moved, start, "the drag did not move anything");
        assert!(app.history.revision() > before);

        app.run(Command::Undo);
        assert_eq!(app.scene.node(id).position, start, "one undo did not take back the whole drag");
        assert_eq!(app.history.undo_label(), Some("Before"), "the drag left more than one undo step");

        app.run(Command::Redo);
        assert_eq!(app.scene.node(id).position, moved, "redo did not put the drag back in one");
    }

    /// Escape mid-drag restores the pre-drag position *and* leaves no undo step
    /// behind: the snapshot taken when the drag opened describes exactly the
    /// state the cancel just restored, so an undo afterwards would do nothing
    /// visible and the user would have to press it twice to get anywhere.
    #[test]
    fn a_cancelled_drag_leaves_no_undo_step_behind() {
        let mut app = headless_app();
        let id = app.primary().unwrap();

        app.edit("Before", None);
        let steps_before = app.history.undo_label().map(str::to_string);
        let start = app.scene.node(id).position;

        let handle = Handle::MoveAxis(0);
        let gizmo = app.gizmo_for(id).unwrap();
        let view = app.current_view();
        let from = view.project(gizmo.handle_point(handle, &view)).unwrap().0;
        let to = view.project(gizmo.origin + Vec3::new(30.0, 0.0, 0.0)).unwrap().0;

        app.hover_handle = Some(handle);
        app.manipulate_step(&gizmo, &view, id, gizmo::DragPhase::Begin, Some(handle), Some(from), Default::default());
        app.manipulate_step(&gizmo, &view, id, gizmo::DragPhase::Continue, Some(handle), Some(to), Default::default());
        assert_ne!(app.scene.node(id).position, start, "the drag never got going, so cancelling proves nothing");

        app.manipulate_step(&gizmo, &view, id, gizmo::DragPhase::Cancel, Some(handle), Some(to), Default::default());
        assert_eq!(app.scene.node(id).position, start, "Escape did not restore the pre-drag position exactly");
        assert!(app.drag.is_none());
        assert_eq!(app.history.undo_label().map(str::to_string), steps_before, "the cancelled drag left an undo step");

        // So the next undo reaches the edit before the drag, not a dead step.
        app.run(Command::Undo);
        assert!(app.history.undo_label().is_none());
    }

    /// The phase ordering is what keeps one gesture to one undo step, so the
    /// cases that could open a second are worth pinning down.
    #[test]
    fn a_second_undo_step_cannot_open_mid_gesture() {
        use gizmo::{drag_phase, DragPhase, PointerState};

        let grab = PointerState { started: true, on_handle: true, have_cursor: true, ..Default::default() };
        assert_eq!(drag_phase(false, grab), DragPhase::Begin);
        // The same frame's facts, once a drag is running, must never be Begin
        // again -- that is the second snapshot that would split the gesture.
        assert_eq!(drag_phase(true, grab), DragPhase::Continue);

        // Escape beats release, so abandoning is never read as completing.
        let both = PointerState { escape: true, released: true, have_cursor: true, ..Default::default() };
        assert_eq!(drag_phase(true, both), DragPhase::Cancel);

        // A press that did not land on a handle starts nothing.
        assert_eq!(
            drag_phase(false, PointerState { started: true, have_cursor: true, ..Default::default() }),
            DragPhase::Idle
        );
        // The pointer leaving the window pauses the drag rather than ending it.
        assert_eq!(drag_phase(true, PointerState::default()), DragPhase::Idle);
        assert_eq!(drag_phase(false, PointerState::default()), DragPhase::Idle);
    }

    /// Spec acceptance criterion 26, from the command a keypress actually
    /// dispatches: hold an arrow key, every repeat steps by the snap increment,
    /// and the whole run undoes in one.
    ///
    /// `gizmo::a_held_nudge_run_steps_by_the_snap_and_undoes_in_one` covers the
    /// arithmetic; this covers the wiring from `App::run` to it, which is the
    /// part a keypress reaches.
    #[test]
    fn holding_an_arrow_key_nudges_by_the_snap_and_undoes_in_one_step() {
        const PRESSES: usize = 8;

        let mut app = headless_app();
        let id = app.primary().expect("the starter scene leaves a plate selected");
        let snap = app.move_snap();
        assert_eq!(snap, 1.0, "the default step changed; this test's arithmetic assumes it");
        let start = app.scene.node(id).position;

        // Something before the run, so "one undo step" is distinguishable from
        // "undo emptied the stack".
        app.run(Command::Rename);
        let before_run = app.history.revision();

        for _ in 0..PRESSES {
            app.run(Command::NudgeRight);
            app.reevaluate_for_test();
        }
        assert!(app.history.revision() > before_run, "the run recorded nothing at all");

        let travelled = (app.scene.node(id).position - start).length();
        assert!(
            (travelled - snap * PRESSES as f64).abs() < 1e-6,
            "{PRESSES} presses travelled {travelled} mm, expected {}",
            snap * PRESSES as f64
        );

        app.run(Command::Undo);
        assert_eq!(app.scene.node(id).position, start, "one undo did not restore the whole run");
        app.run(Command::Redo);
        assert!((app.scene.node(id).position - start).length() > snap, "redo did not put the run back");
    }

    /// Nudging in rotate and resize mode goes through the same key, and a mode
    /// switch mid-way must not be swallowed into the previous run's undo step.
    #[test]
    fn switching_mode_starts_a_new_nudge_undo_step() {
        let mut app = headless_app();
        let id = app.primary().unwrap();

        app.run(Command::ModeMove);
        app.run(Command::NudgeRight);
        app.reevaluate_for_test();
        let moved = app.scene.node(id).position;

        app.run(Command::ModeRotate);
        app.run(Command::NudgeRight);
        app.reevaluate_for_test();
        assert_ne!(app.scene.node(id).rotation, Vec3::ZERO, "a rotate-mode nudge did not rotate");

        // One undo takes back the rotation only.
        app.run(Command::Undo);
        assert_eq!(app.scene.node(id).rotation, Vec3::ZERO);
        assert_eq!(app.scene.node(id).position, moved, "the rotation and the move shared an undo step");
    }

    /// Spec acceptance criterion 20's last clause, which is `App`'s to keep: a
    /// pasted copy is left selected, so a nudge or a drag can follow immediately.
    #[test]
    fn a_pasted_copy_is_left_selected() {
        let mut app = headless_app();
        let original = app.primary().unwrap();

        app.run(Command::Copy);
        app.run(Command::Paste);

        let pasted = app.primary().expect("nothing is selected after a paste");
        assert_ne!(pasted, original, "the paste left the original selected, not the copy");
        assert_eq!(app.selection, vec![pasted], "the copy is not the whole selection");
        assert_eq!(app.scene.node(pasted).position, app.scene.node(original).position);

        // And it really is usable straight away: a nudge acts on the copy.
        app.reevaluate_for_test();
        let start = app.scene.node(pasted).position;
        app.run(Command::NudgeRight);
        assert_ne!(app.scene.node(pasted).position, start, "the pasted copy could not be nudged");
        assert_eq!(app.scene.node(original).position, start, "nudging the copy moved the original");
    }
}
