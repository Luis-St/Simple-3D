//! The application: window layout, command dispatch, file handling and the
//! glue that keeps the outliner, property editor and viewport in step
//! (spec section 7).

use crate::gizmo::{self, Drag, Gizmo, Handle, Mode};
use crate::panel_outliner;
use crate::panel_properties;
use crate::panel_viewport;
use crate::render::Renderable;
use crate::ui::FieldBuffers;
use crate::view::{frame_bounds, ViewPreset};
use crate::worker::{EvalWorker, ExportJob};
use scadstudio_core::clipboard::{self, Clip};
use scadstudio_core::config::{self, AppSettings, DisplayMode, HandleFrame};
use scadstudio_core::eval::Evaluated;
use scadstudio_core::keymap::{Chord, Command, Keymap};
use scadstudio_core::project;
use scadstudio_core::scene::{GroupOp, NodeId, Scene};
use scadstudio_core::undo::History;
use scadstudio_core::unit::Unit;
use scadstudio_export::Format;
use scadstudio_geom::Vec3;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const APP_NAME: &str = "ScadStudio";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PROJECT_EXTENSION: &str = "scadstudio";
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
    pub viewport_rect: egui::Rect,
    pub texture: Option<egui::TextureHandle>,
    pub image_key: u64,

    pub path: Option<PathBuf>,
    saved_revision: u64,

    pub status: Status,
    pub fields: FieldBuffers,
    pub rename: Option<(NodeId, String)>,
    pub outliner_drag: Option<NodeId>,
    pub drop_target: Option<DropTarget>,

    pub export_job: Option<ExportJob>,
    pub export_format: Format,
    pub export_scale: String,
    pub export_selection_only: bool,

    pub modal: Modal,
    pub error_title: String,
    pub error_detail: String,

    pub keymap_search: String,
    pub recording: Option<Command>,
    pub keymap_conflict: Option<(Command, Chord, Command)>,

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
        ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(6.0, 5.0);
            style.spacing.button_padding = egui::vec2(7.0, 3.0);
        });
        let settings = config::load_settings();
        let keymap = config::load_keymap();
        let mut app = App {
            scene: Scene::new(),
            history: History::new(),
            settings,
            keymap,
            selection: Vec::new(),
            clipboard: None,
            worker: EvalWorker::spawn(),
            evaluated: Evaluated {
                mesh: std::sync::Arc::new(scadstudio_geom::Mesh::new()),
                node_meshes: BTreeMap::new(),
                node_frames: BTreeMap::new(),
                node_local_bounds: BTreeMap::new(),
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
            viewport_rect: egui::Rect::NOTHING,
            texture: None,
            image_key: u64::MAX,
            path: None,
            saved_revision: 0,
            status: Status::Idle,
            fields: FieldBuffers::default(),
            rename: None,
            outliner_drag: None,
            drop_target: None,
            export_job: None,
            export_format: Format::ThreeMf,
            export_scale: "1".to_string(),
            export_selection_only: false,
            modal: Modal::None,
            error_title: String::new(),
            error_detail: String::new(),
            keymap_search: String::new(),
            recording: None,
            keymap_conflict: None,
            nudging: false,
            quit_now: false,
        };
        app.export_format = Format::from_id(&app.settings.last_export_format).unwrap_or(Format::ThreeMf);
        app.export_scale = scadstudio_core::unit::format_number(app.settings.last_export_scale, 4);
        match open {
            // Opening a project by passing its path on the command line, so file
            // associations work on both platforms (spec section 10).
            Some(path) => app.open_path(&path),
            None => app.starter_scene(),
        }
        app
    }

    /// A single plate, so the window is not empty on first run and the user has
    /// something to type numbers into immediately.
    fn starter_scene(&mut self) {
        let root = self.scene.root();
        if let Some(id) = self.scene.add_primitive("plate", root, 0) {
            self.selection = vec![id];
        }
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

    /// The move and resize snap increment: the grid spacing (spec section 6.2).
    pub fn move_snap(&self) -> f64 {
        self.scene.settings.grid_spacing
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
        let mut dialog = rfd::FileDialog::new().add_filter("ScadStudio project", &[PROJECT_EXTENSION]);
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
            .add_filter("ScadStudio project", &[PROJECT_EXTENSION])
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
        self.scene.camera.yaw = yaw;
        self.scene.camera.pitch = pitch;
        self.status = Status::Info(format!("View: {}", preset.label()));
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
            DisplayShaded => self.settings.display_mode = DisplayMode::Shaded,
            DisplayShadedEdges => self.settings.display_mode = DisplayMode::ShadedWithEdges,
            DisplayWireframe => self.settings.display_mode = DisplayMode::Wireframe,
            ToggleBoundingBox => self.settings.show_bounding_box = !self.settings.show_bounding_box,
            ToggleGhosts => self.settings.show_ghosts = !self.settings.show_ghosts,

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
        self.edit("Delete", None);
        for id in &targets {
            self.scene.remove(*id);
        }
        self.clear_selection();
        self.status = Status::Info(format!("Deleted {} node{}", targets.len(), if targets.len() == 1 { "" } else { "s" }));
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
        let [horizontal, vertical, third] = gizmo::screen_aligned_axes(&gizmo, &view);

        let (axis, mut sign) = match command {
            Command::NudgeLeft => (horizontal, -1.0),
            Command::NudgeRight => (horizontal, 1.0),
            Command::NudgeDown => (vertical, -1.0),
            Command::NudgeUp => (vertical, 1.0),
            Command::NudgeToward => (third, -1.0),
            Command::NudgeAway => (third, 1.0),
            _ => return,
        };
        // Match the axis's screen direction, so "left" really goes left.
        let vertical_key = matches!(command, Command::NudgeUp | Command::NudgeDown);
        if matches!(command, Command::NudgeLeft | Command::NudgeRight) || vertical_key {
            sign *= gizmo::axis_screen_sign(&gizmo, &view, axis, vertical_key);
        }

        // A run of repeats coalesces into a single undo step.
        let coalesce = format!("nudge:{id}:{:?}", self.mode);
        self.edit("Nudge", Some(&coalesce));
        self.nudging = true;

        match self.mode {
            Mode::Move => {
                let step = self.move_snap() * sign;
                let world_delta = gizmo.axes[axis] * step;
                let target = gizmo.parent.point(self.scene.node(id).position) + world_delta;
                let local = gizmo.parent.inverse().point(target);
                if let Some(node) = self.scene.get_mut(id) {
                    node.position = local;
                }
            }
            Mode::Rotate => {
                let step = self.settings.rotate_snap_deg * sign;
                if let Some(node) = self.scene.get_mut(id) {
                    let current = gizmo::get_axis(node.rotation, axis);
                    let mut rotation = node.rotation;
                    gizmo::set_axis(&mut rotation, axis, current + step);
                    node.rotation = rotation;
                }
            }
            Mode::Resize => {
                let Some(driver) = gizmo.drivers[axis] else {
                    self.status = Status::Info(format!(
                        "{} has no dimension on the {} axis",
                        self.scene.node(id).name,
                        gizmo::axis_name(axis)
                    ));
                    return;
                };
                let extent = gizmo::get_axis(gizmo.local_hi, axis) - gizmo::get_axis(gizmo.local_lo, axis);
                let target = (extent + self.move_snap() * sign).max(1e-3);
                let value = target / driver.factor;
                if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
                    params.insert(
                        driver.param.to_string(),
                        scadstudio_core::primitive::ParamValue::Length(value),
                    );
                }
            }
        }
        self.fields.clear();
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
        let created = match type_id {
            Some(type_id) => self.scene.add_primitive(type_id, parent, index),
            None => Some(self.scene.add_group(op, parent, index)),
        };
        match created {
            Some(id) => {
                self.select_only(id);
                self.status = Status::Info(format!("Added {}", self.scene.node(id).name));
            }
            None => self.status = Status::Warning("Unknown primitive type".into()),
        }
    }

    pub fn gizmo_for(&self, id: NodeId) -> Option<Gizmo> {
        Gizmo::build(
            &self.scene,
            &self.evaluated,
            id,
            self.mode,
            self.settings.handle_frame == HandleFrame::World,
        )
    }

    pub fn current_view(&self) -> crate::view::View {
        crate::view::View::new(self.scene.camera, self.viewport_rect)
    }

    // -- export -------------------------------------------------------------

    pub fn start_export(&mut self) {
        let scale = scadstudio_core::unit::parse_number(&self.export_scale).unwrap_or(1.0);
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
            let mut merged = scadstudio_geom::Mesh::new();
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
        if let Some(dir) = self.settings.last_export_dir.clone().or_else(|| self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()))) {
            dialog = dialog.set_directory(dir);
        }
        let Some(mut path) = dialog.save_file() else { return };
        if path.extension().is_none() {
            path.set_extension(self.export_format.extension());
        }

        self.settings.last_export_dir = path.parent().map(|p| p.to_path_buf());
        self.settings.last_export_format = self.export_format.id().to_string();
        self.settings.last_export_scale = scale;

        let options = scadstudio_export::Options {
            format: self.export_format,
            scale,
            unit: scadstudio_export::Unit3mf::Millimeter,
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
            Err(scadstudio_export::ExportError::Cancelled) => {
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

    pub fn persist(&mut self) {
        let _ = config::save_settings(&self.settings);
        let _ = config::save_keymap(&self.keymap);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
        self.refresh_node_renderables();

        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title()));
        self.handle_shortcuts(ctx);

        self.menu_bar(ctx);
        self.status_bar(ctx);
        panel_outliner::show(self, ctx);
        panel_properties::show(self, ctx);
        panel_viewport::show(self, ctx);
        self.modals(ctx);

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
        if self.worker.is_busy() || self.export_job.is_some() || self.drag.is_some() {
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist();
    }
}
