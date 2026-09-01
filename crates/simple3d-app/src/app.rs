//! The application: window layout, command dispatch, file handling and the
//! glue that keeps the outliner, property editor and viewport in step
//! (spec section 7).

use crate::gizmo::{self, Drag, Gizmo, Handle, Mode};
use crate::panel_viewport;
use crate::render::Renderable;
use crate::ui::{self, FieldBuffers};
use crate::view::{frame_bounds, CameraMove, ViewPreset};
use crate::worker::{EvalWorker, ExportJob};
use simple3d_core::clipboard::{self, Clip};
use simple3d_core::config::{self, AppSettings, DisplayMode, HandleFrame, Placement, Side, SnapMode};
use simple3d_core::eval::Evaluated;
use simple3d_core::keymap::{Chord, Command, Keymap};
use simple3d_core::library;
use simple3d_core::project;
use simple3d_core::scene::{Colour, GroupOp, NodeId, Scene};
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
    About,
    /// A failure worth stopping for, shown in a scrollable, copyable window.
    Error,
    /// Naming a group, or a whole project, to keep on the palette.
    SavePrimitive,
    /// Quitting with unsaved changes.
    ConfirmQuit,
    /// Closing a tab with unsaved changes (issue 61).
    ConfirmCloseTab,
}

/// What the export dialog was last asked to count, so the answer can be reused
/// until something it depends on changes.
#[derive(Clone, PartialEq)]
pub(crate) struct ExportPreviewKey {
    pub selection_only: bool,
    pub selection: Vec<NodeId>,
    pub generation: u64,
    pub bodies: simple3d_export::BodyMode,
    pub marks: Vec<(NodeId, simple3d_core::scene::ExportBody)>,
}

/// What an export is about to write.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ExportSummary {
    pub triangles: usize,
    pub bodies: usize,
}

/// One end of a measurement: where it is, and what kind of feature it caught, so
/// the readout can say "vertex" or "face centre" and the marker can say the
/// point was snapped rather than dropped on the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasurePoint {
    pub at: Vec3,
    pub kind: Option<crate::snap::FeatureKind>,
}

/// The measure tool's state (issue 69): whether it is holding the pointer, and
/// the one or two points picked so far.
///
/// The points stay put until the tool is dismissed -- the readout is meant to be
/// left on screen and looked at while the model is turned -- so this is its own
/// small piece of state rather than something recomputed each frame. Picking a
/// third point starts a fresh measurement from it.
#[derive(Clone, Debug, Default)]
pub struct Measure {
    pub active: bool,
    pub points: Vec<MeasurePoint>,
}

impl Measure {
    /// Add a point, beginning a new measurement once a pair is complete so the
    /// tool flows from one span to the next without a clear in between.
    pub fn add(&mut self, point: MeasurePoint) {
        if self.points.len() >= 2 {
            self.points.clear();
        }
        self.points.push(point);
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }

    /// Move one end of the span, or place it if it is not down yet. What the
    /// editable start and end fields in the property panel write to (issue 78).
    ///
    /// An end can only be placed once the one before it is: an end with no start
    /// is not half a measurement, it is a point with nothing to measure to.
    pub fn set_point(&mut self, index: usize, at: Vec3) {
        // Typing a coordinate is placing the point exactly, so whatever feature
        // it once caught is no longer what it is.
        let placed = MeasurePoint { at, kind: None };
        if index < self.points.len() {
            self.points[index] = placed;
        } else if index == self.points.len() && index < 2 {
            self.points.push(placed);
        }
    }

    /// The finished span, once both ends are placed.
    pub fn span(&self) -> Option<(MeasurePoint, MeasurePoint)> {
        match self.points.as_slice() {
            [a, b] => Some((*a, *b)),
            _ => None,
        }
    }
}

/// The numbers a span reads out: the straight-line distance, the per-axis delta,
/// the angle above the ground plane and the compass bearing around Z (issue 69).
///
/// The angle is split into these two because a single number cannot place a line
/// in space: inclination says how steep it is, bearing which way it runs, and
/// together they are the direction the delta points.
pub struct Measurement {
    pub distance: f64,
    pub delta: Vec3,
    pub inclination_deg: f64,
    pub bearing_deg: f64,
}

impl Measurement {
    pub fn between(a: Vec3, b: Vec3) -> Measurement {
        let delta = b - a;
        let distance = delta.length();
        let horizontal = (delta.x * delta.x + delta.y * delta.y).sqrt();
        // Inclination above the XY plane: 0 for a level span, +/-90 for a
        // vertical one. Undefined for a zero-length span, which reads as level.
        let inclination_deg = if distance < 1e-9 { 0.0 } else { delta.z.atan2(horizontal).to_degrees() };
        // Bearing around Z, measured from +X towards +Y, so it agrees with how a
        // yaw is read. A span with no horizontal run has no bearing; 0 is as good
        // as any and does not mislead because the inclination is then +/-90.
        let bearing_deg = if horizontal < 1e-9 { 0.0 } else { delta.y.atan2(delta.x).to_degrees() };
        Measurement { distance, delta, inclination_deg, bearing_deg }
    }
}

/// One body's snap features, shared out of the cache without copying them.
type Features = std::rc::Rc<Vec<crate::snap::Feature>>;
/// What the cache holds per node: which mesh the features were found on --
/// identified by the address of its `Arc`, which changes on re-evaluation and
/// nowhere else -- together with which axes were shown, since the axis crossings
/// are part of the list (issue 78), and the features themselves.
type CachedFeatures = ((usize, u8), Features);

pub struct App {
    pub scene: Scene,
    pub history: History,
    /// Every open document, one per tab (issue 61). The entry at `active` is a
    /// stand-in: the live state of the document on screen is the one held
    /// directly on this struct, and is only written back into the vector when
    /// another tab is picked. `crate::tabs` owns the swapping.
    pub tabs: Vec<crate::tabs::Document>,
    pub active: usize,
    pub settings: AppSettings,
    pub keymap: Keymap,

    /// The current selection, in click order. The last entry is the primary one
    /// the property editor and the manipulator act on.
    pub selection: Vec<NodeId>,
    /// The row a Shift+click measures its range from: the last outliner row
    /// clicked without Shift (issue 60).
    pub(crate) selection_anchor: Option<NodeId>,
    /// The row the last outliner click landed on, whatever modifiers it had.
    ///
    /// egui decides a double click from the delay between two clicks alone --
    /// the second one does not have to be on the widget the first was -- so two
    /// quick clicks on different rows used to open a rename on the second one
    /// (issue 59). A rename asks this whether both clicks were on the same row.
    pub(crate) outliner_last_click: Option<NodeId>,
    pub clipboard: Option<Clip>,

    pub worker: EvalWorker,
    pub evaluated: Evaluated,
    /// Bumped whenever a new evaluation lands, so cached images and renderables
    /// know to rebuild.
    pub evaluation_generation: u64,
    /// Set when the scene on screen has no viewpoint of its own worth keeping,
    /// so the first evaluation that gives it bounds should frame it. A project
    /// read from a file carries its own camera and must never set this: the
    /// stored viewpoint is the one the user saved.
    pub(crate) frame_when_evaluated: bool,
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
    /// The window's OpenGL context, when there is one. `None` under the test
    /// harness, which has no window -- and that alone makes the GPU renderer
    /// unavailable there, so the tests always exercise the software path.
    pub(crate) gl: Option<std::sync::Arc<eframe::glow::Context>>,
    /// The GPU renderer, built the first time it is asked for.
    pub(crate) gpu: Option<crate::gpu::Gpu>,
    /// Why the GPU renderer is not in use, when it was asked for and could not
    /// be had. Shown beside the engine picker, and the viewport falls back to
    /// the CPU rather than showing nothing.
    pub(crate) gpu_error: Option<String>,
    /// What the GPU renderer drew this frame, when it is the engine in use.
    pub(crate) gpu_texture: Option<egui::TextureId>,

    pub path: Option<PathBuf>,
    pub(crate) saved_revision: u64,

    /// The recent colours as they stood when the current painting run began, so
    /// a drag through the colour picker leaves one entry behind and not one per
    /// frame. `None` outside a coalescing run.
    paint_run_colours: Option<Vec<[u8; 3]>>,

    pub status: Status,
    /// When the current message was set, so a message that has been read can
    /// fade out instead of sitting there looking current.
    pub status_at: std::time::Instant,
    pub fields: FieldBuffers,
    /// Which field label is being dragged, if any. Held on the app rather than
    /// in widget state so the gesture survives the panel being relaid out.
    pub scrub: crate::ui::Scrub,
    pub rename: Option<(NodeId, String)>,
    /// The groups whose children the outliner is not showing. Held here rather
    /// than in widget state so it survives a relayout, and so selecting a node
    /// from the viewport can open the groups above it.
    pub collapsed: std::collections::HashSet<NodeId>,
    pub outliner_drag: Option<NodeId>,
    pub drop_target: Option<DropTarget>,

    /// The 3D cursor: where a new shape lands. `None` means the origin, which
    /// is also where it goes back to.
    pub cursor: Option<Vec3>,
    /// The measure tool (issue 69): when it holds the pointer, clicks pick
    /// features to measure between rather than selecting.
    pub measure: Measure,
    /// Whether geometry snapping is being asked for this frame (issue 68), set by
    /// the viewport from the snap-mode setting and the held key and read while a
    /// move drag runs.
    pub(crate) snap_requested: bool,
    /// The geometry feature the current drag is snapped onto, for the viewport to
    /// mark. `None` when nothing is snapped this frame.
    pub snap_indicator: Option<Vec3>,
    /// Every feature of the body the current drag is carrying, as offsets from
    /// its origin; gathered on `Begin`. See `App::drag_feature_offsets`.
    snap_sources: Vec<Vec3>,
    /// Each body's snap features, kept between frames and keyed on the identity
    /// of the mesh they were found on. See `App::features_of`.
    snap_features: std::cell::RefCell<std::collections::HashMap<NodeId, CachedFeatures>>,
    /// A deletion waiting on the outliner's confirmation strip: which nodes,
    /// with the question of what happens to their children still open.
    pub pending_delete: Option<Vec<NodeId>>,
    /// A view change in flight. The camera is the scene's, so the move writes
    /// into it every frame rather than holding a second copy of the truth.
    pub camera_move: Option<CameraMove>,
    /// The orientation cube, turned by hand away from the camera it belongs to.
    /// `None` -- the usual state -- means it shows exactly what the camera
    /// shows.
    pub cube_spin: Option<CubeSpin>,
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
    /// What the objects of an export are (issue 58). Only 3MF can hold more
    /// than one, so the dialog offers the choice only there.
    pub export_bodies: simple3d_export::BodyMode,
    /// What the export dialog last counted, and what it counted it for: the
    /// contents choice, the selection and the evaluation it was measured
    /// against. Counting a selection means evaluating it, which must not happen
    /// on every frame the dialog is open.
    pub(crate) export_preview: Option<(ExportPreviewKey, ExportSummary)>,

    pub modal: Modal,
    /// The dialog window that has already been placed over the middle of the
    /// main window. A dialog is centred once, when it opens; after that it is
    /// the window manager's and the user's to move.
    pub(crate) dialog_placed: Option<egui::ViewportId>,
    /// The tab a close confirmation is about, while that dialog is open.
    pub(crate) pending_close: Option<usize>,
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
    /// A modifier held on its own is a binding of its own (issue 77), and the
    /// toolkit reports no key event for one, so the hold is watched frame by
    /// frame -- once for firing shortcuts, once for the keymap editor's
    /// recorder, which never run at the same time but must not share a state.
    pub shortcut_mods: ui::ChordHold,
    pub record_mods: ui::ChordHold,

    /// Where settings and the keymap are read from and written back to. Held
    /// rather than looked up at each call site so a test can point an `App` at a
    /// temp directory, and so a running application cannot start reading one
    /// directory and writing another.
    config_dir: PathBuf,

    /// The message the fade clock is running for, so any assignment to `status`
    /// anywhere restarts it without having to remember to.
    last_status: Status,
    /// The title the window is already wearing. `Context::send_viewport_cmd`
    /// requests a repaint for every command it is handed, so sending the title
    /// unconditionally each frame asked for the next frame each frame and the
    /// application never went idle. Only a title that changed is sent.
    last_title: String,
    /// True while a run of held-down nudge keys is coalescing into one undo step.
    nudging: bool,
    /// Set once a quit has been confirmed, so the event loop can close the window.
    quit_now: bool,
}

/// The orientation cube turned on its own, so a side the camera cannot see can
/// still be picked. `camera` is the camera it was turned away from: the moment
/// the scene moves, the spin is stale and the cube goes back to following it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubeSpin {
    pub yaw: f64,
    pub pitch: f64,
    pub camera: (f64, f64),
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
    pub fn new(ctx: &egui::Context, gl: Option<std::sync::Arc<eframe::glow::Context>>, open: Option<PathBuf>) -> App {
        let mut app = App::with_config_dir(ctx, open, config::config_dir());
        app.gl = gl;
        app
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
            tabs: vec![crate::tabs::Document::empty()],
            active: 0,
            settings,
            paint_run_colours: None,
            keymap,
            selection: Vec::new(),
            selection_anchor: None,
            outliner_last_click: None,
            clipboard: None,
            worker: EvalWorker::spawn(),
            evaluated: crate::tabs::empty_evaluation(),
            evaluation_generation: 0,
            frame_when_evaluated: false,
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
            gl: None,
            gpu: None,
            gpu_error: None,
            gpu_texture: None,
            path: None,
            saved_revision: 0,
            status: Status::Idle,
            status_at: std::time::Instant::now(),
            fields: FieldBuffers::default(),
            scrub: crate::ui::Scrub::default(),
            rename: None,
            collapsed: std::collections::HashSet::new(),
            outliner_drag: None,
            drop_target: None,
            cursor: None,
            measure: Measure::default(),
            snap_requested: false,
            snap_sources: Vec::new(),
            snap_indicator: None,
            snap_features: std::cell::RefCell::new(std::collections::HashMap::new()),
            pending_delete: None,
            camera_move: None,
            cube_spin: None,
            dock_drag: crate::dock::DockDrag::default(),
            dock_headers: Vec::new(),
            dock_rects: Vec::new(),
            export_job: None,
            export_format: Format::ThreeMf,
            export_scale: "1".to_string(),
            export_selection_only: false,
            export_bodies: simple3d_export::BodyMode::One,
            export_preview: None,
            modal: Modal::None,
            dialog_placed: None,
            pending_close: None,
            error_title: String::new(),
            error_detail: String::new(),
            primitive_clip: None,
            primitive_name: String::new(),
            library: Vec::new(),
            keymap_search: String::new(),
            recording: None,
            shortcut_mods: ui::ChordHold::default(),
            record_mods: ui::ChordHold::default(),
            keymap_conflict: None,
            config_dir,
            last_status: Status::Idle,
            last_title: String::new(),
            nudging: false,
            quit_now: false,
        };
        app.export_format = Format::from_id(&app.settings.last_export_format).unwrap_or(Format::ThreeMf);
        app.export_scale = simple3d_core::unit::format_number(app.settings.last_export_scale, 4);
        app.export_bodies = simple3d_export::BodyMode::from_id(&app.settings.last_export_bodies).unwrap_or_default();
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
    pub(crate) fn starter_scene(&mut self) {
        self.history.clear();
        self.saved_revision = self.history.revision();
        self.frame_all();
        self.frame_when_evaluated = true;
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
        self.selection_anchor = Some(id);
        self.on_selection_changed();
    }

    pub fn toggle_selected(&mut self, id: NodeId) {
        if let Some(at) = self.selection.iter().position(|&x| x == id) {
            self.selection.remove(at);
        } else {
            self.selection.push(id);
        }
        // Ctrl+click puts the anchor on the row it touched, so a Shift+click
        // after it measures from where the pointer last was rather than from
        // wherever a range happened to start (issue 60).
        self.selection_anchor = Some(id);
        self.on_selection_changed();
    }

    /// Select everything between the anchor and `id`, over `rows` -- the rows
    /// the outliner is actually showing, so a range never reaches into a
    /// collapsed group the user cannot see (issue 60).
    ///
    /// Replacing the selection rather than adding to it is what Shift+click
    /// means in every list: the range is the selection, and Shift+clicking
    /// somewhere else re-measures it from the same anchor instead of piling
    /// ranges up. The anchor itself does not move, which is what lets a range
    /// be adjusted by clicking again.
    ///
    /// The scene root is left out: it is every other row's ancestor, and a
    /// selection holding it means "everything" to every command that reads one.
    pub fn select_range_to(&mut self, id: NodeId, rows: &[NodeId]) {
        let root = self.scene.root();
        let anchor = match self.selection_anchor {
            Some(anchor) if anchor != id && rows.contains(&anchor) => anchor,
            // Nothing to measure from: a Shift+click with no anchor is a plain
            // click, and sets one.
            _ => return self.select_only(id),
        };
        let (Some(from), Some(to)) =
            (rows.iter().position(|row| *row == anchor), rows.iter().position(|row| *row == id))
        else {
            return self.select_only(id);
        };
        let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
        self.selection = rows[lo..=hi].iter().copied().filter(|row| *row != root).collect();
        if self.selection.is_empty() {
            self.selection = vec![id];
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
        // Whatever is selected has to be findable: a node picked in the
        // viewport, or one left selected by an undo, opens the collapsed
        // groups above it rather than being selected out of sight.
        for id in self.selection.clone() {
            self.reveal(id);
        }
    }

    /// Open every group above `id`, so its row is one of the ones drawn.
    pub fn reveal(&mut self, id: NodeId) {
        let mut walk = self.scene.get(id).and_then(|node| node.parent);
        while let Some(parent) = walk {
            self.collapsed.remove(&parent);
            walk = self.scene.get(parent).and_then(|node| node.parent);
        }
    }

    /// Shut or open one group in the outliner.
    pub fn set_collapsed(&mut self, id: NodeId, collapsed: bool) {
        if collapsed {
            self.collapsed.insert(id);
        } else {
            self.collapsed.remove(&id);
        }
    }

    // -- edits --------------------------------------------------------------

    /// Take an undo snapshot and mark the scene for re-evaluation. Every
    /// model-mutating path in the application goes through here.
    pub fn edit(&mut self, label: &str, coalesce: Option<&str>) -> bool {
        let coalescing = self.history.record(&self.scene, label, coalesce);
        self.dirty = true;
        coalescing
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
        let name = crate::tabs::document_name(self.path.as_deref());
        format!("{}{name} - {APP_NAME}", if self.unsaved() { "*" } else { "" })
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

    /// Read `path` into the document on screen, replacing whatever it held.
    /// Which tab that is, `crate::tabs::open_path` has already decided.
    pub(crate) fn load_into_active(&mut self, path: &Path) {
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
                // The file's own camera stands, whether it was opened from the
                // menu or handed to the binary on the command line.
                self.frame_when_evaluated = false;
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
            CloseTab => self.close_tab(self.active),
            NextTab => self.cycle_tab(1),
            PreviousTab => self.cycle_tab(-1),
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
            Pattern => self.make_pattern(),
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

            // Reaching for a transform tool puts the measure tool away: only one
            // of them can own a click.
            ModeMove => self.pick_transform(Mode::Move),
            ModeRotate => self.pick_transform(Mode::Rotate),
            ModeResize => self.pick_transform(Mode::Resize),
            ModeScale => self.pick_transform(Mode::Scale),
            ToggleHandleFrame => {
                self.settings.handle_frame = self.settings.handle_frame.toggled();
                self.status = Status::Info(format!("Handles: {} frame", self.settings.handle_frame.label()));
            }
            MeasureTool => self.toggle_measure(),
            // A hold key, read live while a drag runs rather than acted on when
            // pressed, so pressing it on its own does nothing (issue 68).
            SnapToGeometry => {}
            NudgeLeft | NudgeRight | NudgeUp | NudgeDown | NudgeAway | NudgeToward => self.nudge(command),
        }
    }

    fn toggle_axis(&mut self, axis: usize) {
        let on = !self.scene.settings.axes_visible[axis];
        self.scene.settings.axes_visible[axis] = on;
        let name = ["X", "Y", "Z"][axis];
        self.status = Status::Info(format!("{name} axis {}", if on { "shown" } else { "hidden" }));
    }

    /// Switch to a transform tool, which also takes the measure tool out of the
    /// pointer's way and clears its span.
    fn pick_transform(&mut self, mode: Mode) {
        self.mode = mode;
        if self.measure.active {
            self.measure.active = false;
            self.measure.clear();
        }
    }

    /// Turn the measure tool on or off. Leaving it clears the span it was showing
    /// -- that is what "dismiss" means -- so the next time it is picked up it
    /// starts clean rather than with a stale line hanging in the scene.
    pub fn toggle_measure(&mut self) {
        self.measure.active = !self.measure.active;
        if self.measure.active {
            self.status = Status::Info("Measure: click two features to read the span between them".into());
        } else {
            self.measure.clear();
            self.status = Status::Info("Measure tool off".into());
        }
    }

    /// Where a measure click lands: the nearest snap feature of any shown body if
    /// one is within reach on screen, otherwise a point along the nearest edge,
    /// otherwise the point on the surface under the pointer, otherwise the ground
    /// plane. `None` only when the pointer is on empty sky, where there is
    /// nothing to measure to.
    ///
    /// The order is what makes placement predictable (issue 78): the exact
    /// points -- corners, midpoints, axis crossings -- win whenever one is in
    /// reach, and an edge catches the pointer only where none of them does, so
    /// aiming at a corner never lands part-way along the edge beside it.
    pub fn measure_point_at(&self, view: &crate::view::View, cursor: egui::Pos2) -> Option<MeasurePoint> {
        // An axis is only there to be caught where it is *drawn*: the line is cut
        // out of the material it runs through and hidden behind whatever is in
        // front of it, so a stretch the model covers is not a place to measure
        // from. Without this the catch followed the axis straight through a body,
        // which is the one thing the line on screen never does. A body's own
        // features are not filtered this way -- a corner around the back is still
        // a corner, and reaching for one is deliberate.
        let shown = |feature: &crate::snap::Feature| {
            feature.kind != crate::snap::FeatureKind::AxisCrossing || self.in_clear_view(view, feature.point)
        };
        if let Some((feature, _)) = self.nearest_feature_where(view, cursor, &[], shown) {
            return Some(MeasurePoint { at: feature.point, kind: Some(feature.kind) });
        }
        if let Some((at, kind, _)) = self.nearest_line_point(view, cursor) {
            return Some(MeasurePoint { at, kind: Some(kind) });
        }
        let (origin, dir) = view.ray(cursor);
        if let Some(t) = crate::pick::ray_mesh(&self.evaluated.mesh, origin, dir) {
            return Some(MeasurePoint { at: origin + dir * t, kind: None });
        }
        view.ray_plane_ahead(cursor, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).map(|at| MeasurePoint { at, kind: None })
    }

    /// The snap feature of a shown body nearest the cursor on screen, within the
    /// catch radius. Shared by the measure tool and by geometry snapping during a
    /// drag; `exclude` drops the bodies a drag is itself moving so it never snaps
    /// to the very thing it is carrying.
    pub fn nearest_feature_excluding(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
        exclude: &[NodeId],
    ) -> Option<(crate::snap::Feature, f32)> {
        self.nearest_feature_where(view, cursor, exclude, |_| true)
    }

    /// The same, for a caller that will not take every kind of feature -- the
    /// measure tool, which drops an axis crossing the model is covering.
    pub fn nearest_feature_where(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
        exclude: &[NodeId],
        accept: impl Fn(&crate::snap::Feature) -> bool,
    ) -> Option<(crate::snap::Feature, f32)> {
        let mut best: Option<(crate::snap::Feature, f32)> = None;
        for (&id, mesh) in &self.evaluated.node_meshes {
            if !self.scene.is_shown(id) || exclude.contains(&id) {
                continue;
            }
            let features = self.features_of(id, mesh);
            let wanted: Vec<crate::snap::Feature> = features.iter().copied().filter(&accept).collect();
            let hit = crate::snap::nearest_on_screen(
                &wanted,
                |p| view.project(p).map(|(screen, _)| screen),
                cursor,
                crate::snap::CATCH_PIXELS,
            );
            if let Some((feature, distance)) = hit {
                if best.is_none_or(|(_, d)| distance < d) {
                    best = Some((*feature, distance));
                }
            }
        }
        best
    }

    /// Whether a point can be seen from where the camera is: nothing solid
    /// between the eye and it.
    ///
    /// The evaluated mesh is exactly what the renderer draws as material, and
    /// what it cuts the axis lines out of, so asking it is asking the same
    /// question the picture answers. A point *inside* a body fails too, since
    /// the body's own near surface is in front of it.
    pub fn in_clear_view(&self, view: &crate::view::View, at: Vec3) -> bool {
        let Some((screen, _)) = view.project(at) else { return false };
        let (origin, dir) = view.ray(screen);
        let reach = (at - origin).dot(dir);
        match crate::pick::ray_mesh(&self.evaluated.mesh, origin, dir) {
            // A point on a surface is its own hit, so the comparison has to
            // leave room for one: a hundredth of a millimetre is far below
            // anything a measurement cares about and far above the arithmetic.
            Some(hit) => hit >= reach - 1e-2,
            None => true,
        }
    }

    /// One body's snap features, remembered between frames.
    ///
    /// Finding them welds the mesh, builds two hash maps over its edges, runs a
    /// union-find across its coplanar triangles and sorts the result -- 6.5 ms
    /// for a 16k-triangle body in a release build. Every visible body was paying
    /// that on *every frame* of a snapped drag and of a measure hover, which is
    /// most of a frame's budget spent recomputing something that only changes
    /// when the mesh does. The evaluated meshes are shared `Arc`s, so the
    /// pointer is exactly the "has this changed" key: a re-evaluation makes a new
    /// allocation and misses, and anything else hits.
    fn features_of(&self, id: NodeId, mesh: &std::sync::Arc<simple3d_geom::Mesh>) -> Features {
        let axes = self.scene.settings.axes_visible;
        let mask = (axes[0] as u8) | (axes[1] as u8) << 1 | (axes[2] as u8) << 2;
        let key = (std::sync::Arc::as_ptr(mesh) as usize, mask);
        if let Some((cached_key, features)) = self.snap_features.borrow().get(&id) {
            if *cached_key == key {
                return features.clone();
            }
        }
        let mut found = crate::snap::features_of(mesh);
        // Where the world axes run through the body, offered as corners and as
        // the edge between them (issue 78).
        found.extend(crate::snap::axis_features(mesh, axes));
        let features = std::rc::Rc::new(found);
        self.snap_features.borrow_mut().insert(id, (key, features.clone()));
        features
    }

    /// The point on the nearest *line* -- a body's edge, or a world axis -- for a
    /// pointer that is near one but not near any of the notable points on it
    /// (issue 78).
    ///
    /// Edges come from the same feature list, which carries each edge's two ends
    /// beside its midpoint, so they need no second pass over the geometry. The
    /// axes are lines in their own right: a point on one is as real a place to
    /// measure from as a corner is, and offering only the handful of places
    /// where an axis meets something left the rest of it -- most of it -- with
    /// nothing to catch.
    pub fn nearest_line_point(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
    ) -> Option<(Vec3, crate::snap::FeatureKind, f32)> {
        let project = |p: Vec3| view.project(p).map(|(screen, _)| screen);
        let mut best: Option<(Vec3, crate::snap::FeatureKind, f32)> = None;
        let mut consider = |a: Vec3, b: Vec3, kind: crate::snap::FeatureKind| {
            if let Some((at, distance)) = crate::snap::nearest_on_edge(a, b, project, cursor, crate::snap::CATCH_PIXELS)
            {
                if best.is_none_or(|(_, _, d)| distance < d) {
                    best = Some((at, kind, distance));
                }
            }
        };
        for (&id, mesh) in &self.evaluated.node_meshes {
            if !self.scene.is_shown(id) {
                continue;
            }
            for feature in self.features_of(id, mesh).iter() {
                if let Some((a, b)) = feature.span {
                    consider(a, b, crate::snap::FeatureKind::Edge);
                }
            }
        }
        // As far as the axes are actually drawn, so nothing is caught out where
        // there is no line to see.
        let reach = crate::render::grid_radius(view);
        for (a, b) in crate::snap::axis_lines(self.scene.settings.axes_visible, reach) {
            let hit = crate::snap::nearest_on_edge(a, b, project, cursor, crate::snap::CATCH_PIXELS);
            // Only where the line is really on screen: the stretch inside a body
            // is cut out of the drawing, and the stretch behind one is covered by
            // it, so neither is a place to measure from.
            if let Some((at, distance)) = hit.filter(|(at, _)| self.in_clear_view(view, *at)) {
                if best.is_none_or(|(_, _, d)| distance < d) {
                    best = Some((at, crate::snap::FeatureKind::Axis, distance));
                }
            }
        }
        best
    }

    /// Whether geometry snapping is being asked for right now (issue 68): always,
    /// never, or only while the snap key is held. `key_down` answers whether a
    /// toolkit key is currently pressed, which only the viewport can see.
    pub fn geometry_snap_wanted(&self, key_down: impl Fn(egui::Key) -> bool, mods: egui::Modifiers) -> bool {
        match self.settings.geometry_snap {
            SnapMode::Never => false,
            SnapMode::Always => true,
            SnapMode::WhileHeld => self.keymap.binding(Command::SnapToGeometry).is_some_and(|chord| {
                // The chord's modifiers count too. Matching on the key name alone
                // meant a hold rebound to Ctrl+V also fired on a bare V -- and on
                // Ctrl+V, which is Paste.
                //
                // A chord that is modifiers alone -- Ctrl, the default since
                // issue 77 -- has no key to ask about, and the modifier state is
                // the whole of it. One of several keys wants all of them down.
                chord.satisfied_by(
                    |name| crate::ui::key_from_name(name).is_some_and(&key_down),
                    mods.command,
                    mods.shift,
                    mods.alt,
                )
            }),
        }
    }

    /// The dragged node and everything under it: the bodies a drag is carrying,
    /// which geometry snapping must never snap to.
    fn drag_subtree(&self, id: NodeId) -> Vec<NodeId> {
        std::iter::once(id).chain(self.scene.descendants(id)).collect()
    }

    /// Every feature of the body a drag is carrying, as an offset from that
    /// node's origin. Taken once, when the handle is grabbed.
    ///
    /// Which feature should meet the target used to be decided here too, by
    /// looking for one within the catch radius of the cursor. A drag always
    /// starts on a manipulator handle, and those sit a fixed 78 screen pixels out
    /// along an axis -- never on the body's own geometry except by coincidence --
    /// so the answer was almost always "none", and it was the node's *origin*
    /// that landed on the target. Two boxes snapped together interpenetrated by
    /// half, which is not what "snap this corner to that corner" means.
    ///
    /// They are kept as *offsets*, and gathered before the body has moved,
    /// because `Evaluated` lags a drag: the meshes still describe where the body
    /// was at the last evaluation while `Node::position` is already live. An
    /// offset from the origin is the same either way, being a fact about the
    /// shape rather than about where it currently sits.
    fn drag_feature_offsets(&self, id: NodeId) -> Vec<Vec3> {
        let Some(frame) = self.evaluated.node_frames.get(&id) else { return Vec::new() };
        let world_origin = frame.point(self.scene.node(id).position);
        let mut offsets = Vec::new();
        for n in self.drag_subtree(id) {
            let Some(mesh) = self.evaluated.node_meshes.get(&n) else { continue };
            offsets.extend(self.features_of(n, mesh).iter().map(|f| f.point - world_origin));
        }
        offsets
    }

    /// Snap the dragged node so one of its own features lands on the nearest
    /// feature of another body under the pointer (issue 68). Returns the world
    /// point it snapped onto, or `None` when nothing was in reach, in which case
    /// the grid drag stands.
    ///
    /// `handle` is what the drag is being steered by, and the snap stays inside
    /// it: an axis handle only ever moves along its axis and a plane handle only
    /// within its plane. Writing the full three-dimensional correction turned an
    /// X-axis drag into a free move -- the body jumped in Y and Z as well, which
    /// is the one thing choosing an axis handle says it must not do.
    fn apply_geometry_snap(
        &mut self,
        id: NodeId,
        gizmo: &Gizmo,
        handle: Handle,
        view: &crate::view::View,
        cursor: egui::Pos2,
    ) -> Option<Vec3> {
        let frame = *self.evaluated.node_frames.get(&id)?;
        let world_origin = frame.point(self.scene.node(id).position);
        let exclude = self.drag_subtree(id);
        let (target, _) = self.nearest_feature_excluding(view, cursor, &exclude)?;
        // Only the components the handle actually governs survive, measured in
        // the handle's own frame rather than the world's so a rotated body's
        // local axes are respected the same way the drag itself respects them.
        let constrain = |wanted: Vec3| {
            let mut correction = Vec3::ZERO;
            for axis in handle.axes() {
                let dir = gizmo.axes[axis];
                correction = correction + dir * wanted.dot(dir);
            }
            correction
        };
        // The feature of the moving body that ends up closest to the target once
        // the handle's constraint has had its say -- so the corner that can
        // actually reach it is the one that meets it. An empty group offers no
        // offsets, and then it is the origin that snaps, which still beats
        // refusing to snap at all.
        let mut best: Option<(Vec3, f64, f64)> = None;
        for offset in std::iter::once(Vec3::ZERO).chain(self.snap_sources.iter().copied()) {
            let correction = constrain(target.point - (world_origin + offset));
            // How far the feature still misses the target after the constrained
            // move: exactly zero when it can reach, and the shortest achievable
            // gap when the handle will not let it all the way there.
            let miss = (world_origin + offset + correction - target.point).length();
            // Several features often reach equally well -- on an X drag towards a
            // corner, the box's left face can meet it just as exactly as its
            // right, by flying the whole body past the target and landing on top
            // of it. Between equals, the one that moves the body least is the one
            // that was meant.
            let travel = correction.length();
            const TIE: f64 = 1e-6;
            let better = match best {
                None => true,
                Some((_, best_miss, best_travel)) => {
                    miss < best_miss - TIE || ((miss - best_miss).abs() <= TIE && travel < best_travel)
                }
            };
            if better {
                best = Some((correction, miss, travel));
            }
        }
        let (correction, _, _) = best?;
        let new_origin = world_origin + correction;
        let new_position = frame.inverse().point(new_origin);
        if let Some(node) = self.scene.get_mut(id) {
            node.position = new_position;
        }
        Some(target.point)
    }

    /// Take the last placed end back off, leaving the one before it in place: a
    /// right-click in the viewport, for a point put down in the wrong place.
    ///
    /// Unset rather than moved: an end that is gone is placed again by the next
    /// click, which is what "reset to unset" has to mean for a tool whose next
    /// click always places the next end.
    pub fn measure_unplace(&mut self) {
        match self.measure.points.pop() {
            Some(_) if self.measure.points.is_empty() => {
                self.status = Status::Info("Measure: the start is unset again".into())
            }
            Some(_) => self.status = Status::Info("Measure: click the second feature".into()),
            None => self.status = Status::Info("Measure: nothing placed to take back".into()),
        }
    }

    /// Record a measure click, and say what the span reads once both ends are
    /// down (issue 69).
    pub fn measure_click(&mut self, point: MeasurePoint) {
        self.measure.add(point);
        if let Some((a, b)) = self.measure.span() {
            let m = Measurement::between(a.at, b.at);
            // With the unit: this line sits in the same bar as "20 x 20 x 20 mm"
            // and "Grid 10 mm", and a bare number among them says nothing at all
            // in a drawing whose unit is centimetres.
            self.status = Status::Info(format!(
                "Distance {} {}  \u{00B7}  incline {}\u{00B0}",
                simple3d_core::unit::format_length(m.distance, self.unit()),
                self.unit().suffix(),
                simple3d_core::unit::format_angle(m.inclination_deg),
            ));
        } else {
            self.status = Status::Info("Measure: click the second feature".into());
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

    /// The parameter keys a pattern's viewport spacing handle drives: its count,
    /// and the step components that make up the run between copies -- one per
    /// axis for a linear pattern, whose run can point anywhere, and the single
    /// column step for a grid, whose first run is along its own X. `None` for a
    /// kind with no straight run to lay out by dragging (issue 67).
    pub fn pattern_spacing_keys(&self, id: NodeId) -> Option<(&'static str, &'static [&'static str])> {
        let node = self.scene.get(id)?;
        if !node.is_pattern() {
            return None;
        }
        match node.params()?.get("kind").copied().map(|v| v.as_u32()).unwrap_or(0) {
            0 => Some(("count", &["step_x", "step_y", "step_z"])), // Linear
            1 => Some(("grid_x", &["grid_step_x"])),               // Grid, along its columns
            _ => None,
        }
    }

    /// The run a pattern's copies step along, in the node's own frame: the step
    /// components the handle drives, read back as a vector.
    pub fn pattern_step_vector(&self, id: NodeId) -> Option<Vec3> {
        use simple3d_core::primitive::ParamsExt;
        let (_, keys) = self.pattern_spacing_keys(id)?;
        let params = self.scene.get(id)?.params()?;
        let mut step = Vec3::ZERO;
        for (axis, key) in keys.iter().enumerate() {
            match axis {
                0 => step.x = params.num(key),
                1 => step.y = params.num(key),
                _ => step.z = params.num(key),
            }
        }
        Some(step)
    }

    /// Set a pattern's step from a drag, coalesced into one undo step so the
    /// whole drag is a single "spacing" edit.
    ///
    /// The whole run is scaled together, so dragging the handle of a pattern that
    /// steps diagonally lengthens the diagonal rather than straightening it onto
    /// X. The length is rounded to the document's move step, because every other
    /// drag in the viewport is -- a handle that alone produced 7.0359 mm under a
    /// 1 mm step was the odd one out.
    pub fn set_pattern_step(&mut self, id: NodeId, along: f64, mods: gizmo::Mods) {
        let Some((_, keys)) = self.pattern_spacing_keys(id) else { return };
        let Some(step) = self.pattern_step_vector(id) else { return };
        let length = step.length();
        // The direction to lengthen along: the run's own, or the first axis when
        // there is no run yet to take a direction from.
        let dir = if length > 1e-9 { step * (1.0 / length) } else { Vec3::new(1.0, 0.0, 0.0) };
        let wanted = mods.snap(along.max(0.0), self.move_snap());
        self.edit("Pattern spacing", Some(&format!("pattern-step:{id}")));
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            let scaled = dir * wanted;
            for (axis, key) in keys.iter().enumerate() {
                let value = match axis {
                    0 => scaled.x,
                    1 => scaled.y,
                    _ => scaled.z,
                };
                params.insert((*key).to_string(), simple3d_core::primitive::ParamValue::Length(value));
            }
        }
        self.touch();
    }

    /// The pattern creation tool (issue 67): wrap the selection in a pattern
    /// node that repeats it, or -- with nothing selected -- drop an empty pattern
    /// at the insertion point for shapes to be put under. Either way the pattern
    /// is selected, so the property editor is right there to lay it out.
    fn make_pattern(&mut self) {
        self.edit("Pattern", None);
        let created = if self.selection.is_empty() {
            let (parent, index) = self.scene.insertion_point(self.primary());
            Some(self.scene.add_pattern(parent, index))
        } else {
            // Reuse the grouping logic to gather the top-level selection under one
            // new node, then make that node a pattern rather than a union.
            let group = self.scene.group_selection(&self.selection.clone());
            if let Some(group) = group {
                // Measured *before* the node becomes a pattern. Asking afterwards
                // measures the repetition rather than the thing being repeated --
                // three 20 mm boxes 20 mm apart read as 60 mm wide, and the
                // spacing derived from that came out three times too large.
                let size = simple3d_core::eval::subtree_bounds(&self.scene, group)
                    .map(|(lo, hi)| hi - lo)
                    .unwrap_or(Vec3::ZERO);
                if let Some(node) = self.scene.get_mut(group) {
                    // The stock 20 mm step is exactly the width of the stock box,
                    // so a pattern made from one laid its copies down touching --
                    // see `pattern::params_for_size`.
                    node.body =
                        simple3d_core::scene::Body::Pattern { params: simple3d_core::pattern::params_for_size(size) };
                    node.name = "Pattern".to_string();
                }
            }
            group
        };
        match created {
            Some(id) => {
                self.select_only(id);
                self.status = Status::Info("Pattern: choose its kind and numbers in the properties panel".into());
            }
            None => self.status = Status::Warning("That selection cannot be made into a pattern".into()),
        }
    }

    /// Paint every node in `targets`, and remember the colour so it can be
    /// offered again. `None` clears the paint instead. One place, because the
    /// property editor and the outliner's menu both do this and both have to
    /// remember it.
    pub(crate) fn paint(&mut self, targets: &[NodeId], colour: Option<Colour>, coalesce: Option<&str>) {
        let coalescing = self.edit(if colour.is_some() { "Colour" } else { "Clear colour" }, coalesce);
        for target in targets {
            self.scene.paint_subtree(*target, colour);
        }
        if let Some(Colour(rgb)) = colour {
            // Only a colour the user picked out for themselves. The eight
            // presets are already on the palette above this row; repeating one
            // of them here spends the recent list on colours that were never
            // hard to find (issue 35).
            if !is_preset(rgb) {
                // A drag through the picker paints on every frame it moves, and
                // remembering each frame would fill the whole row with eight
                // shades of the one colour. The run is one choice: put the list
                // back the way it was when the drag began, then remember the
                // colour the drag has reached (issue 35).
                match (coalescing, &self.paint_run_colours) {
                    (true, Some(before)) => self.settings.recent_colours = before.clone(),
                    (true, None) => {}
                    (false, _) => {
                        self.paint_run_colours = coalesce.map(|_| self.settings.recent_colours.clone());
                    }
                }
                self.settings.remember_colour(rgb);
            }
        }
    }

    /// The recent colours worth offering: the ones that are not already a
    /// preset. Filtered on the way out as well as on the way in, so a list
    /// saved by an earlier version stops showing them too.
    pub(crate) fn custom_recent_colours(&self) -> Vec<[u8; 3]> {
        self.settings.recent_colours.iter().copied().filter(|rgb| !is_preset(*rgb)).collect()
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

    /// Whether a move by `delta` would do anything: at least one of the nodes
    /// it acts on has somewhere to go. What the menus grey their entries out
    /// on, rather than letting a click answer with a status line nobody sees.
    pub fn can_reorder(&self, delta: isize) -> bool {
        !self.reorder_plan(delta).is_empty()
    }

    /// What a reorder acts on: the topmost selected nodes, in tree order,
    /// minus the root. The whole selection rather than the primary alone --
    /// moving one of three selected siblings and leaving the other two behind
    /// looks exactly like the command not working (issue 41).
    fn reorder_targets(&self) -> Vec<NodeId> {
        self.top_level_selection().into_iter().filter(|id| *id != self.scene.root()).collect()
    }

    /// Which of those nodes would actually move, in the order they have to be
    /// moved in.
    ///
    /// Selected siblings move as a block: the one nearest the end goes first,
    /// into the space it has, and a node whose neighbour is a selected node
    /// that could not move cannot move either -- otherwise a run of three
    /// pushed against the end of the list would come apart, one node
    /// overtaking another that had nowhere to go.
    fn reorder_plan(&self, delta: isize) -> Vec<NodeId> {
        let mut targets = self.reorder_targets();
        if delta > 0 {
            targets.reverse();
        }
        let mut stuck: Vec<NodeId> = Vec::new();
        let mut moving: Vec<NodeId> = Vec::new();
        for id in targets {
            let Some(parent) = self.scene.get(id).and_then(|node| node.parent) else {
                stuck.push(id);
                continue;
            };
            let children = &self.scene.node(parent).children;
            let Some(at) = children.iter().position(|&c| c == id) else {
                stuck.push(id);
                continue;
            };
            let to = at as isize + delta;
            if to < 0 || to >= children.len() as isize {
                stuck.push(id);
                continue;
            }
            if stuck.contains(&children[to as usize]) {
                stuck.push(id);
                continue;
            }
            moving.push(id);
        }
        moving
    }

    fn reorder(&mut self, delta: isize) {
        if self.reorder_targets().is_empty() {
            self.status = Status::Info("Select something to move".into());
            return;
        }
        let plan = self.reorder_plan(delta);
        if plan.is_empty() {
            self.status = Status::Info(
                if delta < 0 { "Already first among its siblings" } else { "Already last among its siblings" }.into(),
            );
            return;
        }
        self.edit("Reorder", None);
        for id in plan {
            self.scene.reorder(id, delta);
        }
    }

    /// Set a group's boolean operator, from wherever a group can be pointed at.
    pub fn set_group_op(&mut self, id: NodeId, op: GroupOp) {
        if self.scene.node(id).group_op() == Some(op) {
            return;
        }
        self.edit("Operation", None);
        if let Some(node) = self.scene.get_mut(id) {
            node.body = simple3d_core::scene::Body::Group { op };
        }
        self.status = Status::Info(format!("{} group", op.label()));
    }

    /// Add a group or a primitive *where the tree is pointing*: inside `at` when
    /// it can hold children, beside it otherwise. What the outliner's own Add
    /// menu uses, so a shape made from a row lands on that row rather than at the
    /// document's insertion point (issue 44).
    pub fn add_node_at(&mut self, at: NodeId, type_id: Option<&str>, op: GroupOp) {
        self.edit(if type_id.is_some() { "Add" } else { "Add group" }, None);
        let (parent, index) = match self.scene.get(at) {
            // A pattern holds children exactly as a group does (issue 67), so
            // Add from a pattern's own row has to go *into* it. Asking
            // `is_group` here put the shape beside the pattern instead, which
            // made the row menu disagree with both the drag-and-drop rule and
            // the document-level Add, and both of those already say "into".
            Some(node) if node.can_hold_children() => (at, self.scene.node(at).children.len()),
            Some(node) => match node.parent {
                Some(parent) => {
                    let after = self.scene.node(parent).children.iter().position(|&c| c == at).map_or(0, |i| i + 1);
                    (parent, after)
                }
                None => (self.scene.root(), self.scene.node(self.scene.root()).children.len()),
            },
            None => (self.scene.root(), self.scene.node(self.scene.root()).children.len()),
        };
        let created = match type_id {
            Some(type_id) => self.scene.add_primitive(type_id, parent, index),
            None => Some(self.scene.add_group(op, parent, index)),
        };
        let Some(id) = created else {
            self.history.discard_last();
            self.status = Status::Warning("That shape is not in the palette".into());
            return;
        };
        self.collapsed.remove(&parent);
        self.select_only(id);
        self.status = Status::Info(format!("Added {}", self.scene.node(id).name));
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
                self.snap_indicator = None;
            }
            gizmo::DragPhase::Finish => {
                self.drag = None;
                self.history.close();
                self.fields.clear();
                self.snap_indicator = None;
            }
            gizmo::DragPhase::Continue => {
                let snap = self.move_snap();
                let Some(cursor) = cursor else { return };
                let rotate_snap = self.settings.rotate_snap_deg;
                let unit = self.scene.settings.unit;
                let handle = match self.drag.as_mut() {
                    Some(drag) => {
                        drag.update(&mut self.scene, view, cursor, mods, snap, rotate_snap, unit);
                        drag.handle
                    }
                    None => return,
                };
                // Geometry snapping (issue 68) rides on top of the grid drag, and
                // only for a move: it overrides the position so one of the moved
                // body's own features lands on a body under the pointer.
                if self.snap_requested && matches!(handle, Handle::MoveAxis(_) | Handle::MovePlane(_)) {
                    self.snap_indicator = self.apply_geometry_snap(id, gizmo, handle, view, cursor);
                    // The readout beside the cursor is written by the grid drag
                    // and describes the move the snap has just overridden -- it
                    // read "X +32mm" while the body had actually gone to
                    // (53, -10, 10). Restate it from what really happened.
                    if self.snap_indicator.is_some() {
                        let unit = self.scene.settings.unit;
                        let moved =
                            self.scene.node(id).position - self.drag.as_ref().map_or(Vec3::ZERO, |d| d.start_position);
                        if let Some(drag) = self.drag.as_mut() {
                            drag.readout = format!(
                                "snap {}, {}, {} {}",
                                simple3d_core::unit::format_length(moved.x, unit),
                                simple3d_core::unit::format_length(moved.y, unit),
                                simple3d_core::unit::format_length(moved.z, unit),
                                unit.suffix()
                            );
                        }
                    }
                } else {
                    self.snap_indicator = None;
                }
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
                        Mode::Scale => "Scale",
                    },
                    None,
                );
                self.drag = Drag::begin(&self.scene, gizmo, id, handle, view, cursor);
                // Before anything has moved, while the evaluated meshes and the
                // live positions still agree.
                self.snap_sources = self.drag_feature_offsets(id);
                self.snap_indicator = None;
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

    /// What an outliner drag started on `source` actually carries: the whole
    /// selection when the row that was grabbed is part of it, and that row
    /// alone otherwise -- dragging something unselected is a statement about
    /// that node, not about whatever was selected before (issue 43).
    ///
    /// Topmost nodes only, in document order, which is both the order they land
    /// in and the set `Scene::reparent_many` expects.
    pub fn dragged_nodes(&self, source: NodeId) -> Vec<NodeId> {
        if self.is_selected(source) && self.selection.len() > 1 {
            let tops = self.top_level_selection();
            if !tops.is_empty() {
                return tops;
            }
        }
        vec![source]
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
                // Where a new shape lands is the user's choice; the palette's
                // hint line says which choice is in force. How big the shape is
                // is part of the answer, so the point is asked for only now, with
                // the node in the scene and its own size there to be measured.
                let at = self.insertion_point_world(self.near_face_x(&[id]).unwrap_or(0.0));
                if let Some(node) = self.scene.get_mut(id) {
                    node.position = at;
                }
                self.select_only(id);
                self.status = Status::Info(format!("Added {}", self.scene.node(id).name));
            }
            None => self.status = Status::Warning("Unknown primitive type".into()),
        }
    }

    /// Where the near side of what is being added sits relative to its own
    /// origin, along X, across all of it.
    ///
    /// Measured from the nodes themselves rather than guessed from their
    /// parameters, because a shape's width is not always one of them: the
    /// regular polyhedra name an edge length, and the capsule drives no X
    /// extent at all. `None` when there is nothing to measure, or when the
    /// placement in force does not care -- only "beside the selection" does, so
    /// nothing is built for the other three.
    fn near_face_x(&self, ids: &[NodeId]) -> Option<f64> {
        if self.settings.placement != Placement::BesideSelection {
            return None;
        }
        ids.iter()
            .filter_map(|&id| simple3d_core::eval::subtree_bounds(&self.scene, id))
            .map(|(lo, _)| lo.x)
            .reduce(f64::min)
    }

    /// Where the next shape goes, in world millimetres, under the current
    /// placement choice.
    ///
    /// `near_face_x` is where the near side of the thing being added sits
    /// relative to its own origin -- for a centred 40 mm box, -20. Only
    /// "beside the selection" needs it, and it needs it badly: the point it
    /// answers with is written to `Node::position`, so leaving the shape's own
    /// width out of the sum buries it half inside what it was meant to stand
    /// clear of. Pass 0 where nothing is being added and the answer is only
    /// being described, as the palette's hint line does.
    ///
    /// World rather than parent-frame: adding into a rotated group would
    /// otherwise put the shape somewhere else entirely. `Node::position` is in
    /// the parent's coordinates, so the answer is carried back through the
    /// parent's frame before it is written.
    pub fn insertion_point_world(&self, near_face_x: f64) -> Vec3 {
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
                Some((lo, hi)) => {
                    Vec3::new(hi.x + self.move_snap() - near_face_x, (lo.y + hi.y) * 0.5, (lo.z + hi.z) * 0.5)
                }
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
        let target = self.primary();
        let created = clipboard::insert(&mut self.scene, &clip, target, false);
        if created.is_empty() {
            self.status = Status::Warning(format!("\u{201C}{}\u{201D} has nothing in it", entry.name));
            return;
        }
        // The saved subtree keeps its own internal arrangement; what moves is
        // where the whole thing sits.
        let anchor = self.scene.node(created[0]).position;
        // Its near side is measured across every node in it, so a saved
        // primitive stands clear of the selection as a whole rather than
        // leading with whichever node happens to be first.
        let near = self.near_face_x(&created).map_or(0.0, |x| x - anchor.x);
        let at = self.insertion_point_world(near);
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

    /// The mesh an export writes: the whole scene, or just what is selected.
    ///
    /// A selection is re-evaluated subtree by subtree rather than merged out of
    /// `Evaluated::node_meshes`, which holds primitives only -- merging those
    /// wrote a selected boolean group as its raw operands, so a difference kept
    /// its cutter and a union was refused as non-manifold.
    pub fn export_mesh(&self) -> std::sync::Arc<simple3d_geom::Mesh> {
        if self.export_selection_only {
            let tops = self.top_level_selection();
            std::sync::Arc::new(simple3d_core::eval::selection_mesh(&self.scene, &tops, &self.evaluated.node_frames))
        } else {
            self.evaluated.mesh.clone()
        }
    }

    /// The objects an export keeps apart: the scene's own top-level nodes, or
    /// the top-level nodes of the selection.
    ///
    /// A node is one object however deep it goes -- a boolean group is the
    /// shape it evaluates to, the same body the viewport draws, not its
    /// operands. Evaluated fresh rather than merged out of `Evaluated`, for the
    /// reason `export_mesh` gives.
    pub fn export_parts(&self) -> Vec<simple3d_core::eval::Part> {
        let roots = self.export_roots();
        let frames = &self.evaluated.node_frames;
        match self.export_body_mode() {
            simple3d_export::BodyMode::One => Vec::new(),
            simple3d_export::BodyMode::TopLevel => simple3d_core::eval::part_meshes(&self.scene, &roots, frames),
            simple3d_export::BodyMode::Selected => simple3d_core::eval::body_meshes(&self.scene, &roots, frames),
        }
    }

    /// What this export's bodies really are: the mode chosen, unless the format
    /// has nowhere to put more than one, in which case there is only ever the
    /// single merged body.
    pub fn export_body_mode(&self) -> simple3d_export::BodyMode {
        if self.export_format.keeps_objects_separate() {
            self.export_bodies
        } else {
            simple3d_export::BodyMode::One
        }
    }

    /// The nodes an export starts from: the scene's own top level, or the
    /// top-level nodes of the selection. Also what the body picker's tree
    /// shows, and where `Scene::body_lock` stops walking upwards.
    pub fn export_roots(&self) -> Vec<NodeId> {
        if self.export_selection_only {
            self.top_level_selection()
        } else {
            self.scene.node(self.scene.root()).children.clone()
        }
    }

    /// What the export dialog promises: how many triangles will be verified as
    /// watertight, and how many bodies they will be written as.
    ///
    /// Separated bodies are counted unmerged, which is what will be written:
    /// merging two touching bodies drops the triangles buried inside the join,
    /// and keeping them apart does not.
    ///
    /// Cached, because working it out in the user-chosen mode means unioning
    /// every body -- far too much to do on each frame the window is open. The
    /// key carries the export body marks as well as the evaluation, since
    /// regrouping changes the answer without changing any geometry.
    pub fn export_summary(&mut self) -> ExportSummary {
        let key = ExportPreviewKey {
            selection_only: self.export_selection_only,
            selection: self.selection.clone(),
            generation: self.evaluation_generation,
            bodies: self.export_body_mode(),
            marks: self.scene.export_body_marks(),
        };
        if let Some((cached, summary)) = &self.export_preview {
            if *cached == key {
                return *summary;
            }
        }
        let summary = if key.bodies.separates() {
            let parts = self.export_parts();
            ExportSummary { triangles: parts.iter().map(|part| part.mesh.triangle_count()).sum(), bodies: parts.len() }
        } else {
            ExportSummary { triangles: self.export_mesh().triangle_count(), bodies: 1 }
        };
        self.export_preview = Some((key, summary));
        summary
    }

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
        let bodies = self.export_body_mode();
        let parts: Vec<(String, std::sync::Arc<simple3d_geom::Mesh>)> = if bodies.separates() {
            self.export_parts().into_iter().map(|part| (part.name, std::sync::Arc::new(part.mesh))).collect()
        } else {
            vec![(String::new(), self.export_mesh())]
        };
        if parts.iter().all(|(_, mesh)| mesh.triangle_count() == 0) {
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
        self.settings.last_export_bodies = self.export_bodies.id().to_string();

        let options = simple3d_export::Options {
            format: self.export_format,
            scale,
            unit: simple3d_export::Unit3mf::Millimeter,
            allow_invalid: false,
            bodies,
        };
        self.export_job = Some(ExportJob::spawn_parts(path, parts, options, EXPORT_LIMIT));
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
        if self.any_unsaved() {
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

/// Whether a colour is one of the fixed swatches the palette rows already
/// offer.
fn is_preset(rgb: [u8; 3]) -> bool {
    crate::theme::PAINT_PRESETS.iter().any(|(_, colour)| [colour.r(), colour.g(), colour.b()] == rgb)
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
    // Nothing is being added yet, so nothing has a width to clear: under
    // "beside the selection" this is the line the next shape's near side will
    // meet, and the wording below says so rather than passing it off as the
    // point the shape's origin will sit at.
    let at = app.insertion_point_world(0.0);
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
        Placement::BesideSelection => format!(
            "Lands clear of the selection, its near side at {} {} on X.",
            simple3d_core::unit::format_length(at.x, unit),
            unit.suffix()
        ),
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
    /// Build the GPU renderer if it is wanted and can be had, and give egui the
    /// texture it draws into.
    ///
    /// Failure here is not fatal and is not silent: the reason is kept and shown
    /// beside the engine picker, and the viewport goes on drawing in software.
    fn prepare_gpu(&mut self, frame: &mut eframe::Frame) {
        if self.settings.render_engine != config::RenderEngine::Gpu || self.gpu.is_some() {
            return;
        }
        if self.gpu_error.is_some() {
            return; // asked once, refused once; do not retry every frame
        }
        let Some(gl) = self.gl.clone() else {
            self.gpu_error = Some("no OpenGL context (the window has none)".to_string());
            return;
        };
        let mut gpu = match crate::gpu::Gpu::new(gl) {
            Ok(gpu) => gpu,
            Err(why) => {
                self.gpu_error = Some(why);
                return;
            }
        };
        // A texture of its own, made at once so there is something to register;
        // the first real frame reallocates it to the viewport's size without
        // changing its identity.
        match gpu.prepare_texture() {
            Ok(texture) => {
                let id = frame.register_native_glow_texture(texture);
                gpu.set_texture_id(id);
                self.gpu = Some(gpu);
                self.image_key = u64::MAX;
            }
            Err(why) => self.gpu_error = Some(why),
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        self.menu_bar(ctx);
        self.status_bar(ctx);
        crate::panel_toolrail::show(self, ctx);
        crate::dock::show(self, ctx, Side::Left);
        crate::dock::show(self, ctx, Side::Right);
        // After the docks and the rail, so the row of tabs spans the workspace
        // itself rather than the whole window: the documents belong to the
        // viewport under them, not to the window's chrome.
        crate::tabs::show(self, ctx);
        panel_viewport::show(self, ctx);
        crate::dock::resolve_drag(self, ctx);
        self.modals(ctx);
    }
}

impl eframe::App for App {
    /// What is behind the window's own painting. The frame belongs to the
    /// window system now, so nothing is rounded away and nothing needs to show
    /// the desktop through it: an opaque surface, and no transparency for a
    /// compositor to have to blend.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = crate::theme::token::SURFACE_0;
        [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // The GPU renderer's texture is registered with egui exactly once, here,
        // where `eframe::Frame` is in reach -- `App::ui` is also driven by the
        // test harness, which has no window and so no context to register with.
        // The texture keeps its identity across a resize, so one registration
        // lasts the life of the application.
        self.prepare_gpu(frame);
        // Asked every frame rather than once at startup, so the setting takes
        // effect on the next dialog rather than on the next run. egui reads it
        // when a viewport is shown, and `dialog` already draws the embedded
        // form -- it is the one the headless tests have always driven.
        ctx.set_embed_viewports(self.settings.embed_dialogs);
        // A new message restarts its clock. Watching the value rather than
        // stamping it at every assignment means no `status = ...` anywhere in
        // the application can forget to.
        if self.status != self.last_status {
            self.last_status = self.status.clone();
            self.status_at = std::time::Instant::now();
        }
        if let Some(result) = self.worker.poll() {
            self.evaluated = result;
            self.evaluation_generation += 1;
            if std::mem::take(&mut self.frame_when_evaluated) {
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

        let title = self.title();
        if title != self.last_title {
            self.last_title.clone_from(&title);
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        self.handle_shortcuts(ctx);

        self.ui(ctx);

        // Confirmation on quit (spec section 7.4): intercept the window's own
        // close button as well as the Quit command.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit_now {
            if self.any_unsaved() {
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
        // update without the user having to move the mouse. A drag and a camera
        // move are followed frame by frame, because the next frame is the
        // answer to the pointer; everything else asks for the frame it will
        // actually have something new to show in, since a repaint requested
        // with no delay is a repaint requested for right now, and the loop then
        // runs as fast as the machine allows.
        if self.drag.is_some() || self.camera_move.is_some() {
            ctx.request_repaint();
        } else if self.worker.is_busy() || self.export_job.is_some() {
            // Progress and the preview, at a rate a person can read rather than
            // at whatever the rasterizer can manage.
            ctx.request_repaint_after(Duration::from_millis(33));
        } else if self.status != Status::Idle {
            // A status message is still for its whole lifetime and only then
            // fades. Nothing changes until the fade starts, so ask for the
            // frame that starts it, and only during the fade for frames after
            // that.
            let age = self.status_at.elapsed();
            if age < STATUS_LIFETIME {
                ctx.request_repaint_after(STATUS_LIFETIME - age);
            } else if status_opacity(&self.status, age) > 0.0 {
                ctx.request_repaint_after(Duration::from_millis(33));
            }
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
    use simple3d_core::scene::Visibility;

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
            Modal::About,
            Modal::Error,
            Modal::ConfirmQuit,
            Modal::ConfirmCloseTab,
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
        let app = App::new(&ctx, None, None);
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
        assert_eq!(app.insertion_point_world(0.0), Vec3::new(10.0, -5.0, 0.0));
        assert!(insertion_hint(&app).contains("camera"), "{}", insertion_hint(&app));

        // Beside the selection: clear of it, not inside it.
        app.clear_selection();
        let plate = app.scene.depth_first().into_iter().find(|&id| id != app.scene.root()).unwrap();
        app.select_only(plate);
        app.reevaluate_for_test();
        app.settings.placement = Placement::BesideSelection;
        let (_, hi) = app.selection_bounds().expect("the plate has bounds");
        assert_eq!(app.insertion_point_world(0.0).x, hi.x + app.move_snap());
    }

    /// The point of "beside the selection" is that the two do not overlap. An
    /// assertion on the formula cannot see that -- it restates it -- so this
    /// adds the shape and measures where it actually ended up.
    #[test]
    fn a_shape_added_beside_the_selection_does_not_overlap_it() {
        for type_id in ["box", "sphere", "tetrahedron", "cylinder"] {
            let mut app = headless_app();
            app.settings.placement = Placement::Origin;
            app.add_node(Some(type_id), GroupOp::Union);
            let first = app.primary().unwrap();
            app.reevaluate_for_test();
            let (_, hi) = app.selection_bounds().expect("the first shape has bounds");

            app.settings.placement = Placement::BesideSelection;
            app.add_node(Some(type_id), GroupOp::Union);
            let second = app.primary().unwrap();
            assert_ne!(first, second);
            app.reevaluate_for_test();
            let (lo2, _) = app.selection_bounds().expect("the second shape has bounds");

            assert!(
                lo2.x >= hi.x,
                "a {type_id} added beside the selection starts at {} but the selection reaches {}",
                lo2.x,
                hi.x
            );
            // One step of air between them, no more and no less.
            assert!(
                (lo2.x - hi.x - app.move_snap()).abs() < 1e-6,
                "a {type_id} left {} of air, not one step of {}",
                lo2.x - hi.x,
                app.move_snap()
            );
        }
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

    /// A project handed to the binary on the command line -- which is what a
    /// file association and a double-click do -- keeps the camera it was saved
    /// with. The starter scene has no camera worth keeping and is framed once
    /// it has bounds; the flag is what tells the two apart, and it used to be
    /// "this is the first evaluation of the session", which both are.
    #[test]
    fn a_project_opened_from_the_command_line_keeps_its_saved_camera() {
        let dir = temp_config_dir("cli-camera");
        let mut saver = app_in(dir.clone());
        let root = saver.scene.root();
        saver.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
        saver.scene.camera.target = Vec3::new(100.0, 100.0, 50.0);
        saver.scene.camera.distance = 250.0;
        saver.scene.camera.yaw = 33.0;
        saver.scene.camera.pitch = 12.0;
        let path = dir.join("camera.simple3d");
        saver.save_to(&path);
        let saved = saver.scene.camera;
        drop(saver);

        let ctx = egui::Context::default();
        let mut opened = App::with_config_dir(&ctx, Some(path.clone()), dir.clone());
        opened.viewport_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
        assert!(!opened.frame_when_evaluated, "opening a file from the command line asked for a reframe");
        // The reframe used to happen when the first evaluation landed, so the
        // camera has to still be the saved one *after* that.
        opened.reevaluate_for_test();
        assert_eq!(opened.scene.camera, saved, "the saved camera did not survive being opened from the command line");

        // And the empty starter document still frames itself, which is what the
        // flag exists for.
        let fresh = app_in(temp_config_dir("cli-camera-fresh"));
        assert!(fresh.frame_when_evaluated, "the starter scene will never be framed");
    }

    /// Exporting a selected boolean group writes the shape the viewport shows,
    /// not its operands. `Evaluated::node_meshes` holds primitives only, so
    /// merging those wrote the cutter of a difference back into the result.
    #[test]
    fn exporting_a_selected_difference_writes_the_cut_shape_not_its_operands() {
        let mut app = app_in(temp_config_dir("export-selection"));
        let root = app.scene.root();
        let group = app.scene.add_group(GroupOp::Difference, root, 0);
        let plate = app.scene.add_primitive("plate", group, 0).expect("the plate is in the registry");
        let cutter = app.scene.add_primitive("box", group, 1).expect("the box is in the registry");
        // A cutter that pokes out through the top and the bottom, so a merged
        // export shows up as a taller box than the cut result can be.
        if let Some(node) = app.scene.get_mut(cutter) {
            node.position = Vec3::new(0.0, 0.0, 0.0);
        }
        app.reevaluate_for_test();
        let (plate_lo, plate_hi) = app.evaluated.node_meshes[&plate].bounds().expect("the plate has bounds");
        let (cut_lo, cut_hi) = app.evaluated.node_meshes[&cutter].bounds().expect("the cutter has bounds");
        assert!(cut_lo.z < plate_lo.z && cut_hi.z > plate_hi.z, "this test needs a cutter taller than the plate");

        app.selection = vec![group];
        app.export_selection_only = true;
        let mesh = app.export_mesh();
        let (lo, hi) = mesh.bounds().expect("the selection exported nothing");
        assert!(
            lo.z >= plate_lo.z - 1e-6 && hi.z <= plate_hi.z + 1e-6,
            "the export reaches {lo:?}..{hi:?}, beyond the plate it cut -- the cutter was written out too"
        );
        assert!(mesh.manifold_issue().is_none(), "the exported selection is not a closed solid");

        // The whole scene exports as it always did.
        app.export_selection_only = false;
        let (whole_lo, whole_hi) = app.export_mesh().bounds().expect("the scene exported nothing");
        assert!((whole_lo.z - lo.z).abs() < 1e-6 && (whole_hi.z - hi.z).abs() < 1e-6);
    }
    /// Issue 58: a 3MF used to hold the whole scene as one component, so a
    /// slicer had nothing to pick apart. The objects an export separates are
    /// the scene's top-level nodes, each named and each the body it evaluates
    /// to -- a difference is its cut shape, not its two operands.
    #[test]
    fn separating_objects_writes_one_per_top_level_node_by_name() {
        let mut app = app_in(temp_config_dir("export-parts"));
        let root = app.scene.root();
        let plate = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
        let group = app.scene.add_group(GroupOp::Difference, root, 1);
        let base = app.scene.add_primitive("plate", group, 0).expect("the plate is in the registry");
        let cutter = app.scene.add_primitive("box", group, 1).expect("the box is in the registry");
        if let Some(node) = app.scene.get_mut(plate) {
            node.name = "Lid".into();
            node.position = Vec3::new(200.0, 0.0, 0.0);
        }
        if let Some(node) = app.scene.get_mut(group) {
            node.name = "Drilled base".into();
        }
        app.reevaluate_for_test();
        app.export_bodies = simple3d_export::BodyMode::TopLevel;

        let parts = app.export_parts();
        let names: Vec<&str> = parts.iter().map(|part| part.name.as_str()).collect();
        assert_eq!(names, vec!["Lid", "Drilled base"], "one object per top-level node, in the outliner's order");

        // The group is its cut shape: the cutter pokes out of the plate, so a
        // part holding the operands would be taller than the plate ever is.
        let (plate_lo, plate_hi) = app.evaluated.node_meshes[&base].bounds().expect("the plate has bounds");
        let (cut_lo, cut_hi) = app.evaluated.node_meshes[&cutter].bounds().expect("the cutter has bounds");
        assert!(cut_lo.z < plate_lo.z && cut_hi.z > plate_hi.z, "this test needs a cutter taller than the plate");
        let (lo, hi) = parts[1].mesh.bounds().expect("the group exported nothing");
        assert!(
            lo.z >= plate_lo.z - 1e-6 && hi.z <= plate_hi.z + 1e-6,
            "the group's part reaches {lo:?}..{hi:?}, beyond the plate it cut -- its operands were written out"
        );

        // Each is a closed solid on its own, which is what letting the exporter
        // verify them separately depends on.
        for part in &parts {
            assert!(part.mesh.manifold_issue().is_none(), "{} is not a closed solid", part.name);
        }

        // And the mode only takes effect where the format can hold it.
        app.export_bodies = simple3d_export::BodyMode::TopLevel;
        app.export_format = simple3d_export::Format::ThreeMf;
        assert_eq!(app.export_body_mode(), simple3d_export::BodyMode::TopLevel);
        app.export_format = simple3d_export::Format::StlBinary;
        assert_eq!(
            app.export_body_mode(),
            simple3d_export::BodyMode::One,
            "STL holds one body; separating them there is not a thing to promise"
        );
    }

    /// The third export mode: the user says what shares a body, the marks live
    /// on the nodes, and everything unmarked stays a body of its own -- so a
    /// project nobody has grouped exports exactly as "top level bodies" does.
    #[test]
    fn user_selected_bodies_merge_what_is_marked_and_split_what_is_opened() {
        use simple3d_core::scene::ExportBody;

        let mut app = app_in(temp_config_dir("export-bodies"));
        let root = app.scene.root();
        let plate = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
        let bracket = app.scene.add_primitive("box", root, 1).expect("the box is in the registry");
        let frame = app.scene.add_group(GroupOp::Union, root, 2);
        let left = app.scene.add_primitive("box", frame, 0).expect("the box is in the registry");
        let right = app.scene.add_primitive("box", frame, 1).expect("the box is in the registry");
        for (id, name, x) in
            [(plate, "Plate", 0.0), (bracket, "Bracket", 200.0), (left, "Left rail", 0.0), (right, "Right rail", 60.0)]
        {
            let node = app.scene.get_mut(id).unwrap();
            node.name = name.into();
            node.position = Vec3::new(x, 0.0, 0.0);
        }
        app.scene.get_mut(frame).unwrap().name = "Frame".into();
        app.scene.get_mut(frame).unwrap().position = Vec3::new(0.0, 300.0, 0.0);
        app.reevaluate_for_test();
        app.export_bodies = simple3d_export::BodyMode::Selected;

        // Untouched, it is the top-level answer exactly.
        let names: Vec<String> = app.export_parts().into_iter().map(|part| part.name).collect();
        assert_eq!(names, vec!["Plate", "Bracket", "Frame"], "an unmarked project is not the top-level grouping");

        // The same number on two shapes writes them as one body, named for
        // what is in it, and leaves the rest alone.
        app.scene.set_export_body(plate, Some(ExportBody::Shared(1)));
        app.scene.set_export_body(bracket, Some(ExportBody::Shared(1)));
        let parts = app.export_parts();
        let names: Vec<&str> = parts.iter().map(|part| part.name.as_str()).collect();
        assert_eq!(names, vec!["Plate + Bracket", "Frame"], "the two marked shapes did not become one body");
        // Merged, not merely appended: two solids in one object have to be one
        // closed surface or the export refuses them.
        assert!(parts[0].mesh.manifold_issue().is_none(), "the merged body is not a closed solid");

        // Splitting a group offers what is inside it, in its place.
        app.scene.set_export_body(frame, Some(ExportBody::Split));
        let names: Vec<String> = app.export_parts().into_iter().map(|part| part.name).collect();
        assert_eq!(names, vec!["Plate + Bracket", "Left rail", "Right rail"], "splitting the group did not reach in");

        // And a body reaches across the tree: a rail can join the plate.
        app.scene.set_export_body(left, Some(ExportBody::Shared(1)));
        let names: Vec<String> = app.export_parts().into_iter().map(|part| part.name).collect();
        assert_eq!(names, vec!["Plate + Bracket + Left rail", "Right rail"], "a body did not reach into the group");

        // Closing the group again takes the marks inside it with it, rather
        // than leaving one to spring back the next time it is opened.
        app.scene.set_export_body(frame, None);
        assert_eq!(app.scene.node(left).export_body, None, "a mark survived the group being closed over it");
        let names: Vec<String> = app.export_parts().into_iter().map(|part| part.name).collect();
        assert_eq!(names, vec!["Plate + Bracket", "Frame"]);
    }

    /// The point of choosing the bodies is not having to choose them again: a
    /// shape added afterwards is the only thing left to decide.
    #[test]
    fn a_shape_added_later_is_the_only_body_left_to_place() {
        use simple3d_core::scene::ExportBody;

        let mut app = app_in(temp_config_dir("export-bodies-reexport"));
        let root = app.scene.root();
        let plate = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
        let bracket = app.scene.add_primitive("box", root, 1).expect("the box is in the registry");
        app.scene.get_mut(plate).unwrap().name = "Plate".into();
        app.scene.get_mut(bracket).unwrap().name = "Bracket".into();
        app.scene.get_mut(bracket).unwrap().position = Vec3::new(200.0, 0.0, 0.0);
        app.export_bodies = simple3d_export::BodyMode::Selected;
        app.scene.set_export_body(plate, Some(ExportBody::Shared(1)));
        app.scene.set_export_body(bracket, Some(ExportBody::Shared(1)));
        app.reevaluate_for_test();
        assert_eq!(app.export_parts().len(), 1, "the two marked shapes are one body");

        // Modelling carries on: a new shape, and the marks that were already
        // made still stand.
        let lid = app.scene.add_primitive("box", root, 2).expect("the box is in the registry");
        app.scene.get_mut(lid).unwrap().name = "Lid".into();
        app.scene.get_mut(lid).unwrap().position = Vec3::new(400.0, 0.0, 0.0);
        app.reevaluate_for_test();
        let names: Vec<String> = app.export_parts().into_iter().map(|part| part.name).collect();
        assert_eq!(names, vec!["Plate + Bracket", "Lid"], "the grouping did not survive the scene changing");

        // And placing it is one choice, not a re-grouping of everything.
        app.scene.set_export_body(lid, Some(ExportBody::Shared(1)));
        let names: Vec<String> = app.export_parts().into_iter().map(|part| part.name).collect();
        assert_eq!(names, vec!["Plate + Bracket + Lid"]);
    }

    /// End to end, through the writer the export job really calls: the bodies
    /// the user grouped come out as the components of the file.
    #[test]
    fn the_written_3mf_holds_one_named_component_per_chosen_body() {
        use simple3d_core::scene::ExportBody;

        let dir = temp_config_dir("export-bodies-file");
        let mut app = app_in(dir.clone());
        let root = app.scene.root();
        let plate = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
        let bracket = app.scene.add_primitive("box", root, 1).expect("the box is in the registry");
        let lid = app.scene.add_primitive("box", root, 2).expect("the box is in the registry");
        for (id, name, x) in [(plate, "Plate", 0.0), (bracket, "Bracket", 200.0), (lid, "Lid", 400.0)] {
            let node = app.scene.get_mut(id).unwrap();
            node.name = name.into();
            node.position = Vec3::new(x, 0.0, 0.0);
        }
        app.reevaluate_for_test();
        app.export_bodies = simple3d_export::BodyMode::Selected;
        app.scene.set_export_body(plate, Some(ExportBody::Shared(1)));
        app.scene.set_export_body(bracket, Some(ExportBody::Shared(1)));

        // Exactly what `start_export` hands the export job, written by exactly
        // the call the job makes.
        let parts = app.export_parts();
        let meshes: Vec<(String, simple3d_geom::Mesh)> = parts.into_iter().map(|p| (p.name, p.mesh)).collect();
        let borrowed: Vec<simple3d_export::Part<'_>> =
            meshes.iter().map(|(name, mesh)| simple3d_export::Part { name, mesh }).collect();
        let options = simple3d_export::Options {
            format: simple3d_export::Format::ThreeMf,
            bodies: app.export_body_mode(),
            ..Default::default()
        };
        let path = dir.join("bodies.3mf");
        simple3d_export::write_parts(&path, &borrowed, &options, &mut |_| true).expect("the export should succeed");

        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).to_string();
        assert_eq!(text.matches("<object id=").count(), 2, "{text}");
        assert!(text.contains("name=\"Plate + Bracket\""), "the merged body is not named for what is in it");
        assert!(text.contains("name=\"Lid\""), "the untouched shape lost its own body");
        assert_eq!(text.matches("<item objectid=").count(), 2, "both bodies have to be in the build");
    }

    /// A difference is one new surface: its operands are not shapes the result
    /// still holds, so no export can write one of them as a body.
    #[test]
    fn a_boolean_that_fuses_its_operands_cannot_be_split_into_them() {
        use simple3d_core::scene::ExportBody;

        let mut app = app_in(temp_config_dir("export-bodies-fused"));
        let root = app.scene.root();
        let cut = app.scene.add_group(GroupOp::Difference, root, 0);
        let base = app.scene.add_primitive("plate", cut, 0).expect("the plate is in the registry");
        let bore = app.scene.add_primitive("box", cut, 1).expect("the box is in the registry");
        app.scene.get_mut(cut).unwrap().name = "Drilled".into();
        app.reevaluate_for_test();
        app.export_bodies = simple3d_export::BodyMode::Selected;

        assert!(!app.scene.can_split_for_export(cut), "a difference offered to be split into its operands");
        assert!(app.scene.can_split_for_export(app.scene.root()), "the scene's own top level is a union");

        // Even asked to directly -- a file edited by hand, or a union turned
        // into a difference after it was split -- the export writes the shape
        // the viewport shows rather than the operands that made it.
        app.scene.get_mut(cut).unwrap().export_body = Some(ExportBody::Split);
        app.scene.get_mut(bore).unwrap().export_body = Some(ExportBody::Shared(2));
        let parts = app.export_parts();
        let names: Vec<&str> = parts.iter().map(|part| part.name.as_str()).collect();
        assert_eq!(names, vec!["Drilled"], "the difference was taken apart into its operands");

        let (plate_lo, plate_hi) = app.evaluated.node_meshes[&base].bounds().expect("the plate has bounds");
        let (lo, hi) = parts[0].mesh.bounds().expect("the difference exported nothing");
        assert!(
            lo.z >= plate_lo.z - 1e-6 && hi.z <= plate_hi.z + 1e-6,
            "the exported body reaches {lo:?}..{hi:?}, past the plate it cut"
        );
    }

    #[test]
    fn moving_among_siblings_moves_everything_that_is_selected() {
        // Issue 41: with more than one node selected, only the primary used to
        // move -- which reads as the command doing nothing to the rest.
        let mut app = headless_app();
        let root = app.scene.root();
        let first = app.scene.add_primitive("box", root, 0).unwrap();
        let second = app.scene.add_primitive("box", root, 1).unwrap();
        let third = app.scene.add_primitive("box", root, 2).unwrap();
        let plate = app.scene.node(root).children[3];

        app.selection = vec![second, third];
        assert!(app.can_reorder(1), "there is a sibling below to move past");
        app.run(Command::MoveDown);
        assert_eq!(app.scene.node(root).children, vec![first, plate, second, third]);

        // And the two of them together stop at the end rather than one of them
        // running past the other.
        assert!(!app.can_reorder(1));
        app.run(Command::MoveDown);
        assert_eq!(app.scene.node(root).children, vec![first, plate, second, third]);

        app.selection = vec![first];
        assert!(!app.can_reorder(-1), "already first among its siblings");
        assert!(app.can_reorder(1));
    }

    #[test]
    fn a_group_can_be_made_empty_and_have_its_operator_set_where_the_tree_is() {
        // Issues 37 and 40.
        let mut app = headless_app();
        let plate = app.primary().unwrap();
        app.add_node_at(plate, None, GroupOp::Union);
        let group = app.primary().unwrap();
        assert!(app.scene.node(group).is_group());
        assert!(app.scene.node(group).children.is_empty(), "the new group is empty");
        assert_eq!(app.scene.node(group).parent, Some(app.scene.root()), "beside the node it was made from");

        // Made *inside* a group, since a group is somewhere things can go.
        app.add_node_at(group, None, GroupOp::Union);
        let inner = app.primary().unwrap();
        assert_eq!(app.scene.node(inner).parent, Some(group));

        app.set_group_op(group, GroupOp::Difference);
        assert_eq!(app.scene.node(group).group_op(), Some(GroupOp::Difference));
    }

    #[test]
    fn dragging_one_row_of_a_multi_selection_moves_the_whole_selection() {
        // Issue 43: a drag that started on a selected row carries everything
        // selected, in document order, and leaves it selected where it lands.
        let mut app = headless_app();
        let root = app.scene.root();
        let plate = app.primary().unwrap();
        let second = app.scene.add_primitive("box", root, 1).unwrap();
        let third = app.scene.add_primitive("cylinder", root, 2).unwrap();
        let group = app.scene.add_group(GroupOp::Union, root, 3);

        app.selection = vec![third, plate];
        // Grabbed on a row that is part of the selection: all of it travels,
        // in the order the tree has it rather than the order it was clicked.
        assert_eq!(app.dragged_nodes(third), vec![plate, third]);
        // Grabbed on a row that is not: that row alone, and the selection is
        // not what the gesture was about.
        assert_eq!(app.dragged_nodes(second), vec![second]);

        app.outliner_drag = Some(third);
        app.drop_target = Some(DropTarget { parent: group, index: 0, into: Some(group) });
        crate::panel_outliner::finish_drag(&mut app);
        assert_eq!(app.scene.node(group).children, vec![plate, third]);
        assert_eq!(app.scene.node(root).children, vec![second, group]);
        assert!(app.is_selected(plate) && app.is_selected(third), "the load was dropped out of the selection");
        assert_eq!(app.selection.len(), 2);

        // One undo step for the whole drag, and it puts all of it back.
        app.run(Command::Undo);
        assert_eq!(app.scene.node(root).children, vec![plate, second, third, group]);
    }

    #[test]
    fn a_selection_that_holds_a_group_and_its_child_drags_as_the_group_alone() {
        let mut app = headless_app();
        let root = app.scene.root();
        let plate = app.primary().unwrap();
        let group = app.scene.add_group(GroupOp::Union, root, 1);
        let inner = app.scene.add_primitive("box", group, 0).unwrap();
        let target = app.scene.add_group(GroupOp::Union, root, 2);

        app.selection = vec![group, inner];
        assert_eq!(app.dragged_nodes(inner), vec![group], "the child was torn out of the group carrying it");

        app.outliner_drag = Some(group);
        app.drop_target = Some(DropTarget { parent: target, index: 0, into: Some(target) });
        crate::panel_outliner::finish_drag(&mut app);
        assert_eq!(app.scene.node(target).children, vec![group]);
        assert_eq!(app.scene.node(group).children, vec![inner], "the child did not travel with its group");
        assert_eq!(app.scene.node(root).children, vec![plate, target]);
    }

    #[test]
    fn the_outliner_can_add_a_group_or_any_primitive_where_the_row_is() {
        // Issue 44: the row's own Add menu, which puts a new node inside the
        // group it was opened on and beside anything else.
        let mut app = headless_app();
        let plate = app.primary().unwrap();

        app.add_node_at(plate, Some("cylinder"), GroupOp::Union);
        let cylinder = app.primary().unwrap();
        assert_eq!(app.scene.node(cylinder).parent, Some(app.scene.root()), "beside the node it was added from");
        assert_eq!(app.scene.node(cylinder).spec().map(|s| s.type_id), Some("cylinder"));

        app.add_node_at(cylinder, None, GroupOp::Difference);
        let group = app.primary().unwrap();
        assert_eq!(app.scene.node(group).group_op(), Some(GroupOp::Difference));

        // Into the group, because a group is somewhere things can go.
        app.add_node_at(group, Some("box"), GroupOp::Union);
        let boxed = app.primary().unwrap();
        assert_eq!(app.scene.node(boxed).parent, Some(group));

        // Every shape the palette offers is reachable from the same menu.
        for spec in simple3d_core::primitive::REGISTRY.iter() {
            app.add_node_at(group, Some(spec.type_id), GroupOp::Union);
            let added = app.primary().unwrap();
            assert_eq!(
                app.scene.node(added).spec().map(|s| s.type_id),
                Some(spec.type_id),
                "{} could not be added from the outliner",
                spec.type_id
            );
        }
    }

    #[test]
    fn a_collapsed_group_hides_its_children_and_a_selection_opens_it_again() {
        // Issue 33.
        let mut app = headless_app();
        let plate = app.primary().unwrap();
        app.run(Command::Group);
        let group = app.primary().unwrap();
        assert!(crate::panel_outliner::visible_rows(&app).contains(&plate));

        app.set_collapsed(group, true);
        let rows = crate::panel_outliner::visible_rows(&app);
        assert!(rows.contains(&group), "the group itself is still a row");
        assert!(!rows.contains(&plate), "a collapsed group still drew its children");

        // Selecting something inside it -- from the viewport, say -- has to
        // bring it back into view.
        app.select_only(plate);
        assert!(crate::panel_outliner::visible_rows(&app).contains(&plate));
    }

    #[test]
    fn painting_remembers_the_colour_and_only_a_ghost_is_drawn_as_one() {
        let mut app = headless_app();
        let id = app.primary().expect("the plate is selected");
        assert!(app.settings.recent_colours.is_empty());

        // Issue 29: a colour used once should be offered again, wherever a
        // colour is chosen.
        app.paint(&[id], Some(Colour([0x2E, 0x9A, 0xFF])), None);
        assert_eq!(app.settings.recent_colours, vec![[0x2E, 0x9A, 0xFF]]);
        app.paint(&[id], Some(Colour([0x77, 0x11, 0x22])), None);
        assert_eq!(app.settings.recent_colours[0], [0x77, 0x11, 0x22]);
        // Clearing paints nothing, so it remembers nothing.
        app.paint(&[id], None, None);
        assert_eq!(app.settings.recent_colours.len(), 2);

        // Issue 35: one of the eight presets is already a click away on the
        // row above, so using it is not what "recent" is for.
        let preset = crate::theme::PAINT_PRESETS[6].1;
        app.paint(&[id], Some(Colour([preset.r(), preset.g(), preset.b()])), None);
        assert_eq!(app.settings.recent_colours.len(), 2, "a preset was remembered as a recent colour");
        assert_eq!(app.custom_recent_colours(), vec![[0x77, 0x11, 0x22], [0x2E, 0x9A, 0xFF]]);

        // Issue 35 again: a drag through the picker paints on every frame it
        // moves, and each of those frames used to take a slot -- which is how
        // the row ended up holding eight shades of the same colour. The whole
        // run is one choice, so only where it stopped is remembered.
        for step in 0..6_u8 {
            app.paint(&[id], Some(Colour([0x10 + step, 0x40, 0x90])), Some("colour"));
        }
        assert_eq!(
            app.custom_recent_colours(),
            vec![[0x15, 0x40, 0x90], [0x77, 0x11, 0x22], [0x2E, 0x9A, 0xFF]],
            "a single drag through the picker filled the recent row"
        );
        // A drag that ends and a new one that begins are two choices.
        app.history.close();
        app.paint(&[id], Some(Colour([0x01, 0x02, 0x03])), Some("colour"));
        assert_eq!(app.custom_recent_colours()[..2], [[0x01, 0x02, 0x03], [0x15, 0x40, 0x90]]);

        // Issue 21: the three states, and which of them the viewport is asked
        // to draw as a ghost.
        assert!(app.ghosts().is_empty());
        app.scene.get_mut(id).unwrap().set_visibility(Visibility::Hidden);
        assert!(app.ghosts().is_empty(), "a hidden node is gone, not translucent");
        app.scene.get_mut(id).unwrap().set_visibility(Visibility::Ghost);
        assert_eq!(app.ghosts(), vec![id]);
        app.scene.get_mut(id).unwrap().set_visibility(Visibility::Visible);
        assert!(app.ghosts().is_empty());
    }

    /// Several documents open at once, each with its own model, selection,
    /// history and camera (issue 61). What tabs are for: switching has to be a
    /// change of document, not a change of what one document is showing.
    #[test]
    fn each_tab_keeps_its_own_document() {
        let mut app = app_in(temp_config_dir("tabs-own-document"));
        let root = app.scene.root();
        let plate = app.scene.add_primitive("plate", root, 0).unwrap();
        app.select_only(plate);
        app.scene.camera.distance = 321.0;

        app.run(Command::New);
        assert_eq!(app.tab_count(), 2, "File > New did not open a second document");
        assert_eq!(app.active, 1);
        assert_eq!(app.scene.depth_first(), vec![app.scene.root()], "the new tab opened on the other tab's model");
        assert!(app.selection.is_empty(), "the new tab inherited a selection");

        let second_root = app.scene.root();
        let cube = app.scene.add_primitive("box", second_root, 0).unwrap();
        app.select_only(cube);

        app.activate_tab(0);
        assert_eq!(app.selection, vec![plate], "the first document lost its selection while it was away");
        assert_eq!(app.scene.camera.distance, 321.0, "the first document lost its camera while it was away");
        assert_eq!(app.scene.depth_first().len(), 2, "the first document lost its model while it was away");

        app.cycle_tab(1);
        assert_eq!(app.active, 1);
        assert_eq!(app.selection, vec![cube], "the second document lost its selection while it was away");
        app.cycle_tab(1);
        assert_eq!(app.active, 0, "walking off the end of the row did not wrap");
    }

    /// Opening a file uses an untouched document rather than leaving an empty
    /// tab behind, and a file that is already open is shown rather than opened
    /// a second time (issue 61).
    #[test]
    fn opening_a_project_reuses_a_scratch_tab_and_never_opens_one_file_twice() {
        let dir = temp_config_dir("tabs-open");
        let mut app = app_in(dir.clone());
        let root = app.scene.root();
        app.scene.add_primitive("plate", root, 0).unwrap();
        let first = dir.join("first.simple3d");
        app.save_to(&first);

        let second = dir.join("second.simple3d");
        std::fs::write(&second, project::to_string(&Scene::new())).unwrap();

        // The document on screen has been saved, so it is not scratch space: the
        // second file gets a tab of its own.
        app.open_path(&second);
        assert_eq!(app.tab_count(), 2);
        assert_eq!(app.path.as_deref(), Some(second.as_path()));

        // Both files are open now, so neither opens again.
        app.open_path(&first);
        assert_eq!(app.tab_count(), 2, "a file that was already open opened a second time");
        assert_eq!(app.active, 0);
        app.open_path(&second);
        assert_eq!(app.tab_count(), 2);
        assert_eq!(app.active, 1);

        // A new, untouched document is scratch space: a file opened from it
        // lands in that tab rather than in one more.
        app.run(Command::New);
        assert_eq!(app.tab_count(), 3);
        let third = dir.join("third.simple3d");
        std::fs::write(&third, project::to_string(&Scene::new())).unwrap();
        app.open_path(&third);
        assert_eq!(app.tab_count(), 3, "an empty, never-saved document was left behind as its own tab");
        assert_eq!(app.path.as_deref(), Some(third.as_path()));
    }

    /// Closing asks before it throws work away, and the last document does not
    /// close: it empties, so there is always somewhere to work (issue 61).
    #[test]
    fn closing_a_tab_asks_about_changes_and_the_last_one_empties_instead_of_vanishing() {
        let mut app = app_in(temp_config_dir("tabs-close"));
        app.run(Command::New);
        app.add_node(Some("plate"), GroupOp::Union);
        assert!(app.unsaved());

        app.run(Command::CloseTab);
        assert_eq!(app.modal, Modal::ConfirmCloseTab, "closing a modified document asked nothing");
        assert_eq!(app.tab_count(), 2, "the document closed before the question was answered");
        app.cancel_close_tab();
        assert_eq!(app.modal, Modal::None);
        assert_eq!(app.tab_count(), 2, "cancelling the question closed the document anyway");

        app.run(Command::CloseTab);
        app.confirm_close_tab();
        assert_eq!(app.tab_count(), 1);
        assert_eq!(app.active, 0);
        assert_eq!(app.modal, Modal::None);

        // The one remaining document: closing it leaves an empty one open.
        app.add_node(Some("plate"), GroupOp::Union);
        app.close_tab_now(0);
        assert_eq!(app.tab_count(), 1, "the last document closed and left no document at all");
        assert_eq!(app.scene.depth_first(), vec![app.scene.root()]);
        assert!(!app.unsaved(), "the emptied document counts as modified");
    }

    /// Quitting asks about every open document, not only the one on screen
    /// (issue 61) -- the changes in a tab that is not showing are exactly the
    /// ones that would be lost without anybody noticing.
    #[test]
    fn quitting_asks_about_a_document_that_is_not_on_screen() {
        let mut app = app_in(temp_config_dir("tabs-quit"));
        app.add_node(Some("plate"), GroupOp::Union);
        assert!(app.unsaved());
        app.run(Command::New);
        assert!(!app.unsaved(), "the new document is untouched");
        assert!(app.any_unsaved(), "the changes in the other tab were not noticed");

        app.request_quit();
        assert_eq!(app.modal, Modal::ConfirmQuit);
        assert!(!app.quit_now, "quitting went ahead with unsaved changes in another tab");
    }

    /// The row of tabs, and the question closing one asks, both draw.
    #[test]
    fn a_row_of_tabs_draws_and_so_does_the_question_a_close_asks() {
        let mut app = headless_app();
        app.run(Command::New);
        app.run(Command::New);
        draw_one_frame(&mut app);
        app.activate_tab(0);
        draw_one_frame(&mut app);
        app.pending_close = Some(0);
        app.modal = Modal::ConfirmCloseTab;
        draw_one_frame(&mut app);
    }

    #[test]
    fn a_measurement_reads_distance_delta_and_the_direction_it_points() {
        // A 3-4-0 span: five long, level, and running mostly along +Y from +X.
        let m = Measurement::between(Vec3::ZERO, Vec3::new(3.0, 4.0, 0.0));
        assert!((m.distance - 5.0).abs() < 1e-9);
        assert_eq!(m.delta, Vec3::new(3.0, 4.0, 0.0));
        assert!(m.inclination_deg.abs() < 1e-9, "a level span should not be inclined: {}", m.inclination_deg);
        assert!((m.bearing_deg - 53.13010).abs() < 1e-3, "bearing was {}", m.bearing_deg);

        // Straight up: ninety degrees of incline, and no bearing to speak of.
        let up = Measurement::between(Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0));
        assert!((up.inclination_deg - 90.0).abs() < 1e-9);
        assert_eq!(up.bearing_deg, 0.0);

        // A zero-length span is level rather than undefined, so the readout never
        // shows NaN while a second point is being aimed.
        let none = Measurement::between(Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(none.distance, 0.0);
        assert_eq!(none.inclination_deg, 0.0);
    }

    #[test]
    fn the_measure_tool_takes_two_points_and_the_third_begins_a_new_span() {
        let mut measure = Measure::default();
        assert!(measure.span().is_none());
        measure.add(MeasurePoint { at: Vec3::ZERO, kind: None });
        assert!(measure.span().is_none(), "one point is not a span");
        measure.add(MeasurePoint { at: Vec3::new(10.0, 0.0, 0.0), kind: Some(crate::snap::FeatureKind::Vertex) });
        assert!(measure.span().is_some(), "two points make a span");

        // A third point starts fresh rather than piling up, so the tool flows
        // from one measurement to the next.
        measure.add(MeasurePoint { at: Vec3::new(5.0, 5.0, 0.0), kind: None });
        assert_eq!(measure.points.len(), 1);
        assert!(measure.span().is_none());
    }

    #[test]
    fn the_measure_tool_snaps_a_click_to_a_bodys_vertex() {
        // The whole point of picking features rather than raw surface hits: a
        // click near a box corner reports the corner exactly.
        let mut app = headless_app();
        let root = app.scene.root();
        let id = app.scene.add_primitive("box", root, 0).unwrap();
        app.scene.get_mut(id).unwrap().position = Vec3::new(0.0, 0.0, 0.0);
        app.reevaluate_for_test();

        let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
        // A box is 20mm to a side by default; aim a hair off its +X +Y +Z corner.
        let (lo, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();
        let corner = Vec3::new(hi.x, hi.y, hi.z);
        let (screen, _) = view.project(corner).unwrap();
        let point = app.measure_point_at(&view, screen + egui::vec2(3.0, 3.0)).expect("a point under the cursor");
        assert_eq!(point.kind, Some(crate::snap::FeatureKind::Vertex), "the click did not catch the corner");
        assert!((point.at - corner).length() < 1e-6, "snapped to {:?}, not the corner {:?}", point.at, corner);
        let _ = lo;
    }

    #[test]
    fn a_measure_click_between_two_corners_catches_the_edge_itself() {
        // Issue 78: aiming at the middle of nothing in particular, part-way along
        // an edge, used to fall through to the surface hit under the pointer.
        let mut app = headless_app();
        let root = app.scene.root();
        let id = app.scene.add_primitive("box", root, 0).unwrap();
        app.reevaluate_for_test();

        let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
        let (lo, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();
        // A third of the way along the top +Y edge: near no corner and not the
        // midpoint either, so only the edge itself can answer.
        let (a, b) = (Vec3::new(lo.x, hi.y, hi.z), Vec3::new(hi.x, hi.y, hi.z));
        let along = a + (b - a) * (1.0 / 3.0);
        let (screen, _) = view.project(along).unwrap();
        let point = app.measure_point_at(&view, screen).expect("a point under the cursor");
        assert_eq!(point.kind, Some(crate::snap::FeatureKind::Edge), "caught {:?}, not the edge", point.kind);
        assert!((point.at - along).length() < 0.2, "caught {:?}, a third along is {:?}", point.at, along);

        // A corner still wins where one is in reach, so aiming at a corner never
        // lands part-way along the edge beside it.
        let (screen, _) = view.project(b).unwrap();
        let point = app.measure_point_at(&view, screen + egui::vec2(2.0, 2.0)).unwrap();
        assert_eq!(point.kind, Some(crate::snap::FeatureKind::Vertex));
    }

    #[test]
    fn a_measure_click_catches_where_an_axis_crosses_a_body() {
        // Issue 78: the axes run through the model, and where one leaves a body
        // is a place to measure from even though the mesh has no corner there.
        let mut app = headless_app();
        let root = app.scene.root();
        let id = app.scene.add_primitive("box", root, 0).unwrap();
        // Off to one side, so where the Z axis leaves the top face is nowhere
        // near that face's own centre and only the crossing can answer.
        app.scene.get_mut(id).unwrap().position = Vec3::new(6.0, 0.0, 0.0);
        app.reevaluate_for_test();

        let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
        let (_, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();
        let crossing = Vec3::new(0.0, 0.0, hi.z);
        let (screen, _) = view.project(crossing).unwrap();
        let point = app.measure_point_at(&view, screen).expect("a point under the cursor");
        assert_eq!(
            point.kind,
            Some(crate::snap::FeatureKind::AxisCrossing),
            "the Z axis leaving the top face was not catchable; caught {:?}",
            point.kind
        );
        assert!((point.at - crossing).length() < 1e-6);

        // With the Z axis turned off there is nothing on screen there to catch:
        // whatever the click then lands on, it is not that crossing.
        app.scene.settings.axes_visible[2] = false;
        let point = app.measure_point_at(&view, screen).unwrap();
        assert_ne!(
            point.kind,
            Some(crate::snap::FeatureKind::AxisCrossing),
            "an axis that is not shown was still snapped to"
        );
    }

    #[test]
    fn a_measure_click_catches_a_world_axis_anywhere_along_it() {
        // The follow-up to issue 78: the crossings alone left most of an axis
        // with nothing to catch, so the axes are caught as lines -- anywhere
        // along them, the way an edge is.
        let mut app = headless_app();
        let root = app.scene.root();
        app.scene.add_primitive("box", root, 0).unwrap();
        app.reevaluate_for_test();
        let view = crate::view::View::new(app.scene.camera, app.viewport_rect);

        // Well clear of the box, where no feature of any body is in reach.
        let on_axis = Vec3::new(30.0, 0.0, 0.0);
        let (screen, _) = view.project(on_axis).unwrap();
        let point = app.measure_point_at(&view, screen).expect("a point under the cursor");
        assert_eq!(point.kind, Some(crate::snap::FeatureKind::Axis), "caught {:?}", point.kind);
        assert!((point.at - on_axis).length() < 0.2, "caught {:?}, not the point on the axis", point.at);
        // It really is *on* the axis, not merely near it.
        assert!(point.at.y.abs() < 1e-6 && point.at.z.abs() < 1e-6, "{:?} is off the X axis", point.at);

        // A little to the side of the line there is nothing to catch, and the
        // click falls through to the ground.
        let beside = app.measure_point_at(&view, screen + egui::vec2(0.0, 40.0)).unwrap();
        assert_eq!(beside.kind, None, "the catch reached far past the axis");

        // An axis that is not shown is not a line to catch either.
        app.scene.settings.axes_visible[0] = false;
        let hidden = app.measure_point_at(&view, screen).unwrap();
        assert_ne!(hidden.kind, Some(crate::snap::FeatureKind::Axis), "a hidden axis was still snapped to");
    }

    #[test]
    fn an_axis_is_only_caught_where_it_is_actually_drawn() {
        // The line is cut out of the material it runs through and covered by
        // whatever is in front of it, so the catch must stop at the surface
        // rather than following the axis on through the body.
        let mut app = headless_app();
        let root = app.scene.root();
        let id = app.scene.add_primitive("box", root, 0).unwrap();
        app.reevaluate_for_test();
        let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
        let (lo, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();

        // Dead centre of a body the axes run through: they are inside material
        // here, where the renderer cuts the line out altogether, so there is no
        // line to catch. Asked of the line pass directly, since a face centre
        // happens to sit near this spot and would answer first.
        let middle = (lo + hi) * 0.5;
        let (screen, _) = view.project(middle).unwrap();
        assert!(
            app.nearest_line_point(&view, screen).is_none_or(|(_, kind, _)| kind != crate::snap::FeatureKind::Axis),
            "the catch followed an axis into the middle of a body, where no line is drawn"
        );
        let inside = app.measure_point_at(&view, screen).expect("a point under the cursor");
        assert!(
            !matches!(inside.kind, Some(crate::snap::FeatureKind::Axis | crate::snap::FeatureKind::AxisCrossing)),
            "an axis inside a body was caught: {inside:?}"
        );

        // The crossing the camera can see is still caught -- this is about what
        // is covered, not about axes in general.
        let near_top = Vec3::new(0.0, 0.0, hi.z);
        assert!(app.in_clear_view(&view, near_top), "the top crossing is covered in this view; pick another point");
        let (screen, _) = view.project(near_top).unwrap();
        let seen = app.measure_point_at(&view, screen).unwrap();
        assert!(seen.kind.is_some() && (seen.at - near_top).length() < 1e-6, "the visible crossing was lost: {seen:?}");

        // The crossing on the underside is behind the body from here, and the
        // line arriving at it is not drawn either.
        let far = Vec3::new(0.0, 0.0, lo.z);
        assert!(!app.in_clear_view(&view, far), "the far crossing is not covered in this view; pick another point");
        let (screen, _) = view.project(far).unwrap();
        let hidden = app.measure_point_at(&view, screen).unwrap();
        assert!(
            !matches!(hidden.kind, Some(crate::snap::FeatureKind::Axis | crate::snap::FeatureKind::AxisCrossing)),
            "an axis behind the body was caught through it: {hidden:?}"
        );
    }

    #[test]
    fn either_end_of_a_span_can_be_typed_rather_than_clicked() {
        // Issue 78: the property panel's start and end fields write here.
        let mut measure = Measure::default();
        // Nothing is placed yet, so the end cannot be: it would be a point with
        // nothing to measure to.
        measure.set_point(1, Vec3::new(5.0, 0.0, 0.0));
        assert!(measure.points.is_empty());

        measure.set_point(0, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(measure.points.len(), 1);
        measure.set_point(1, Vec3::new(4.0, 2.0, 3.0));
        let (a, b) = measure.span().expect("both ends are down");
        assert_eq!(a.at, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(b.at, Vec3::new(4.0, 2.0, 3.0));

        // Typing a coordinate over an end that had caught a feature makes it an
        // exact point rather than leaving it claiming a feature it has left.
        measure.points[0].kind = Some(crate::snap::FeatureKind::Vertex);
        measure.set_point(0, Vec3::ZERO);
        assert_eq!(measure.points[0].kind, None);
        let distance = Measurement::between(measure.points[0].at, measure.points[1].at).distance;
        assert!((distance - 29.0_f64.sqrt()).abs() < 1e-9, "the span reads {distance} from the typed ends");
    }

    #[test]
    fn a_placed_end_can_be_taken_back_off_one_at_a_time() {
        // A right-click in the viewport undoes the last placement, back to
        // nothing placed at all.
        let mut app = headless_app();
        app.toggle_measure();
        app.measure_click(MeasurePoint { at: Vec3::ZERO, kind: None });
        app.measure_click(MeasurePoint { at: Vec3::new(10.0, 0.0, 0.0), kind: None });
        assert!(app.measure.span().is_some());

        app.measure_unplace();
        assert_eq!(app.measure.points.len(), 1, "the end did not come off");
        assert!(app.measure.span().is_none());
        app.measure_unplace();
        assert!(app.measure.points.is_empty(), "the start did not come off");
        // And with nothing placed it is harmless.
        app.measure_unplace();
        assert!(app.measure.points.is_empty());
        assert!(app.measure.active, "taking a point back also put the tool away");
    }

    #[test]
    fn the_measure_overlay_draws_a_placed_span_without_panicking() {
        let mut app = headless_app();
        app.measure.active = true;
        app.measure.add(MeasurePoint { at: Vec3::ZERO, kind: Some(crate::snap::FeatureKind::Vertex) });
        app.measure
            .add(MeasurePoint { at: Vec3::new(20.0, 8.0, 5.0), kind: Some(crate::snap::FeatureKind::FaceCentre) });
        draw_one_frame(&mut app);
        // And with only one end down, where the live preview line is drawn.
        app.measure.clear();
        app.measure.add(MeasurePoint { at: Vec3::ZERO, kind: None });
        draw_one_frame(&mut app);
    }

    #[test]
    fn picking_a_transform_tool_puts_the_measure_tool_away() {
        let mut app = headless_app();
        app.run(Command::MeasureTool);
        assert!(app.measure.active);
        app.measure.add(MeasurePoint { at: Vec3::ZERO, kind: None });
        app.run(Command::ModeRotate);
        assert!(!app.measure.active, "the measure tool held the pointer after a transform tool was chosen");
        assert!(app.measure.points.is_empty(), "its span was left hanging in the scene");
    }

    #[test]
    fn the_pattern_tool_wraps_the_selection_and_the_editor_draws() {
        // Issue 67: the creation tool wraps what is selected in a pattern node
        // that repeats it, selects the pattern, and the property editor's Pattern
        // section draws without panicking.
        let mut app = headless_app();
        let plate = app.primary().unwrap();
        app.run(Command::Pattern);
        let pat = app.primary().unwrap();
        assert!(app.scene.node(pat).is_pattern(), "the tool did not make a pattern");
        assert_eq!(app.scene.node(pat).children, vec![plate], "the shape was not put under the pattern");
        app.reevaluate_for_test();
        draw_one_frame(&mut app);

        // Turning it into a circular pattern and drawing again exercises the
        // choice-gated fields the editor shows per kind.
        app.scene
            .get_mut(pat)
            .unwrap()
            .params_mut()
            .unwrap()
            .insert("kind".into(), simple3d_core::primitive::ParamValue::Choice(2));
        app.reevaluate_for_test();
        draw_one_frame(&mut app);

        // With nothing selected the tool drops a bare pattern to fill later.
        app.clear_selection();
        app.run(Command::Pattern);
        assert!(app.scene.node(app.primary().unwrap()).is_pattern());
    }

    #[test]
    fn a_linear_patterns_spacing_is_laid_out_by_a_handle_that_follows_the_kind() {
        use simple3d_core::primitive::ParamValue;
        let free = gizmo::Mods { free: true, ..Default::default() };
        let mut app = headless_app();
        app.run(Command::Pattern);
        let pat = app.primary().unwrap();
        // Linear by default: the viewport handle drives its count and the three
        // components of its run.
        assert_eq!(app.pattern_spacing_keys(pat), Some(("count", &["step_x", "step_y", "step_z"][..])));
        app.set_pattern_step(pat, 42.0, free);
        assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(42.0)));
        app.reevaluate_for_test();
        draw_one_frame(&mut app);

        // A kind with no straight run -- a mirror -- offers no spacing handle.
        app.scene.get_mut(pat).unwrap().params_mut().unwrap().insert("kind".into(), ParamValue::Choice(3));
        assert_eq!(app.pattern_spacing_keys(pat), None);

        // Negative drags cannot push the step below zero.
        app.scene.get_mut(pat).unwrap().params_mut().unwrap().insert("kind".into(), ParamValue::Choice(0));
        app.set_pattern_step(pat, -5.0, free);
        assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(0.0)));
    }

    #[test]
    fn adding_from_a_patterns_own_row_puts_the_shape_inside_it() {
        // Issue 67: the outliner's row menu asked `is_group`, so Add from a
        // pattern's row dropped the shape beside the pattern -- disagreeing with
        // the drag-and-drop rule and with the document-level Add, both of which
        // already put it in.
        let mut app = headless_app();
        app.run(Command::Pattern);
        let pat = app.primary().unwrap();
        let before = app.scene.node(pat).children.len();
        let root_before = app.scene.node(app.scene.root()).children.len();

        app.add_node_at(pat, Some("box"), GroupOp::Union);
        assert_eq!(app.scene.node(pat).children.len(), before + 1, "the shape did not go into the pattern");
        assert_eq!(app.scene.node(app.scene.root()).children.len(), root_before, "it landed beside the pattern");

        // And the document-level Add agrees, as it already did.
        assert_eq!(app.scene.insertion_point(Some(pat)).0, pat);
    }

    #[test]
    fn the_spacing_handle_lengthens_a_diagonal_run_without_straightening_it() {
        // The handle drives the whole run, not just its X component: a pattern
        // stepping diagonally must stay diagonal when it is dragged longer, and
        // the handle must stay at the last copy rather than off along X.
        use simple3d_core::primitive::ParamValue;
        let free = gizmo::Mods { free: true, ..Default::default() };
        let mut app = headless_app();
        app.run(Command::Pattern);
        let pat = app.primary().unwrap();
        {
            let params = app.scene.get_mut(pat).unwrap().params_mut().unwrap();
            params.insert("step_x".into(), ParamValue::Length(30.0));
            params.insert("step_y".into(), ParamValue::Length(40.0));
        }
        assert!((app.pattern_step_vector(pat).unwrap().length() - 50.0).abs() < 1e-9, "a 3-4-5 run");

        app.set_pattern_step(pat, 100.0, free);
        let step = app.pattern_step_vector(pat).unwrap();
        assert!((step.length() - 100.0).abs() < 1e-6, "the run was not doubled: {step:?}");
        assert!((step - Vec3::new(60.0, 80.0, 0.0)).length() < 1e-6, "the run was straightened onto X: {step:?}");
    }

    #[test]
    fn a_dragged_spacing_lands_on_the_documents_step() {
        // Every other viewport drag rounds to the move step; this one used to
        // write whatever the ray happened to hit, so laying a pattern out by eye
        // gave "7.0359 mm" under a 1 mm step.
        use simple3d_core::primitive::ParamValue;
        let mut app = headless_app();
        app.run(Command::Pattern);
        let pat = app.primary().unwrap();
        app.scene.settings.snap_step = 1.0;
        app.set_pattern_step(pat, 7.0359, gizmo::Mods::default());
        assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(7.0)));
        // Shift is the coarse step everywhere else, and here too.
        app.set_pattern_step(pat, 24.0, gizmo::Mods { coarse: true, ..Default::default() });
        assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(20.0)));
    }

    #[test]
    fn the_pattern_tool_spaces_its_copies_clear_of_the_shapes_it_wraps() {
        // Regression, issue 67: the stock 20 mm step is exactly the stock box's
        // width, so the tool's own output was three copies face to face -- and
        // the scene went red on the feature's very first use. The spacing now
        // comes from what is being wrapped, whatever size that is.
        for (w, d, h) in [(20.0, 20.0, 20.0), (4.0, 4.0, 4.0), (120.0, 60.0, 8.0)] {
            let mut app = headless_app();
            let root = app.scene.root();
            let id = app.scene.add_primitive("box", root, 0).unwrap();
            {
                let params = app.scene.get_mut(id).unwrap().params_mut().unwrap();
                params.insert("width".into(), simple3d_core::primitive::ParamValue::Length(w));
                params.insert("depth".into(), simple3d_core::primitive::ParamValue::Length(d));
                params.insert("height".into(), simple3d_core::primitive::ParamValue::Length(h));
            }
            app.select_only(id);
            app.run(Command::Pattern);
            let pat = app.primary().unwrap();
            let step = app.pattern_step_vector(pat).unwrap();
            assert!(step.x > w, "a {w}mm shape got a {}mm step, so its copies touch or overlap", step.x);
            // Scaled to the shape itself, not to what the pattern makes of it:
            // measuring the node *after* it became a pattern measured three
            // copies rather than one and inflated every distance threefold.
            assert!(
                (step.x - w * 1.5).abs() < 1e-6,
                "a {w}mm shape got a {}mm step; the spacing was taken from the repetition, not the shape",
                step.x
            );
            app.reevaluate_for_test();
            assert!(app.evaluated.errors.is_empty(), "{w}x{d}x{h}: {:?}", app.evaluated.errors);
        }
    }

    #[test]
    fn geometry_snapping_is_asked_for_by_the_mode_and_the_held_key() {
        // Issue 68's three modes, resolved through the same call the viewport
        // makes -- the key-down closure standing in for the live keyboard.
        let none = egui::Modifiers::NONE;
        let mut app = headless_app();
        app.settings.geometry_snap = SnapMode::Never;
        assert!(!app.geometry_snap_wanted(|_| true, none), "never should snap for no key");
        app.settings.geometry_snap = SnapMode::Always;
        assert!(app.geometry_snap_wanted(|_| false, none), "always should snap with no key held");

        app.settings.geometry_snap = SnapMode::WhileHeld;
        // The default hold is Ctrl on its own (issue 77): no key to hold, the
        // modifier state is the whole binding.
        assert_eq!(app.keymap.binding(Command::SnapToGeometry), Some(&Chord::modifiers(true, false, false)));
        assert!(!app.geometry_snap_wanted(|_| true, none), "held mode with no modifier down must not snap");
        assert!(app.geometry_snap_wanted(|_| false, egui::Modifiers::COMMAND), "Ctrl held must snap");
        assert!(
            !app.geometry_snap_wanted(|_| false, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT),
            "Ctrl+Shift is not the Ctrl binding"
        );

        // Rebound to a key, the key has to be down and the modifiers have to
        // match exactly. A hold on Ctrl+V must not fire on a bare V -- and the
        // unmodified binding must not fire on Ctrl+V, which is Paste.
        app.keymap.set(Command::SnapToGeometry, Chord::key("V"), true).unwrap();
        let key = crate::ui::key_from_name("V").unwrap();
        assert!(!app.geometry_snap_wanted(|_| false, none), "held mode with nothing down must not snap");
        assert!(app.geometry_snap_wanted(|k| k == key, none), "held mode with the snap key down must snap");
        assert!(
            !app.geometry_snap_wanted(|k| k == key, egui::Modifiers::COMMAND),
            "the bare binding fired with Ctrl down"
        );
        app.keymap.set(Command::SnapToGeometry, Chord::ctrl("V"), true).unwrap();
        assert!(!app.geometry_snap_wanted(|k| k == key, none), "a Ctrl+V hold fired on a bare V");
        assert!(app.geometry_snap_wanted(|k| k == key, egui::Modifiers::COMMAND));
    }

    #[test]
    fn a_move_drag_snaps_a_body_onto_another_bodys_vertex() {
        // Two boxes; drag the near one toward a corner of the far one with
        // geometry snapping asked for, and it lands exactly on that corner rather
        // than on the grid (issue 68).
        let mut app = app_in(temp_config_dir("snap-drag"));
        let root = app.scene.root();
        let a = app.scene.add_primitive("box", root, 0).unwrap();
        let b = app.scene.add_primitive("box", root, 1).unwrap();
        app.scene.get_mut(b).unwrap().position = Vec3::new(43.0, 0.0, 0.0);
        app.select_only(a);
        app.history.clear();
        app.reevaluate_for_test();

        // A real corner of B, taken from its evaluated world mesh.
        let (lo, hi) = app.evaluated.node_meshes[&b].bounds().unwrap();
        let corner = Vec3::new(lo.x, lo.y, hi.z);
        assert!(crate::snap::features_of(&app.evaluated.node_meshes[&b])
            .iter()
            .any(|f| f.kind == crate::snap::FeatureKind::Vertex && (f.point - corner).length() < 1e-6));

        app.settings.geometry_snap = SnapMode::Always;
        app.snap_requested = true;
        drag_gesture(&mut app, a, Handle::MoveAxis(0), corner, 3);

        // A *corner of A* caught the corner of B -- not A's origin. Snapping used
        // to move the origin onto the target, which left two boxes overlapping by
        // half rather than meeting at a corner.
        app.reevaluate_for_test();
        let landed = crate::snap::features_of(&app.evaluated.node_meshes[&a])
            .iter()
            .any(|f| f.kind == crate::snap::FeatureKind::Vertex && (f.point - corner).length() < 1e-6);
        assert!(
            landed,
            "no corner of the dragged box met the target corner {corner:?}; it sits at {:?}",
            app.scene.node(a).position
        );
        // 43 is not a multiple of the 1mm step, so a grid-only drag could not
        // have produced this.
        assert!((app.scene.node(a).position.x - 23.0).abs() < 1e-6, "{:?}", app.scene.node(a).position);

        // And an axis handle stayed on its axis. The whole point of grabbing the
        // X arrow is that Y and Z do not move; the snap used to overwrite all
        // three and slide the body off to (53, -10, 10).
        let after = app.scene.node(a).position;
        assert!(after.y.abs() < 1e-9 && after.z.abs() < 1e-9, "an X-axis drag moved in Y or Z: {after:?}");
    }

    #[test]
    fn a_snapped_plane_drag_stays_in_its_plane() {
        // The same constraint for the other move handle: a plane handle may move
        // in its two axes and must leave the third alone.
        let mut app = app_in(temp_config_dir("snap-plane"));
        let root = app.scene.root();
        let a = app.scene.add_primitive("box", root, 0).unwrap();
        let b = app.scene.add_primitive("box", root, 1).unwrap();
        app.scene.get_mut(b).unwrap().position = Vec3::new(43.0, 37.0, 25.0);
        app.select_only(a);
        app.history.clear();
        app.reevaluate_for_test();
        let (lo, hi) = app.evaluated.node_meshes[&b].bounds().unwrap();
        let corner = Vec3::new(lo.x, lo.y, hi.z);

        app.settings.geometry_snap = SnapMode::Always;
        app.snap_requested = true;
        // MovePlane(2): free in X and Y, pinned in Z.
        drag_gesture(&mut app, a, Handle::MovePlane(2), corner, 3);
        let after = app.scene.node(a).position;
        assert!(after.z.abs() < 1e-9, "a Z-plane drag moved in Z: {after:?}");
        assert!(after.x.abs() > 1e-6 && after.y.abs() > 1e-6, "the drag did not snap at all: {after:?}");
    }

    #[test]
    fn a_move_drag_without_snapping_asked_for_keeps_to_the_grid() {
        // The same drag with snapping off stays on the grid step and does not
        // jump onto the corner.
        let mut app = app_in(temp_config_dir("snap-off"));
        let root = app.scene.root();
        let a = app.scene.add_primitive("box", root, 0).unwrap();
        let b = app.scene.add_primitive("box", root, 1).unwrap();
        app.scene.get_mut(b).unwrap().position = Vec3::new(43.0, 0.0, 0.0);
        app.select_only(a);
        app.history.clear();
        app.reevaluate_for_test();
        let (lo, _) = app.evaluated.node_meshes[&b].bounds().unwrap();
        let corner = Vec3::new(lo.x, lo.y, lo.z);

        app.settings.geometry_snap = SnapMode::Never;
        app.snap_requested = false;
        drag_gesture(&mut app, a, Handle::MoveAxis(0), corner, 3);
        assert!(
            (app.scene.node(a).position - corner).length() > 1.0,
            "the drag snapped to the corner with snapping off: {:?}",
            app.scene.node(a).position
        );
    }
}
