//! The application: window layout, command dispatch, file handling and the
//! glue that keeps the outliner, property editor and viewport in step
//! (spec section 7).

use crate::gizmo::{self, Drag, Gizmo, Handle, Mode};
use crate::panel_viewport;
use crate::render::Renderable;
use crate::ui::{self, FieldBuffers};
use crate::view::{frame_bounds, CameraMove, ViewPreset};
use crate::worker::{EvalWorker, ExportJob, SplitJob};
use simple3d_core::clipboard::{self, Clip};
use simple3d_core::config::{self, AppSettings, DisplayMode, HandleFrame, Placement, Side, SnapMode};
use simple3d_core::eval::Evaluated;
use simple3d_core::keymap::{Chord, Command, Keymap};
use simple3d_core::library;
use simple3d_core::project;
use simple3d_core::scene::{Camera, Colour, GroupOp, NodeId, Scene};
use simple3d_core::undo::History;
use simple3d_core::unit::Unit;
use simple3d_export::Format;
use simple3d_geom::{Mesh, Vec3};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
    /// Building a custom pattern kind out of stages (issue 67).
    PatternKind,
    /// Quitting with unsaved changes.
    ConfirmQuit,
    /// Emptying a collection of every piece, which turns it into a union group
    /// and lets go of the object it was made from (issue 82).
    ConfirmExtractAll,
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

/// Everything one body offers a snap or a measurement: the notable points on it,
/// and the lines the principal planes leave across its surface -- the marks the
/// renderer draws on the solid, which are as catchable as any other line in the
/// picture.
#[derive(Default)]
pub struct BodySnaps {
    pub features: Vec<crate::snap::Feature>,
    pub marks: Vec<(Vec3, Vec3)>,
}

/// One body's snap targets, shared out of the cache without copying them.
type Snaps = std::rc::Rc<BodySnaps>;
/// What the cache holds per node: which mesh the targets were found on --
/// identified by the address of its `Arc`, which changes on re-evaluation and
/// nowhere else -- together with what the settings were showing, since the axis
/// crossings (issue 78) and the plane marks are part of the answer, and the
/// targets themselves.
type CachedSnaps = ((usize, u8), Snaps);

/// One of a pattern's lay-out grips (issue 67), placed in the world.
///
/// [`simple3d_core::pattern::grips`] works in the pattern's own frame, which is
/// where the copies are laid out; this is the same grip after the node's
/// transform, ready to be projected, hit-tested and dragged.
#[derive(Clone, Copy, Debug)]
pub struct PatternGrip {
    /// The grip's own name, which is what a drag holds on to across frames: an
    /// index would shift under the drag itself, since adding a copy can add a
    /// grip.
    pub label: &'static str,
    /// Where the grip sits, in world space.
    pub at: Vec3,
    /// The world line it slides along: a point on it, and a unit direction.
    pub from: Vec3,
    pub dir: Vec3,
    /// How many world millimetres one millimetre of the pattern's own frame
    /// covers along that line, so a scaled pattern still reads back the numbers
    /// its property editor shows.
    pub scale: f64,
    /// For a span grip: the world axis it turns about, the world direction zero
    /// degrees points in, and how far out the grip rides. `None` for a grip that
    /// slides along a line.
    pub turn: Option<(Vec3, Vec3, f64)>,
}

/// What a drag over the outliner is holding.
///
/// Rows already in the tree are moved by it; a shape from the palette is not in
/// the scene at all until the drop lands, so the two are told apart here rather
/// than by whether the load happens to be empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carried {
    /// The row that was grabbed. What travels with it is `dragged_nodes`: the
    /// whole selection when the grabbed row is part of it, that row alone
    /// otherwise.
    Rows(NodeId),
    /// A primitive type from the palette, dropped into the tree rather than
    /// added at the document's insertion point.
    Shape(&'static str),
}

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
    /// The settings as they are on disk, so a change to them can be noticed and
    /// written without every place that makes one having to remember to.
    persisted_settings: AppSettings,
    /// When they were last written, so a scrubbed number -- which changes on
    /// every frame of the drag -- costs one write a moment rather than one a
    /// frame.
    settings_written: Option<Instant>,
    pub keymap: Keymap,

    /// The current selection, in click order. The last entry is the primary one
    /// the property editor and the manipulator act on.
    pub selection: Vec<NodeId>,
    /// Which of a collection's pieces are ticked in its panel (issue 82).
    ///
    /// Not part of `selection`, and deliberately: a piece is reached *through*
    /// the collection, so selecting one the ordinary way would swap the panel
    /// away from the very list it was ticked in. These are marked in the
    /// viewport like a selection and are what Extract acts on, and they are
    /// dropped the moment the selection moves off the collection they belong to.
    pub(crate) piece_ticks: std::collections::BTreeSet<NodeId>,
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
    /// A line to hand the desktop's clipboard on the next frame.
    ///
    /// The nodes themselves stay in `clipboard`, which is the whole of what a
    /// paste reads. This is for the *other* half of the problem: `egui-winit`
    /// only reports a Ctrl+V at all when the system clipboard holds non-empty
    /// text, so an application that never writes to it can never be pasted
    /// into by keyboard. Writing a line saying what was copied also makes
    /// Ctrl+C do what a desktop expects of it.
    pub(crate) clipboard_text: Option<String>,
    /// The file dialog in flight, if there is one. See [`FilePrompt`].
    pub(crate) file_prompt: Option<FilePrompt>,

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

    /// The colour the picker has reached while it is open, waiting for the
    /// picker to be put away before it goes on the recent row (issue 85).
    ///
    /// A drag through the picker paints on every frame it moves, and every one
    /// of those shades is somewhere the pointer passed through rather than a
    /// colour anybody chose. One visit to the picker is one choice: the colour
    /// it ends on.
    picker_colour: Option<[u8; 3]>,

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
    pub outliner_drag: Option<Carried>,
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
    /// How far the section plane stood from the point the pointer took hold of
    /// it, for as long as its grip is being dragged (issue 71). `None` when the
    /// plane is not being moved.
    pub section_grab: Option<f64>,
    /// Which of the section plane's five grips the pointer is on this frame, so
    /// the frame can say that it can be taken hold of before it is -- and so
    /// the arrows that say which way it travels are drawn on that grip alone
    /// (issue 72). `None` when the pointer is on none of them.
    pub section_hover: Option<usize>,
    /// Every feature of the body the current drag is carrying, as offsets from
    /// its origin; gathered on `Begin`. See `App::drag_feature_offsets`.
    snap_sources: Vec<Vec3>,
    /// Each body's snap features, kept between frames and keyed on the identity
    /// of the mesh they were found on. See `App::snaps_of`.
    snap_features: std::cell::RefCell<std::collections::HashMap<NodeId, CachedSnaps>>,
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
    /// The split tool's window while it is open, and the cutting it started
    /// (issue 82). The tool holds the shape it is about to cut; the job holds
    /// the thread cutting it, and nothing in the document changes until it
    /// lands -- see [`crate::split_tool`].
    pub split_tool: Option<crate::split_tool::SplitTool>,
    pub split_job: Option<SplitJob>,
    /// The collection the "extract every piece" question is being asked about
    /// (issue 82). Set only while [`Modal::ConfirmExtractAll`] is up.
    pub(crate) confirm_extract: Option<NodeId>,
    /// Where each in-place popup sits and whether it is rolled up (issue 82).
    /// Keyed by the popup's own name, so a tool re-opened comes back where it
    /// was last dragged to rather than back in the middle.
    pub(crate) popups: std::collections::HashMap<&'static str, crate::popup::Placement>,
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

    /// The pattern the custom-kind creation tool is building a rule for
    /// (issue 67). The tool edits that node directly, so what it says and what
    /// the viewport shows cannot disagree.
    pub pattern_tool: Option<NodeId>,
    /// The name the rule would be saved under, and the one the last applied kind
    /// came by.
    pub pattern_tool_name: String,
    /// The saved kinds, read when the tool opens rather than every frame -- the
    /// shelf is a directory and the dialog draws sixty times a second.
    pub pattern_kinds: Vec<simple3d_core::pattern_library::Entry>,
    /// The camera the tool's preview looks through: its own, so turning the
    /// pattern round to see what a rule made does not move the viewport behind
    /// the window, and so it can be framed on the pattern rather than on
    /// whatever the scene happens to be showing.
    pub pattern_preview_camera: Camera,
    /// Whether the preview's camera has been framed on the pattern yet. Framing
    /// needs the aspect the picture is actually drawn at, so it cannot happen
    /// where it is asked for -- opening the tool and the Frame button both clear
    /// this, and `pattern_tool::preview` does the framing on the next frame.
    ///
    /// Nothing else clears it. Once the picture is framed, where it is looking
    /// from is the user's, and typing a number must not take it off them.
    pub(crate) pattern_preview_framed: bool,
    /// The last image the preview drew, and what it was drawn from. The preview
    /// is a full render of the scene, so it is kept between frames exactly the
    /// way the viewport's own image is.
    pub(crate) pattern_preview_texture: Option<egui::TextureHandle>,
    pub(crate) pattern_preview_key: u64,

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
            persisted_settings: settings.clone(),
            settings_written: None,
            settings,
            picker_colour: None,
            keymap,
            selection: Vec::new(),
            piece_ticks: std::collections::BTreeSet::new(),
            selection_anchor: None,
            outliner_last_click: None,
            clipboard: None,
            clipboard_text: None,
            file_prompt: None,
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
            section_grab: None,
            section_hover: None,
            snap_features: std::cell::RefCell::new(std::collections::HashMap::new()),
            pending_delete: None,
            camera_move: None,
            cube_spin: None,
            dock_drag: crate::dock::DockDrag::default(),
            dock_headers: Vec::new(),
            dock_rects: Vec::new(),
            export_job: None,
            split_tool: None,
            split_job: None,
            confirm_extract: None,
            popups: std::collections::HashMap::new(),
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
            pattern_tool: None,
            pattern_tool_name: String::new(),
            pattern_kinds: Vec::new(),
            pattern_preview_camera: Camera::default(),
            pattern_preview_framed: false,
            pattern_preview_texture: None,
            pattern_preview_key: u64::MAX,
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
        // And the saved pattern kinds, for the same reason: the property panel
        // offers them on any custom pattern, which is a place the user reaches
        // without ever opening the tool that keeps the shelf up to date.
        app.refresh_pattern_kinds();
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
        // A half-typed field belongs to the node it was opened on, and so does
        // a tick in a collection's list of pieces (issue 82).
        self.fields.clear();
        self.piece_ticks.clear();
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

    /// Put a file dialog up and say what to do with the path it answers with.
    ///
    /// `what` names the wait for the footer, and is what a cancelled dialog
    /// reports having cancelled -- silence there is the other half of the bug
    /// this fixes: a portal that answered with nothing left Ctrl+S on an unsaved
    /// document with no dialog *and* no message, which is indistinguishable from
    /// a save the user cancelled on purpose.
    ///
    /// One at a time. A second dialog while one is up would be two windows
    /// asking the same question, and only one answer could be acted on.
    pub(crate) fn ask_for_file(
        &mut self,
        what: &'static str,
        dialog: rfd::FileDialog,
        saving: bool,
        then: impl FnOnce(&mut App, std::path::PathBuf) + 'static,
    ) {
        if let Some(waiting) = &self.file_prompt {
            self.status = Status::Warning(format!("Still choosing a file for {}", waiting.what().to_lowercase()));
            return;
        }
        self.file_prompt = Some(FilePrompt {
            what,
            answer: ask_for_path(dialog, saving),
            then: Box::new(then),
            started: std::time::Instant::now(),
        });
    }

    /// Act on a file dialog that has answered. Called once a frame, beside the
    /// export job's own poll.
    pub(crate) fn poll_file_prompt(&mut self) {
        let Some(prompt) = &self.file_prompt else { return };
        let answer = match prompt.answer.try_recv() {
            Ok(answer) => answer,
            // The dialog thread went away without answering, which is the same
            // outcome as a cancel and is reported as one.
            Err(std::sync::mpsc::TryRecvError::Disconnected) => None,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
        };
        let prompt = self.file_prompt.take().expect("checked just above");
        match answer {
            Some(path) => (prompt.then)(self, path),
            None => self.status = Status::Info(format!("{} cancelled", prompt.what)),
        }
    }

    /// Stop waiting on the dialog in flight.
    ///
    /// The thread stays parked until the portal answers, if it ever does, and
    /// its answer is dropped: what it is holding is a channel nobody is
    /// listening to any more.
    pub(crate) fn stop_waiting_for_file(&mut self) {
        if let Some(prompt) = self.file_prompt.take() {
            self.status = Status::Warning(format!("Stopped waiting for the file dialog ({})", prompt.what));
        }
    }

    pub fn open_dialog(&mut self) {
        let mut dialog = rfd::FileDialog::new().add_filter("Simple 3D project", &[PROJECT_EXTENSION]);
        if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        self.ask_for_file("Open", dialog, false, |app, path| app.open_path(&path));
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
        self.ask_for_file("Save as", dialog, true, |app, mut path| {
            if path.extension().is_none() {
                path.set_extension(PROJECT_EXTENSION);
            }
            app.save_to(&path);
        });
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

    /// Frame `lo`..`hi`, honouring the view-centre lock: a pinned centre keeps
    /// its place and only the zoom changes.
    ///
    /// Framing is the one command whose whole job is to move the view centre, so
    /// a lock could as easily have disabled it. It fits the zoom instead, and
    /// says which half it did: a Frame button that does nothing at all reads as
    /// a broken button, and half of framing is still worth having.
    fn frame_onto(&mut self, lo: Vec3, hi: Vec3) {
        let aspect = self.aspect();
        let pinned = self.settings.lock_view_centre.then_some(self.scene.camera.target);
        frame_bounds(&mut self.scene.camera, lo, hi, aspect);
        if let Some(target) = pinned {
            self.scene.camera.target = target;
            self.status = Status::Info("The view centre is locked, so framing changed the zoom only".into());
        }
    }

    pub fn frame_all(&mut self) {
        match self.evaluated.mesh.bounds().or_else(|| self.selection_bounds()) {
            Some((lo, hi)) => self.frame_onto(lo, hi),
            None => {
                if !self.settings.lock_view_centre {
                    self.scene.camera.target = Vec3::ZERO;
                }
                self.scene.camera.distance = 160.0;
            }
        }
    }

    pub fn frame_selection(&mut self) {
        match self.selection_bounds() {
            Some((lo, hi)) => self.frame_onto(lo, hi),
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
            ConvertToMesh => self.convert_selection_to_mesh(),
            SplitIntoPieces => self.open_split_tool(),
            Rejoin => self.rejoin_selection(),
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
            ToggleSection => self.toggle_section(),
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

    /// Switch the section plane on or off (issue 71).
    ///
    /// Switching it on puts it in the middle of the model along its axis unless
    /// it has already been placed somewhere. A plane left at zero cuts nothing
    /// at all for a part that stands beside the origin, and a section that
    /// appears to do nothing reads as a broken one rather than as a plane that
    /// needs sliding.
    fn toggle_section(&mut self) {
        let on = !self.scene.settings.section.enabled;
        self.scene.settings.section.enabled = on;
        if on && self.scene.settings.section.offset == 0.0 {
            let axis = self.scene.settings.section.axis();
            self.scene.settings.section.offset = crate::section_tool::middle_of(self.evaluated.mesh.bounds(), axis);
        }
        self.status = Status::Info(match on {
            true => crate::section_tool::readout(self),
            false => "Section off".to_string(),
        });
    }

    /// Slide the section plane to `offset`, in millimetres along its own axis.
    /// Nothing about the model changes, so this is not an edit and there is no
    /// undo step for it -- see [`crate::section_tool`].
    pub fn set_section_offset(&mut self, offset: f64) {
        self.scene.settings.section.offset = offset;
        self.status = Status::Info(crate::section_tool::readout(self));
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
    /// one is within reach on screen, otherwise a point along the nearest line --
    /// an edge, a plane mark, an axis -- otherwise the point on the surface under
    /// the pointer, otherwise the ground plane. `None` only when the pointer is
    /// on empty sky, where there is nothing to measure to.
    ///
    /// The order is what makes placement predictable (issue 78): the exact
    /// points -- corners, midpoints, axis crossings -- win whenever one is in
    /// reach, and a line catches the pointer only where none of them does, so
    /// aiming at a corner never lands part-way along the edge beside it.
    pub fn measure_point_at(&self, view: &crate::view::View, cursor: egui::Pos2) -> Option<MeasurePoint> {
        // Only what is on screen can be caught. A feature the model covers is not
        // being pointed at: the corner is around the back of the solid, or the
        // edge is its far bottom one, and neither is drawn -- but both project
        // into the middle of the face in front of them, where a pointer aimed at
        // that face snapped to them out of nowhere. So the same question the axes
        // have always been asked, "does the picture show this?", is asked of
        // every feature.
        let shown = |feature: &crate::snap::Feature, _: &Mesh| self.shows(view, feature.point);
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
    ///
    /// Only the features the body actually turns towards the camera: a corner
    /// round the back of a solid is not one anybody is aiming at, however near
    /// the pointer its projection lands, and a drag that jumped onto one moved
    /// the body for no reason the picture gave. The measure tool asks the same
    /// question of the whole scene; a drag asks it of the target's own body
    /// only, because the scene it would have to ask describes where the dragged
    /// body *was* -- `Evaluated` lags a drag by a frame -- and the body being
    /// carried would spend the whole gesture covering the very thing it is being
    /// aimed at.
    pub fn nearest_feature_excluding(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
        exclude: &[NodeId],
    ) -> Option<(crate::snap::Feature, f32)> {
        self.nearest_feature_where(view, cursor, exclude, |feature, mesh| self.faces_the_camera(view, feature, mesh))
    }

    /// Whether a feature is on the side of its own body that the camera can see.
    ///
    /// Wireframe fills nothing and hides nothing, so there every feature is as
    /// catchable as the line that shows it.
    fn faces_the_camera(&self, view: &crate::view::View, feature: &crate::snap::Feature, mesh: &Mesh) -> bool {
        if self.settings.display_mode == DisplayMode::Wireframe {
            return true;
        }
        let Some((screen, _)) = view.project(feature.point) else { return false };
        let (origin, dir) = view.ray(screen);
        let reach = (feature.point - origin).dot(dir);
        match crate::pick::ray_mesh(mesh, origin, dir) {
            // A point on the surface is its own hit, so the comparison leaves
            // room for one -- the same hundredth of a millimetre `in_clear_view`
            // allows, far below anything a placement cares about.
            Some(hit) => hit >= reach - 1e-2,
            None => true,
        }
    }

    /// The same, for a caller that will not take every feature -- the measure
    /// tool, which takes only the ones the picture shows.
    ///
    /// The candidates within reach are ranked by screen distance and offered
    /// outward from the cursor, so the first one accepted is the nearest
    /// acceptable one. Ranking rather than filtering is what makes that cheap:
    /// "does the frame show this?" costs a ray cast through the scene, and asking
    /// it of the feature under the pointer is one cast where asking it of every
    /// feature of every body is hundreds.
    pub fn nearest_feature_where(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
        exclude: &[NodeId],
        accept: impl Fn(&crate::snap::Feature, &Mesh) -> bool,
    ) -> Option<(crate::snap::Feature, f32)> {
        let project = |p: Vec3| view.project(p).map(|(screen, _)| screen);
        let mut near: Vec<(crate::snap::Feature, f32, NodeId)> = Vec::new();
        for (&id, mesh) in &self.evaluated.node_meshes {
            if !self.scene.is_shown(id) || exclude.contains(&id) {
                continue;
            }
            let snaps = self.snaps_of(id, mesh);
            let found = crate::snap::near_on_screen(&snaps.features, project, cursor, crate::snap::CATCH_PIXELS);
            near.extend(found.into_iter().map(|(feature, distance)| (*feature, distance, id)));
        }
        near.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        near.into_iter()
            .find(|(feature, _, id)| self.evaluated.node_meshes.get(id).is_some_and(|mesh| accept(feature, mesh)))
            .map(|(feature, distance, _)| (feature, distance))
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
    fn snaps_of(&self, id: NodeId, mesh: &std::sync::Arc<simple3d_geom::Mesh>) -> Snaps {
        let axes = self.scene.settings.axes_visible;
        let marked = self.plane_marks_drawn();
        let mask = (axes[0] as u8) | (axes[1] as u8) << 1 | (axes[2] as u8) << 2 | (marked as u8) << 3;
        let key = (std::sync::Arc::as_ptr(mesh) as usize, mask);
        if let Some((cached_key, snaps)) = self.snap_features.borrow().get(&id) {
            if *cached_key == key {
                return snaps.clone();
            }
        }
        let mut features = crate::snap::features_of(mesh);
        // Where the world axes run through the body, offered as corners and as
        // the edge between them (issue 78).
        features.extend(crate::snap::axis_features(mesh, axes));
        // And the lines the principal planes leave across it, which are drawn on
        // the surface and so can be caught along their length.
        let marks = if marked { crate::snap::plane_mark_lines(mesh, axes) } else { Vec::new() };
        let snaps = std::rc::Rc::new(BodySnaps { features, marks });
        self.snap_features.borrow_mut().insert(id, (key, snaps.clone()));
        snaps
    }

    /// Whether the plane marks are on screen: the switch for them, and a display
    /// mode with a surface for them to sit on. They are not drawn in wireframe,
    /// so there is nothing there to catch either.
    fn plane_marks_drawn(&self) -> bool {
        self.scene.settings.plane_marks && self.settings.display_mode != DisplayMode::Wireframe
    }

    /// Whether the picture shows this point, or material stands in front of it.
    ///
    /// What the measure tool takes is what the frame shows -- a corner on the far
    /// side of a solid is not a corner anybody is pointing at, however near the
    /// pointer its projection lands. Wireframe fills nothing, so nothing there
    /// covers anything: the far side of a body is drawn exactly like the near
    /// side, which is the point of that mode, and every feature of it is as
    /// catchable as the line that shows it.
    fn shows(&self, view: &crate::view::View, at: Vec3) -> bool {
        self.settings.display_mode == DisplayMode::Wireframe || self.in_clear_view(view, at)
    }

    /// The point on the nearest *line* -- a body's edge, the mark a principal
    /// plane leaves across it, or a world axis -- for a pointer that is near one
    /// but not near any of the notable points on it (issue 78).
    ///
    /// Edges come from the same feature list, which carries each edge's two ends
    /// beside its midpoint, so they need no second pass over the geometry. The
    /// axes are lines in their own right: a point on one is as real a place to
    /// measure from as a corner is, and offering only the handful of places
    /// where an axis meets something left the rest of it -- most of it -- with
    /// nothing to catch. The plane marks are the same argument on the surface:
    /// where the axis itself runs through the material and is not drawn, the
    /// mark is what the picture puts there, and it is what a pointer over the
    /// body is aiming at.
    pub fn nearest_line_point(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
    ) -> Option<(Vec3, crate::snap::FeatureKind, f32)> {
        let project = |p: Vec3| view.project(p).map(|(screen, _)| screen);
        let mut near: Vec<(Vec3, crate::snap::FeatureKind, f32)> = Vec::new();
        let mut consider = |a: Vec3, b: Vec3, kind: crate::snap::FeatureKind| {
            if let Some((at, distance)) = crate::snap::nearest_on_edge(a, b, project, cursor, crate::snap::CATCH_PIXELS)
            {
                near.push((at, kind, distance));
            }
        };
        for (&id, mesh) in &self.evaluated.node_meshes {
            if !self.scene.is_shown(id) {
                continue;
            }
            let snaps = self.snaps_of(id, mesh);
            for feature in &snaps.features {
                if let Some((a, b)) = feature.span {
                    consider(a, b, crate::snap::FeatureKind::Edge);
                }
            }
            // The marks the principal planes leave on the surface. They are
            // lines on the body, drawn in the colour of the axis whose plane
            // made them, and a measurement along one -- how far along this face
            // is the plane through zero -- is exactly what they are read for.
            for &(a, b) in &snaps.marks {
                consider(a, b, crate::snap::FeatureKind::PlaneMark);
            }
        }
        // As far as the axes are actually drawn, so nothing is caught out where
        // there is no line to see.
        let reach = crate::render::grid_radius(view);
        for (a, b) in crate::snap::axis_lines(self.scene.settings.axes_visible, reach) {
            consider(a, b, crate::snap::FeatureKind::Axis);
        }
        // Nearest first, and the nearest one the picture actually shows wins.
        //
        // Every one of these is a line that stops where the drawing stops. An
        // axis is cut out of the material it runs through and covered by whatever
        // is in front of it; an edge on the far side of a solid, and a plane mark
        // on the back of one, are behind that solid however near the pointer
        // their projection lands. Catching them anyway is what made the tool jump
        // to lines inside the object, which is the one thing no line on screen
        // does.
        near.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        near.into_iter().find(|&(at, kind, _)| match kind {
            // The axis keeps its own question in every display mode: wireframe
            // fills nothing and hides nothing, but the stretch inside a body is
            // still cut out of the line there.
            crate::snap::FeatureKind::Axis => self.in_clear_view(view, at),
            _ => self.shows(view, at),
        })
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
            offsets.extend(self.snaps_of(n, mesh).features.iter().map(|f| f.point - world_origin));
        }
        offsets
    }

    /// Snap a resize so the face being pulled lands on the nearest feature of
    /// another body under the pointer (issue 68). Returns the world point it
    /// caught, or `None` when nothing was in reach, in which case the grid
    /// resize stands.
    ///
    /// A resize is a drag, and it snapped only to the grid step. Pulling a plate
    /// out until it meets the block beside it is the same gesture as sliding it
    /// there, and it wants the same answer. Only a *face* handle: a corner moves
    /// three faces at once, and there is no one face to put on a point.
    fn apply_resize_snap(
        &mut self,
        id: NodeId,
        view: &crate::view::View,
        cursor: egui::Pos2,
        mods: gizmo::Mods,
    ) -> Option<(Vec3, f64)> {
        let exclude = self.drag_subtree(id);
        let (target, _) = self.nearest_feature_excluding(view, cursor, &exclude)?;
        // Taken out and put back so the drag can write the scene the app owns.
        let mut drag = self.drag.take()?;
        let applied = drag.resize_face_to(&mut self.scene, target.point, mods.symmetric);
        self.drag = Some(drag);
        applied.map(|extent| (target.point, extent))
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
        // Where every feature the carried body offers currently is.
        let sources: Vec<Vec3> =
            std::iter::once(Vec3::ZERO).chain(self.snap_sources.iter().copied()).map(|o| world_origin + o).collect();
        // Brought alongside first, aimed at second: the drag catches on whatever
        // the body has come near, and only when it has come near nothing does the
        // feature the pointer is over get its say.
        let (correction, target) = self
            .snap_alongside(view, &sources, &exclude, &constrain)
            .or_else(|| self.snap_at_pointer(view, cursor, &sources, &exclude, &constrain))?;
        let new_origin = world_origin + correction;
        let new_position = frame.inverse().point(new_origin);
        if let Some(node) = self.scene.get_mut(id) {
            node.position = new_position;
        }
        Some(target)
    }

    /// The snap a drag catches by bringing the body alongside another (issue 68):
    /// the least the body can be moved, within what the handle allows, to put one
    /// of its own features onto a feature of a body it is not carrying.
    ///
    /// This is what makes snapping reachable at all. Taking the target from
    /// whatever the *pointer* is over cannot place two bodies against each other:
    /// the handle is grabbed some seventy pixels out from the body, so by the time
    /// the pointer reaches the corner to meet, the body has already been dragged
    /// on top of it -- two 20 mm boxes could be snapped into the same 20 mm of
    /// space and into nothing else. What a person is actually judging as they drag
    /// is whether the thing they are carrying has come alongside the thing they
    /// want it against, which is the question asked here.
    ///
    /// Nearness is judged on screen, where the judgement is being made, so a snap
    /// takes the same aim whatever the zoom. Candidates are bucketed by screen
    /// cell rather than compared all against all: a body of any size offers a
    /// feature per corner and per edge, and a drag asks this on every frame.
    fn snap_alongside(
        &self,
        view: &crate::view::View,
        sources: &[Vec3],
        exclude: &[NodeId],
        constrain: &impl Fn(Vec3) -> Vec3,
    ) -> Option<(Vec3, Vec3)> {
        let cell = crate::snap::DRAG_CATCH_PIXELS;
        let mut buckets: std::collections::HashMap<(i32, i32), Vec<usize>> = std::collections::HashMap::new();
        let mut screens: Vec<Option<egui::Pos2>> = Vec::with_capacity(sources.len());
        for (index, point) in sources.iter().enumerate() {
            let screen = view.project(*point).map(|(at, _)| at);
            if let Some(at) = screen {
                buckets.entry(((at.x / cell).floor() as i32, (at.y / cell).floor() as i32)).or_default().push(index);
            }
            screens.push(screen);
        }
        // Every pair near enough on screen to be meant, cheapest move first. The
        // whole list rather than the single best, because the winner still has to
        // be a feature the picture shows -- and that question costs a ray cast, so
        // it is asked of the pairs in the order they would be taken rather than of
        // all of them.
        let mut pairs: Vec<(f64, Vec3, crate::snap::Feature, NodeId)> = Vec::new();
        for (&node, mesh) in &self.evaluated.node_meshes {
            if !self.scene.is_shown(node) || exclude.contains(&node) {
                continue;
            }
            for feature in self.snaps_of(node, mesh).features.iter() {
                let Some((at, _)) = view.project(feature.point) else { continue };
                let (cx, cy) = ((at.x / cell).floor() as i32, (at.y / cell).floor() as i32);
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        let Some(near) = buckets.get(&(cx + dx, cy + dy)) else { continue };
                        for &index in near {
                            let Some(source_at) = screens[index] else { continue };
                            if (source_at - at).length() > cell {
                                continue;
                            }
                            let correction = constrain(feature.point - sources[index]);
                            pairs.push((correction.length(), correction, *feature, node));
                        }
                    }
                }
            }
        }
        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        pairs
            .into_iter()
            .find(|(_, _, feature, node)| {
                self.evaluated.node_meshes.get(node).is_some_and(|mesh| self.faces_the_camera(view, feature, mesh))
            })
            .map(|(_, correction, feature, _)| (correction, feature.point))
    }

    /// The snap a drag catches by being aimed: the feature under the pointer, and
    /// whichever of the carried body's own features the handle lets reach it.
    ///
    /// The fallback to [`App::snap_alongside`], and the one that can cross a gap:
    /// nothing has to have come near anything, so this is how a body is thrown
    /// onto a corner some way off. An empty group offers no features of its own,
    /// and then it is the origin that lands on the target, which still beats
    /// refusing to snap at all.
    fn snap_at_pointer(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
        sources: &[Vec3],
        exclude: &[NodeId],
        constrain: &impl Fn(Vec3) -> Vec3,
    ) -> Option<(Vec3, Vec3)> {
        let (target, _) = self.nearest_feature_excluding(view, cursor, exclude)?;
        let mut best: Option<(Vec3, f64, f64)> = None;
        for source in sources {
            let correction = constrain(target.point - *source);
            // How far the feature still misses the target after the constrained
            // move: exactly zero when it can reach, and the shortest achievable
            // gap when the handle will not let it all the way there.
            let miss = (*source + correction - target.point).length();
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
        best.map(|(correction, _, _)| (correction, target.point))
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
        let what = match clip.nodes.as_slice() {
            [only] => only.name.clone(),
            nodes => format!("{} nodes", nodes.len()),
        };
        self.clipboard_text = Some(format!("Simple 3D: {what}"));
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
        // A pattern pasted into can now be measured (issue 67).
        self.size_fresh_patterns();
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
        self.size_fresh_patterns();
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
        let holds_children = |id: &NodeId| {
            (self.scene.node(*id).is_group() || self.scene.node(*id).is_split())
                && !self.scene.node(*id).children.is_empty()
        };
        if targets.iter().any(holds_children) {
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

    /// Every lay-out grip the pattern `id` offers, in world space (issue 67).
    ///
    /// Empty for anything that is not a pattern, and for a mirror, which has no
    /// distance and no count to lay out. The pattern's own frame is what places
    /// them -- not the handle frame, which the user may have switched to world:
    /// a grip marks a copy the pattern actually makes, and those turn with the
    /// node.
    pub fn pattern_grips(&self, id: NodeId) -> Vec<PatternGrip> {
        let Some(node) = self.scene.get(id) else { return Vec::new() };
        if !node.is_pattern() {
            return Vec::new();
        }
        let Some(params) = node.params() else { return Vec::new() };
        let Some(gizmo) = self.gizmo_for(id) else { return Vec::new() };
        let own = gizmo.own;
        simple3d_core::pattern::grips(params)
            .into_iter()
            .map(|grip| {
                let along = own.vector(grip.dir);
                let scale = along.length().max(1e-9);
                let turn = match grip.drive {
                    simple3d_core::pattern::Drive::Angle { axis, .. } => {
                        let radial = simple3d_core::pattern::radial_axis(axis);
                        let mut zero = Vec3::ZERO;
                        match radial {
                            0 => zero.x = 1.0,
                            1 => zero.y = 1.0,
                            _ => zero.z = 1.0,
                        }
                        Some((along * (1.0 / scale), own.vector(zero).normalized(), grip.radius))
                    }
                    _ => None,
                };
                PatternGrip {
                    label: grip.label,
                    at: own.point(grip.at),
                    from: own.point(grip.from),
                    dir: along * (1.0 / scale),
                    scale,
                    turn,
                }
            })
            .collect()
    }

    /// What the pointer is asking a grip for: a distance along its line, or --
    /// for a span grip -- the angle it has been carried round to.
    ///
    /// Both come back in the pattern's own units, which is what its parameters
    /// are written in, so a scaled pattern is read back at its own numbers
    /// rather than the world's.
    pub fn pattern_grip_value(&self, grip: &PatternGrip, view: &crate::view::View, cursor: egui::Pos2) -> Option<f64> {
        match grip.turn {
            Some((axis, zero, _)) => {
                let at = view.ray_plane(cursor, grip.from, axis)?;
                let radial = at - grip.from;
                let tangent = axis.cross(zero);
                let degrees = radial.dot(tangent).atan2(radial.dot(zero)).to_degrees();
                // Round the back of the circle a span reads as the whole turn
                // rather than as nothing: dragging past 359 degrees means "all
                // the way", which is the number a full ring wants.
                Some(if degrees < 0.0 { degrees + 360.0 } else { degrees })
            }
            None => Some(view.ray_axis(cursor, grip.from, grip.dir)? / grip.scale),
        }
    }

    /// Write what a grip was dragged to, coalesced into one undo step so the
    /// whole drag is a single edit.
    ///
    /// The value is rounded the way every other viewport drag is -- to the
    /// document's move step for a distance, to the rotation snap for a span --
    /// because a handle that alone produced "7.0359 mm" under a 1 mm step was
    /// the odd one out. A count rounds to whole copies on its own.
    pub fn set_pattern_grip(&mut self, id: NodeId, label: &str, value: f64, mods: gizmo::Mods) {
        let Some(params) = self.scene.get(id).and_then(|n| n.params()) else { return };
        let Some(grip) = simple3d_core::pattern::grip(params, label) else { return };
        let wanted = match grip.drive {
            simple3d_core::pattern::Drive::Length { .. } => mods.snap(value, self.move_snap()),
            simple3d_core::pattern::Drive::Angle { .. } => mods.snap(value, self.settings.rotate_snap_deg),
            simple3d_core::pattern::Drive::Count { .. } => value,
        };
        self.edit("Pattern layout", Some(&format!("pattern-grip:{id}:{label}")));
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            simple3d_core::pattern::apply_grip(params, &grip, wanted);
        }
        self.touch();
    }

    /// Size a fresh pattern's spacing to what it holds, the moment it first
    /// holds something (issue 67).
    ///
    /// The pattern *tool* measures the shapes it wraps, so making a pattern of a
    /// 20 mm box gives a 30 mm step. A pattern made with nothing selected has
    /// nothing to measure yet and keeps the stock numbers, so a 20 mm shape
    /// dropped into it afterwards was repeated at exactly its own width and the
    /// copies came out as one welded bar instead of three boxes standing clear.
    ///
    /// Only while the numbers are still untouched -- exactly the defaults a bare
    /// pattern is born with -- so nothing typed into the property editor, and
    /// nothing laid out with a grip, is ever overwritten under the user.
    ///
    /// The children are measured, not the pattern: asking the pattern measures
    /// the repetition rather than the thing being repeated, and the spacing
    /// derived from that comes out a whole run too large.
    pub(crate) fn size_fresh_patterns(&mut self) {
        let defaults = simple3d_core::pattern::default_params();
        let fresh: Vec<NodeId> = self
            .scene
            .depth_first()
            .into_iter()
            .filter(|&id| {
                let node = self.scene.node(id);
                node.is_pattern() && !node.children.is_empty() && node.params() == Some(&defaults)
            })
            .collect();
        for id in fresh {
            let mut bounds: Option<(Vec3, Vec3)> = None;
            for child in self.scene.node(id).children.clone() {
                let Some((lo, hi)) = simple3d_core::eval::subtree_bounds(&self.scene, child) else { continue };
                bounds = Some(match bounds {
                    Some((l, h)) => (l.min(lo), h.max(hi)),
                    None => (lo, hi),
                });
            }
            let Some((lo, hi)) = bounds else { continue };
            if let Some(node) = self.scene.get_mut(id) {
                node.body =
                    simple3d_core::scene::Body::Pattern { params: simple3d_core::pattern::params_for_size(hi - lo) };
            }
        }
    }

    /// The pattern creation tool (issue 67): wrap the selection in a pattern
    /// node that repeats it, or -- with nothing selected -- drop an empty pattern
    /// at the insertion point for shapes to be put under. Either way the pattern
    /// is selected, so the property editor is right there to lay it out.
    pub(crate) fn make_pattern(&mut self) {
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
        self.apply_paint(targets, colour, coalesce);
        if let Some(Colour(rgb)) = colour {
            // Only a colour the user picked out for themselves. The eight
            // presets are already on the palette above this row; repeating one
            // of them here spends the recent list on colours that were never
            // hard to find (issue 35).
            if !is_preset(rgb) {
                self.settings.remember_colour(rgb);
            }
        }
    }

    /// Paint from the colour picker, where the colour is still being chosen.
    ///
    /// Every frame of a drag through the picker comes through here, so nothing
    /// is remembered yet: what the picker has reached is held until it is put
    /// away, and [`App::picker_closed`] puts that one colour on the recent row
    /// (issue 85). Before this, a drag deposited a swatch every time it paused
    /// for longer than the undo history's coalescing window -- which is how a
    /// single wander through the dark corner of the picker left eight shades of
    /// black on a row meant to hold eight colours.
    pub(crate) fn paint_from_picker(&mut self, targets: &[NodeId], rgb: [u8; 3]) {
        self.apply_paint(targets, Some(Colour(rgb)), Some("colour"));
        self.picker_colour = Some(rgb);
    }

    /// The picker is closed: whatever it ended on is the colour that was
    /// chosen, and the only one of the drag worth offering again.
    ///
    /// Called on every frame the picker is not open, so it has to cost nothing
    /// when there is nothing waiting.
    pub(crate) fn picker_closed(&mut self) {
        let Some(rgb) = self.picker_colour.take() else { return };
        if !is_preset(rgb) {
            self.settings.remember_colour(rgb);
        }
    }

    /// The paint itself, without the question of what to remember.
    fn apply_paint(&mut self, targets: &[NodeId], colour: Option<Colour>, coalesce: Option<&str>) {
        self.edit(if colour.is_some() { "Colour" } else { "Clear colour" }, coalesce);
        for target in targets {
            self.scene.paint_subtree(*target, colour);
        }
    }

    /// The recent colours worth offering: the ones that are not already a
    /// preset, and not a shade of one further up the row.
    ///
    /// Filtered on the way out as well as on the way in, so a list saved by an
    /// earlier version stops showing them too -- which matters here, because
    /// the version that filled a row with eight shades of black wrote them to
    /// the settings file and they would otherwise sit there forever (issue 85).
    pub(crate) fn custom_recent_colours(&self) -> Vec<[u8; 3]> {
        let mut kept: Vec<[u8; 3]> = Vec::new();
        for rgb in self.settings.recent_colours.iter().copied() {
            if is_preset(rgb) || kept.iter().any(|shown| simple3d_core::config::indistinguishable(*shown, rgb)) {
                continue;
            }
            kept.push(rgb);
        }
        kept
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
    /// Where a node added *from an outliner row* lands: inside the row when it
    /// can hold children, beside it otherwise (issue 44).
    ///
    /// A pattern holds children exactly as a group does (issue 67), so Add from
    /// a pattern's own row goes *into* it. Asking `is_group` here put the shape
    /// beside the pattern instead, which made the row menu disagree with both
    /// the drag-and-drop rule and the document-level Add, and both of those
    /// already say "into".
    fn insertion_from_row(&self, at: NodeId) -> (NodeId, usize) {
        let end_of_root = (self.scene.root(), self.scene.node(self.scene.root()).children.len());
        match self.scene.get(at) {
            // Not into a collection: the tree does not open one, so a shape
            // added on its row would land somewhere it cannot be seen
            // (issue 82).
            Some(node) if node.can_hold_children() && !node.is_split() => (at, self.scene.node(at).children.len()),
            Some(node) => match node.parent {
                Some(parent) => {
                    let after = self.scene.node(parent).children.iter().position(|&c| c == at).map_or(0, |i| i + 1);
                    (parent, after)
                }
                None => end_of_root,
            },
            None => end_of_root,
        }
    }

    /// Bake the selection into stored geometry (issue 80).
    ///
    /// What is captured is what the node evaluates to -- booleans done, pattern
    /// copies laid down -- in the node's *own* frame, so its position, rotation
    /// and scale still mean what they meant and the shape does not move at the
    /// moment it is converted.
    pub fn convert_selection_to_mesh(&mut self) {
        let targets = self.top_level_selection();
        if targets.is_empty() {
            self.status = Status::Warning("Select something to convert".into());
            return;
        }
        if targets.iter().all(|&id| self.scene.node(id).is_mesh()) {
            self.status = Status::Info("That is already a mesh".into());
            return;
        }
        let mut baked: Vec<(NodeId, simple3d_core::mesh_data::MeshData)> = Vec::new();
        for &id in &targets {
            if self.scene.node(id).is_mesh() {
                continue;
            }
            let mesh = simple3d_core::eval::baked_mesh(&self.scene, id);
            if mesh.triangle_count() == 0 {
                continue;
            }
            baked.push((id, simple3d_core::mesh_data::MeshData::new(mesh)));
        }
        if baked.is_empty() {
            self.status = Status::Warning("There is no geometry there to convert".into());
            return;
        }
        self.edit("Convert to a mesh", None);
        let mut triangles = 0;
        for (id, mesh) in baked {
            triangles += mesh.triangle_count();
            self.scene.convert_to_mesh(id, mesh);
        }
        // The children went with the old body, so anything selected inside one
        // of them no longer exists.
        self.selection.retain(|id| self.scene.contains(*id));
        if self.selection.is_empty() {
            self.select_only(targets[0]);
        }
        self.status = Status::Info(format!(
            "Converted to {triangles} triangle{} of geometry -- the parameters behind it are gone",
            plural(triangles)
        ));
    }

    /// Stand a split where a node stands, holding `pieces`, and select it
    /// (issue 82). The cutting of a shape into a pattern of cells ends here.
    ///
    /// A node that is *already* a split keeps its identity and the shape it was
    /// made from -- only its pieces are replaced -- which is what makes cutting
    /// a split again a change of pattern rather than a split of a split, and
    /// what keeps one Join back together enough to undo any number of them.
    ///
    /// Returns the name the pieces were made from and how many there are, or
    /// `None` where the document could not be changed at all; the caller has
    /// taken the history snapshot and discards it in that case.
    pub(crate) fn hold_pieces(
        &mut self,
        id: NodeId,
        pieces: Vec<simple3d_geom::Mesh>,
        plan: Option<simple3d_geom::tiling::SplitPlan>,
    ) -> Option<(String, usize)> {
        let count = pieces.len();
        let split = if self.scene.node(id).is_split() {
            for child in self.scene.node(id).children.clone() {
                self.scene.remove(child);
            }
            if let Some(node) = self.scene.get_mut(id) {
                if let simple3d_core::scene::Body::Split { plan: was, .. } = &mut node.body {
                    *was = plan;
                }
            }
            id
        } else {
            let parent = self.scene.node(id).parent?;
            let index = self.scene.node(parent).children.iter().position(|&c| c == id).unwrap_or(0);
            // The shape itself, in the portable form the project file already
            // uses, taken before it leaves the document: it is what the split
            // holds, and what joining the pieces back together puts back.
            let original = self.scene.export_subtree(id)?;
            // Out first, so the split standing in its place can carry its name
            // rather than a numbered variant of it.
            self.scene.remove(id);
            self.scene.add_split(original, plan, parent, index)
        };
        let name = self.scene.node(split).name.clone();
        for (index, piece) in pieces.into_iter().enumerate() {
            // "Box Piece 3", not "Box 3": a piece is not another box, and
            // numbering it as one takes the number out of the series the
            // objects use -- eighty pieces called "Box 1" to "Box 80" leave the
            // next box the user adds to be called "Box 81". The name is taken
            // as it is, since the pieces of one collection are numbered apart
            // by construction.
            self.scene.add_mesh_named(
                format!("{name} Piece {}", index + 1),
                simple3d_core::mesh_data::MeshData::new(piece),
                split,
                index,
            );
        }
        self.collapsed.remove(&split);
        self.select_only(split);
        Some((name, count))
    }

    // -- a collection's pieces (issue 82) -----------------------------------

    /// Tick or untick one of a collection's pieces. `adding` is a click with
    /// Ctrl or Shift held, which adds to what is ticked rather than replacing
    /// it.
    pub(crate) fn tick_piece(&mut self, id: NodeId, adding: bool) {
        if !adding {
            let only = self.piece_ticks.len() == 1 && self.piece_ticks.contains(&id);
            self.piece_ticks.clear();
            if only {
                return;
            }
            self.piece_ticks.insert(id);
            return;
        }
        if !self.piece_ticks.remove(&id) {
            self.piece_ticks.insert(id);
        }
    }

    /// The object an in-place popup is drawing a preview over, if one is
    /// (issue 82).
    ///
    /// One question with one answer, asked by everything that has to behave
    /// differently while a preview is up: the viewport's grid and axes, what is
    /// drawn as the model, the renderables kept ready, and the frame cache's
    /// key. Only the split tool has a preview today; the next tool that grows
    /// one answers here too, and nothing downstream has to learn about it.
    pub(crate) fn preview_subject(&self) -> Option<NodeId> {
        self.split_tool.as_ref().map(|tool| tool.target).filter(|&id| self.scene.contains(id))
    }

    /// The collection whose pieces the properties panel is listing: the one
    /// selected node, when it is a collection at all.
    pub(crate) fn listed_collection(&self) -> Option<NodeId> {
        let id = self.primary()?;
        (self.selection.len() == 1 && self.scene.is_collection(id)).then_some(id)
    }

    /// Lift the ticked pieces out of the collection, giving each a row of its
    /// own under it (issue 82).
    ///
    /// Extracting every last piece is a different thing and goes through
    /// [`App::extract_all_pieces`]: with nothing left inside it a collection is
    /// a union group, and turning it into one throws the recipe away.
    pub fn extract_ticked_pieces(&mut self, collection: NodeId) {
        let ticked: Vec<NodeId> = self.piece_ticks.iter().copied().collect();
        if ticked.is_empty() {
            self.status = Status::Warning("Tick the pieces to extract first".into());
            return;
        }
        let inside: Vec<NodeId> =
            self.scene.node(collection).children.iter().copied().filter(|c| !self.scene.node(*c).extracted).collect();
        if inside.iter().all(|id| ticked.contains(id)) {
            // The last piece out empties the collection, which is the one case
            // that has to ask before it acts.
            self.ask_to_extract_all(collection);
            return;
        }
        self.edit("Extract pieces", None);
        let moved = self.scene.extract_pieces(collection, &ticked);
        if moved == 0 {
            self.history.discard_last();
            self.status = Status::Info("Those pieces are already extracted".into());
            return;
        }
        self.collapsed.remove(&collection);
        // The ticks stay. Extract and Put back are the two directions of one
        // gesture, and clearing them left Put back greyed out the moment
        // anything had been extracted -- the way back was to find the same
        // pieces in the list and tick them again.
        self.status = Status::Info(format!("Extracted {} {}", moved, if moved == 1 { "piece" } else { "pieces" }));
    }

    /// Put the ticked pieces back inside the collection, losing their rows.
    pub fn return_ticked_pieces(&mut self, collection: NodeId) {
        let ticked: Vec<NodeId> = self.piece_ticks.iter().copied().collect();
        if ticked.is_empty() {
            self.status = Status::Warning("Tick the pieces to put back first".into());
            return;
        }
        self.edit("Put pieces back", None);
        let moved = self.scene.return_pieces(collection, &ticked);
        if moved == 0 {
            self.history.discard_last();
            self.status = Status::Info("Those pieces are already inside".into());
            return;
        }
        // Still ticked, so they can go straight back out again -- and so the
        // viewport keeps saying which pieces were just put away.
        self.status = Status::Info(format!("Put {} {} back", moved, if moved == 1 { "piece" } else { "pieces" }));
    }

    /// Ask before emptying a collection, because that is where the break stops
    /// being reversible: with every piece extracted the collection is a union
    /// group, and the object it was made from goes with it (issue 82).
    pub fn ask_to_extract_all(&mut self, collection: NodeId) {
        if !self.scene.is_collection(collection) {
            return;
        }
        self.confirm_extract = Some(collection);
        self.modal = Modal::ConfirmExtractAll;
    }

    /// Extract every piece, which leaves the collection with nothing inside it
    /// and so turns it into an ordinary union group (issue 82).
    pub fn extract_all_pieces(&mut self, collection: NodeId) {
        if !self.scene.is_collection(collection) {
            return;
        }
        let count = self.scene.node(collection).children.len();
        let name = self.scene.node(collection).name.clone();
        self.edit("Extract every piece", None);
        self.scene.dissolve_collection(collection);
        self.collapsed.remove(&collection);
        self.piece_ticks.clear();
        self.select_only(collection);
        self.status = Status::Info(format!(
            "Extracted {count} {} -- {name} is a union group now",
            if count == 1 { "piece" } else { "pieces" }
        ));
    }

    /// How to undo a split, in the words the status line ends with. Said in the
    /// same breath as the split itself, because a split that cannot be seen to
    /// be reversible is one nobody tries.
    pub(crate) fn way_back(&self) -> String {
        let shortcut = self.keymap.shortcut_text(simple3d_core::keymap::Command::Rejoin);
        if shortcut.is_empty() {
            "they can be joined back together".to_string()
        } else {
            format!("{shortcut} joins them back together")
        }
    }

    /// Put a shape that was cut into pieces back together (issue 82): the
    /// object returns, with its parameters and its operands, and the pieces go.
    ///
    /// The split keeps its own transform through this, so pieces that were moved
    /// about as one item come back where they now stand rather than where the
    /// shape was when it was broken. What was done to the pieces themselves is
    /// let go -- they are triangles, and what comes back is the recipe -- so
    /// this is a step the history holds like any other.
    pub fn rejoin_selection(&mut self) {
        let targets = self.top_level_selection();
        let Some(&id) = targets.first() else {
            self.status = Status::Warning("Select a shape that was split into pieces to join back together".into());
            return;
        };
        if targets.len() > 1 {
            self.status = Status::Warning("Join one split shape back together at a time".into());
            return;
        }
        if !self.scene.node(id).is_split() {
            self.status =
                Status::Warning("Only something that was split into pieces can be joined back together".into());
            return;
        }
        let pieces = self.scene.node(id).children.len();
        self.edit("Join the pieces back together", None);
        let Some(restored) = self.scene.restore_split(id) else {
            self.history.discard_last();
            self.status = Status::Warning("The object this was made from could not be rebuilt".into());
            return;
        };
        let node = self.scene.node(restored);
        let (name, kind) = (node.name.clone(), node.kind_label());
        self.collapsed.remove(&restored);
        self.select_only(restored);
        self.status = Status::Info(format!("Joined {pieces} pieces back into {name}, the {kind} they came from"));
    }

    /// Add an empty pattern, for shapes to be put under it afterwards
    /// (issue 67).
    ///
    /// The "add a container" gesture, which is why it sits beside "Union group"
    /// in both Add menus. `Command::Pattern` is the other half of the same
    /// feature and does the opposite thing -- it wraps whatever is selected --
    /// so this one always makes an empty pattern, whatever is selected.
    pub fn add_pattern_at(&mut self, at: NodeId) {
        let where_to = self.insertion_from_row(at);
        self.add_empty_pattern(where_to);
    }

    /// The same, at the document's own insertion point rather than at a row.
    pub fn add_pattern(&mut self) {
        let where_to = self.scene.insertion_point(self.primary());
        self.add_empty_pattern(where_to);
    }

    fn add_empty_pattern(&mut self, (parent, index): (NodeId, usize)) {
        self.edit("Add pattern", None);
        let id = self.scene.add_pattern(parent, index);
        self.collapsed.remove(&parent);
        self.select_only(id);
        self.status = Status::Info("Added an empty pattern: put shapes into it and it repeats them".into());
    }

    /// Add a shape dragged out of the palette, exactly where the drop indicator
    /// said it would land.
    ///
    /// Where a shape *added* lands is a document setting -- at the view centre,
    /// beside the selection, at the 3D cursor -- and this keeps to it: the drag
    /// says which row of the tree the shape belongs on, not where in the world
    /// it sits, and the two are separate questions.
    pub fn add_dropped_primitive(&mut self, type_id: &str, parent: NodeId, index: usize) {
        self.edit("Add", None);
        let Some(id) = self.scene.add_primitive(type_id, parent, index) else {
            self.history.discard_last();
            self.status = Status::Warning("That shape is not in the palette".into());
            return;
        };
        let at = self.insertion_point_world(self.near_face_x(&[id]).unwrap_or(0.0));
        if let Some(node) = self.scene.get_mut(id) {
            node.position = at;
        }
        self.collapsed.remove(&parent);
        self.select_only(id);
        self.status = Status::Info(format!("Added {}", self.scene.node(id).name));
    }

    pub fn add_node_at(&mut self, at: NodeId, type_id: Option<&str>, op: GroupOp) {
        self.edit(if type_id.is_some() { "Add" } else { "Add group" }, None);
        let (parent, index) = self.insertion_from_row(at);
        let created = match type_id {
            Some(type_id) => self.scene.add_primitive(type_id, parent, index),
            None => Some(self.scene.add_group(op, parent, index)),
        };
        let Some(id) = created else {
            self.history.discard_last();
            self.status = Status::Warning("That shape is not in the palette".into());
            return;
        };
        // A pattern that has just gained its first child can now be measured.
        self.size_fresh_patterns();
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
                // Geometry snapping (issue 68) rides on top of the grid drag: a
                // move puts one of the carried body's own features on a body
                // under the pointer, and a face resize puts the face it is
                // pulling there. A rotation and a corner resize keep the grid --
                // an angle has no feature to land on, and a corner has no single
                // face to place.
                if self.snap_requested && matches!(handle, Handle::ResizeFace(..)) {
                    let caught = self.apply_resize_snap(id, view, cursor, mods);
                    self.snap_indicator = caught.map(|(at, _)| at);
                    // The readout the grid resize wrote describes the extent the
                    // cursor asked for, which the snap has just overridden.
                    if let (Some((_, extent)), Handle::ResizeFace(axis, _)) = (caught, handle) {
                        let unit = self.scene.settings.unit;
                        if let Some(drag) = self.drag.as_mut() {
                            drag.readout = format!(
                                "snap {} {}",
                                gizmo::axis_name(axis),
                                simple3d_core::unit::format_length(extent, unit)
                            );
                        }
                    }
                } else if self.snap_requested && matches!(handle, Handle::MoveAxis(_) | Handle::MovePlane(_)) {
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
                // A pattern that has just gained its first child can now be
                // measured, the same as when one is dropped in or pasted in.
                // Without this an empty pattern kept the stock 20 mm step, and
                // a 20 mm shape added into it afterwards was repeated at
                // exactly its own width -- one welded bar rather than shapes
                // standing clear, which is not what the tool gives for the same
                // shapes the other way round.
                self.size_fresh_patterns();
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
        self.size_fresh_patterns();
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
        // Everything the export needs is settled here, before the dialog goes
        // up, and travels with it: the answer arrives on a later frame, and the
        // job must be the one the user asked for and not whatever the document
        // looks like by the time they have finished choosing a folder.
        let options = simple3d_export::Options {
            format: self.export_format,
            scale,
            unit: simple3d_export::Unit3mf::Millimeter,
            allow_invalid: false,
            bodies,
        };
        let extension = self.export_format.extension();
        let format_id = self.export_format.id().to_string();
        let bodies_id = self.export_bodies.id().to_string();
        self.ask_for_file("Export", dialog, true, move |app, mut path| {
            if path.extension().is_none() {
                path.set_extension(extension);
            }
            app.settings.last_export_dir = path.parent().map(|p| p.to_path_buf());
            app.settings.last_export_format = format_id;
            app.settings.last_export_scale = scale;
            app.settings.last_export_bodies = bodies_id;
            app.export_job = Some(ExportJob::spawn_parts(path, parts, options, EXPORT_LIMIT));
            app.modal = Modal::None;
        });
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
        self.persisted_settings = self.settings.clone();
        self.settings_written = Some(Instant::now());
        self.persist_keymap();
    }

    /// Remember how big the window is and whether it is maximized, so the next
    /// run opens the way this one was left (issue 95).
    ///
    /// Nothing ever wrote these two. `main` reads `window_size` and
    /// `window_maximized` out of the settings to build the window, and no code
    /// path put a new value back -- so however the window was left, every run
    /// opened at the 1400 x 880 default. They are read off the window itself
    /// here, on every frame, and the ordinary save-on-change below writes them
    /// out; sampling them in `on_exit` instead would lose them to exactly the
    /// endings that setting has already been fixed for.
    ///
    /// The size is only taken while the window is in its ordinary state. What a
    /// maximized or fullscreen window reports is the screen, and restoring the
    /// screen as the *unmaximized* size is how a window comes back filling the
    /// display with no way back to the size it used to have. Rounded to whole
    /// points because a resize otherwise writes the file for a fraction of a
    /// pixel of drift.
    fn record_window_shape(&mut self, ctx: &egui::Context) {
        let (size, maximized, fullscreen, minimized) = ctx.input(|i| {
            let viewport = i.viewport();
            (viewport.inner_rect.map(|rect| rect.size()), viewport.maximized, viewport.fullscreen, viewport.minimized)
        });
        // Only where the window system answers at all: a platform that reports
        // nothing must not reset a maximized window to "not maximized".
        if let Some(maximized) = maximized {
            self.settings.window_maximized = maximized;
        }
        let ordinary = !maximized.unwrap_or(false) && !fullscreen.unwrap_or(false) && !minimized.unwrap_or(false);
        if let Some(size) = size.filter(|_| ordinary) {
            let size = [size.x.round(), size.y.round()];
            if size.iter().all(|n| n.is_finite() && *n >= 1.0) {
                self.settings.window_size = size;
            }
        }
    }

    /// Write the settings out as soon as they change, rather than only when the
    /// application is closed cleanly.
    ///
    /// A setting changed in the property panel -- where a shape lands, the snap
    /// mode, a snap step -- used to live in memory until `on_exit` ran, so
    /// anything that ended the process another way took it with it: a crash, a
    /// kill, a power cut. The keymap has been written on the spot since
    /// acceptance criterion 28 asked for a rebinding to survive a hard kill, and
    /// there is no reason the rest of the settings deserve less.
    ///
    /// Noticed by comparing rather than by calling `persist` from each of the
    /// dozen places that change something, because that is a list nobody keeps
    /// complete. Rate-limited because a scrubbed number changes every frame,
    /// and the frame that the limit turned away asks for one more frame so the
    /// value is not left unwritten until something else happens to repaint.
    fn persist_settings_if_changed(&mut self, ctx: &egui::Context) {
        const GAP: Duration = Duration::from_millis(250);
        if self.settings == self.persisted_settings {
            return;
        }
        match self.settings_written.map(|at| at.elapsed()) {
            Some(since) if since < GAP => ctx.request_repaint_after(GAP - since),
            _ => {
                let _ = config::save_settings_to(&self.config_dir, &self.settings);
                self.persisted_settings = self.settings.clone();
                self.settings_written = Some(Instant::now());
            }
        }
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

/// A file dialog that has been put up, and what to do with the path it comes
/// back with.
///
/// The dialog used to be called straight from `App::update`. `rfd` 0.17 is in
/// the lock file with neither `ashpd` nor `gtk`, so on Linux it is the raw
/// D-Bus portal backend: it talks to the portal itself and waits on `pollster`,
/// on the calling thread. A portal that is slow, absent or confused therefore
/// took the whole application down with it -- the last frame stayed on screen
/// with its hover states frozen mid-frame, and the process had to be killed.
///
/// It waits on its own thread now. The window keeps drawing, the footer says
/// what is being waited for and offers a way to stop waiting, and an answer
/// that never comes costs nothing but a parked thread.
pub(crate) struct FilePrompt {
    what: &'static str,
    answer: std::sync::mpsc::Receiver<Option<std::path::PathBuf>>,
    then: FollowUp,
    started: std::time::Instant,
}

/// What to do with the path a dialog answers with, on the frame it arrives.
/// Runs on the interaction thread, so it can touch the whole application.
type FollowUp = Box<dyn FnOnce(&mut App, std::path::PathBuf)>;

impl FilePrompt {
    pub(crate) fn what(&self) -> &'static str {
        self.what
    }

    pub(crate) fn waiting_for(&self) -> std::time::Duration {
        self.started.elapsed()
    }
}

/// Put the dialog up on a thread of its own and hand back the channel its
/// answer will arrive on.
///
/// Not on macOS, where AppKit requires a file dialog to be run from the main
/// thread and moving it would be a crash rather than a fix. The bug this
/// addresses is the Linux portal's, and the platform that cannot have the fix
/// is the one that does not have the bug.
fn ask_for_path(dialog: rfd::FileDialog, saving: bool) -> std::sync::mpsc::Receiver<Option<std::path::PathBuf>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let run = move || if saving { dialog.save_file() } else { dialog.pick_file() };
    #[cfg(target_os = "macos")]
    let _ = tx.send(run());
    #[cfg(not(target_os = "macos"))]
    std::thread::Builder::new()
        .name("simple3d-file-dialog".into())
        .spawn(move || {
            let _ = tx.send(run());
        })
        .expect("the platform can start a thread");
    rx
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
        // Before the panels rather than after: what they change this frame is
        // written on the next one, and a frame is drawn for every change any of
        // them makes.
        self.record_window_shape(ctx);
        self.persist_settings_if_changed(ctx);
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
        // Over the viewport, and drawn after it so it is the layer above: an
        // in-place popup is part of the picture rather than a window in front of
        // the application (issue 82).
        crate::split_tool::show(self, ctx);
        crate::measure_tool::show(self, ctx);
        crate::section_tool::show(self, ctx);
        crate::dock::resolve_drag(self, ctx);
        // A dialog is modal, and it was only half of one: `handle_shortcuts`
        // hands it the keyboard, but nothing stopped the main window taking the
        // pointer, so a dialog left open behind it was an application whose
        // shortcuts had all silently stopped while the document could still be
        // edited by mouse -- including, with the Export dialog up, editing the
        // very geometry that dialog is reporting on.
        //
        // Drawn before the dialog itself, so the dialog's own layer is created
        // after this one and therefore sits above it: an embedded dialog is a
        // window in this same viewport and has to stay live. A dialog that is a
        // window of its own is a viewport of its own, which this cannot reach.
        if self.modal != Modal::None {
            egui::Modal::new(egui::Id::new("dialog-backdrop"))
                .backdrop_color(egui::Color32::from_black_alpha(64))
                .frame(egui::Frame::NONE)
                .show(ctx, |_ui| {});
        }
        self.modals(ctx);
        // A load let go of somewhere no drop could take it is simply put down.
        // The outliner ends a drag it can see the end of, but a shape picked up
        // from the palette can be released over a window with no outliner in it
        // at all -- and a drag left running would have shown a phantom slab the
        // next time the tree was opened.
        if ctx.input(|i| !i.pointer.any_down()) {
            self.outliner_drag = None;
            self.drop_target = None;
        }
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
        self.poll_split();
        self.poll_file_prompt();
        self.advance_camera();
        self.refresh_node_renderables();
        if let Some(text) = self.clipboard_text.take() {
            ctx.copy_text(text);
        }

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
        } else if self.worker.is_busy() || self.export_job.is_some() || self.split_job.is_some() {
            // Progress and the preview, at a rate a person can read rather than
            // at whatever the rasterizer can manage.
            ctx.request_repaint_after(Duration::from_millis(33));
        } else if self.file_prompt.is_some() {
            // The dialog answers on a thread, and nothing else will wake this
            // loop to notice: the answer arrives with no event of its own. Four
            // times a second is enough to pick it up without being felt, and it
            // keeps the footer's count of seconds honest.
            ctx.request_repaint_after(Duration::from_millis(250));
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
    use simple3d_core::primitive::{ParamValue, ParamsExt};
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

    /// Draw one frame with a given modifier state and key events, so a binding
    /// that is nothing but a held modifier can be typed at the application the
    /// way a hand types it (issue 76).
    fn draw_frame_with(app: &mut App, modifiers: egui::Modifiers, events: Vec<egui::Event>) {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 880.0))),
            modifiers,
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    }

    /// Issue 76, through the application rather than through `ChordHold` alone:
    /// the keymap editor records a modifier held on its own, the binding it
    /// writes reads back as that modifier, and holding it afterwards is what
    /// asks for a geometry snap.
    ///
    /// The recorder and the shortcut dispatcher watch the same hold from two
    /// different places, and the editor's is fed by *its own window's* input --
    /// which is exactly the seam a unit test of the hold cannot reach.
    #[test]
    fn a_modifier_alone_can_be_bound_in_the_editor_and_held_afterwards() {
        let mut app = headless_app();
        // Something with no modifier in it to begin with, so what is recorded
        // cannot be what was already there.
        app.keymap.set(Command::ToggleBoundingBox, Chord::key("B"), true).unwrap();
        app.modal = Modal::Keymap;
        app.recording = Some(Command::ToggleBoundingBox);

        // Alt goes down, is held for a frame, and comes up with nothing under it.
        draw_frame_with(&mut app, egui::Modifiers::ALT, Vec::new());
        assert_eq!(app.recording, Some(Command::ToggleBoundingBox), "the press alone should not finish the binding");
        draw_frame_with(&mut app, egui::Modifiers::ALT, Vec::new());
        draw_frame_with(&mut app, egui::Modifiers::NONE, Vec::new());

        assert_eq!(app.recording, None, "the release should have finished the recording");
        assert_eq!(app.keymap.binding(Command::ToggleBoundingBox), Some(&Chord::modifiers(false, false, true)));
        assert_eq!(app.keymap.shortcut_text(Command::ToggleBoundingBox), "Alt");
        app.modal = Modal::None;

        // And the other half: the default hold for geometry snapping is Ctrl on
        // its own, and holding it is what turns snapping on.
        app.keymap = Keymap::default();
        app.settings.geometry_snap = simple3d_core::config::SnapMode::WhileHeld;
        assert_eq!(app.keymap.binding(Command::SnapToGeometry), Some(&Chord::modifiers(true, false, false)));
        assert!(!app.geometry_snap_wanted(|_| false, egui::Modifiers::NONE));
        assert!(
            app.geometry_snap_wanted(|_| false, egui::Modifiers::COMMAND),
            "Ctrl on its own did not ask for a snap"
        );
    }

    /// The other half of issue 76: an ordinary key held while another goes down
    /// is a combination, and it fires on the key that completes it rather than
    /// each key firing its own binding on the way.
    #[test]
    fn keys_held_together_bind_as_one_combination() {
        let mut app = headless_app();
        app.modal = Modal::Keymap;
        app.recording = Some(Command::ToggleBoundingBox);
        let down = |key: egui::Key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        // Q, then W on top of it, then both let go.
        draw_frame_with(&mut app, egui::Modifiers::NONE, vec![down(egui::Key::Q)]);
        draw_frame_with(&mut app, egui::Modifiers::NONE, vec![down(egui::Key::W)]);
        draw_frame_with(&mut app, egui::Modifiers::NONE, Vec::new());
        assert_eq!(app.recording, None, "the release should have finished the recording");
        assert_eq!(app.keymap.shortcut_text(Command::ToggleBoundingBox), "Q+W");
        app.modal = Modal::None;
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
            Modal::PatternKind,
        ] {
            app.modal = modal;
            draw_one_frame(&mut app);
        }
        app.modal = Modal::None;
    }

    /// The custom pattern kind creation tool, end to end (issue 67): it makes a
    /// pattern out of what is selected, builds a rule that no fixed kind can
    /// say, keeps it under a name, and puts it back on a different pattern in a
    /// different project.
    #[test]
    fn a_custom_pattern_kind_can_be_built_saved_and_used_again() {
        let dir = temp_config_dir("pattern-kind");
        let mut app = app_in(dir.clone());
        let root = app.scene.root();
        let shape = app.scene.add_primitive("box", root, 0).unwrap();
        app.select_only(shape);

        // Opening the tool with a shape selected wraps it, which is what makes
        // this a creation tool rather than an editor of something already there.
        app.open_pattern_tool();
        let pattern = app.pattern_tool.expect("the tool should have a pattern to work on");
        assert!(app.scene.node(pattern).is_pattern());
        assert_eq!(app.scene.node(pattern).params().unwrap().int("kind"), simple3d_core::pattern::CUSTOM);
        assert_eq!(app.modal, Modal::PatternKind);
        draw_one_frame(&mut app);

        // A row of three, turned four times about Z: two stages, and twelve
        // copies that no single fixed kind lays out.
        let mut stage = simple3d_core::pattern::stage(app.scene.node(pattern).params().unwrap(), 1);
        stage.count = 4;
        stage.turn = 90.0;
        stage.step = simple3d_geom::Vec3::ZERO;
        {
            let params = app.scene.get_mut(pattern).and_then(|n| n.params_mut()).unwrap();
            params.insert("stages".to_string(), simple3d_core::primitive::ParamValue::Count(2));
            params.insert("stage1_count".to_string(), ParamValue::Count(3));
            simple3d_core::pattern::set_stage(params, 1, stage);
        }
        let params = app.scene.node(pattern).params().unwrap().clone();
        assert_eq!(simple3d_core::pattern::instance_count(&params).1, 12);
        draw_one_frame(&mut app);

        app.pattern_tool_name = "Turned row".to_string();
        app.save_current_kind();
        assert_eq!(app.pattern_kinds.iter().map(|e| e.name.clone()).collect::<Vec<_>>(), vec!["Turned row"]);

        // A second, unrelated pattern in a fresh document takes the same rule.
        let mut other = app_in(dir);
        let root = other.scene.root();
        let shape = other.scene.add_primitive("cylinder", root, 0).unwrap();
        other.select_only(shape);
        other.open_pattern_tool();
        let saved = other.pattern_kinds.first().cloned().expect("the shelf should have the saved kind");
        other.apply_saved_kind(&saved);
        let applied = other.pattern_tool.and_then(|id| other.scene.node(id).params().cloned()).unwrap();
        assert_eq!(applied.int("kind"), simple3d_core::pattern::CUSTOM);
        assert_eq!(simple3d_core::pattern::instance_count(&applied).1, 12, "the saved rule did not come back");
        assert_eq!(other.scene.node(other.pattern_tool.unwrap()).name, "Turned row");

        // And it comes off the shelf again when it is deleted.
        other.delete_saved_kind(&saved);
        assert!(other.pattern_kinds.is_empty());
    }

    /// The rule the tool builds is an ordinary parameter edit, which is the
    /// whole reason it was built out of parameters: undo covers it, and so does
    /// saving and reloading the project.
    #[test]
    fn a_custom_rule_survives_undo_and_a_round_trip_through_the_project_file() {
        let mut app = headless_app();
        let root = app.scene.root();
        let shape = app.scene.add_primitive("box", root, 0).unwrap();
        app.select_only(shape);
        app.open_pattern_tool();
        let pattern = app.pattern_tool.unwrap();

        app.edit("Pattern stages", None);
        {
            let params = app.scene.get_mut(pattern).and_then(|n| n.params_mut()).unwrap();
            params.insert("stages".to_string(), ParamValue::Count(3));
        }
        assert_eq!(app.scene.node(pattern).params().unwrap().int("stages"), 3);
        app.run(Command::Undo);
        assert_eq!(app.scene.node(pattern).params().unwrap().int("stages"), 1, "undo did not reach the stage count");
        app.run(Command::Redo);
        assert_eq!(app.scene.node(pattern).params().unwrap().int("stages"), 3);

        let text = simple3d_core::project::to_string(&app.scene);
        let back = simple3d_core::project::from_str(&text).expect("the project should reload");
        let reloaded = back
            .depth_first()
            .into_iter()
            .find(|id| back.node(*id).is_pattern())
            .and_then(|id| back.node(id).params().cloned())
            .expect("the pattern should have survived the file");
        assert_eq!(reloaded.int("kind"), simple3d_core::pattern::CUSTOM);
        assert_eq!(reloaded.int("stages"), 3);
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
    fn a_setting_changed_in_the_panel_is_on_disk_before_the_application_closes() {
        // The user set "Add at" to the view centre, the process was killed
        // rather than quit, and the next run opened on the origin again. Only
        // `on_exit` wrote the settings, so anything that ended the process
        // another way -- a crash, a kill, the machine going down -- took every
        // setting changed that session with it. The keymap has been written on
        // the spot since acceptance criterion 28 asked for a rebinding to
        // survive a hard kill; this is the rest of the settings catching up.
        //
        // A real frame is what has to write it, so a real frame is what this
        // draws: calling the writer directly would pass on the broken code,
        // where nothing called it until the application closed.
        let dir = temp_config_dir("settings-survive-a-kill");
        let ctx = egui::Context::default();
        let mut app = app_in(dir.clone());
        assert_eq!(app.settings.placement, Placement::Origin, "the default this test is about has changed");

        // Changed as the panel changes it, and then one frame of the running
        // application -- and then the process is gone.
        app.settings.placement = Placement::ViewCentre;
        app.settings.rotate_snap_deg = 22.5;
        // A window's worth of screen: the default is unbounded, and the
        // viewport would try to rasterize a texture the size of it.
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0))),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
        drop(app);

        let next = app_in(dir.clone());
        assert_eq!(next.settings.placement, Placement::ViewCentre, "\"Add at\" was lost with the process");
        assert_eq!(next.settings.rotate_snap_deg, 22.5, "the snap setting was lost with the process");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_window_size_and_maximized_state_are_remembered_for_the_next_run() {
        // Issue 95. `main` builds the window out of `window_size` and
        // `window_maximized`, and nothing ever wrote either of them back: the
        // window was resized, the application closed cleanly, and the next run
        // opened at the 1400 x 880 default again.
        //
        // Driven through a real frame, because reading the window is something
        // only a frame can do -- calling the writer directly would pass on the
        // broken code, where no frame ever called it.
        let dir = temp_config_dir("window-shape");
        let ctx = egui::Context::default();
        let mut app = app_in(dir.clone());
        assert_eq!(app.settings.window_size, [1400.0, 880.0], "the default this test is about has changed");

        let frame = |app: &mut App, size: egui::Vec2, maximized: bool| {
            let mut viewports = egui::ViewportIdMap::default();
            viewports.insert(
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    inner_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                    maximized: Some(maximized),
                    ..Default::default()
                },
            );
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                viewports,
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| app.ui(ctx));
        };

        frame(&mut app, egui::vec2(1000.0, 700.0), false);
        assert_eq!(app.settings.window_size, [1000.0, 700.0], "the size the window was left at was not recorded");
        assert!(!app.settings.window_maximized);

        // Maximized, the size the window reports is the screen's -- and that is
        // exactly the size it must not come back with once it is restored.
        frame(&mut app, egui::vec2(2560.0, 1440.0), true);
        assert!(app.settings.window_maximized, "the window being maximized was not recorded");
        assert_eq!(app.settings.window_size, [1000.0, 700.0], "the maximized size overwrote the restored size");

        // The write is rate-limited, so the frame that changed something asks
        // for a later one to carry it to disk. That frame is what this is: the
        // running application draws it on its own, and without it the test
        // would be asserting against the gap rather than against the setting.
        std::thread::sleep(Duration::from_millis(300));
        frame(&mut app, egui::vec2(2560.0, 1440.0), true);
        drop(app);

        let next = app_in(dir.clone());
        assert_eq!(next.settings.window_size, [1000.0, 700.0], "the size did not survive the process");
        assert!(next.settings.window_maximized, "the maximized state did not survive the process");
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
    fn the_turn_step_governs_a_rotate_nudge_rather_than_a_fixed_fifteen_degrees() {
        // Issue 98. A move and a resize have had a step to set since the
        // beginning; a turn was fifteen degrees with nowhere on screen to say
        // otherwise. It is a number like every other number now, and it is what
        // one press of a nudge key turns by.
        let mut app = headless_app();
        let id = app.primary().unwrap();
        assert_eq!(app.settings.rotate_snap_deg, 15.0, "the default this test is about has changed");
        app.run(Command::ModeRotate);

        app.settings.rotate_snap_deg = 5.0;
        let before = app.scene.node(id).rotation;
        app.run(Command::NudgeRight);
        let turned = (app.scene.node(id).rotation - before).length();
        assert!((turned - 5.0).abs() < 1e-9, "a nudge turned {turned} degrees rather than the 5 degree step");

        // And the step it is set to, not the one it used to be fixed at.
        assert!((turned - 15.0).abs() > 1e-9, "the nudge is still turning by the old fixed fifteen");
    }

    #[test]
    fn the_section_plane_lands_in_the_middle_of_the_model_and_moves_without_an_edit() {
        // Issue 71. Switched on at zero it would cut nothing at all for a part
        // standing beside the origin, and a section that appears to do nothing
        // reads as broken rather than as one that needs sliding.
        let mut app = headless_app();
        let id = app.primary().unwrap();
        app.scene.get_mut(id).unwrap().position = Vec3::new(40.0, 0.0, 0.0);
        app.reevaluate_for_test();
        let (lo, hi) = app.evaluated.mesh.bounds().expect("the plate is in the scene");
        // The plane stands on X to begin with, so that is the coordinate it
        // has to find the middle of.
        let middle = (lo.x + hi.x) / 2.0;

        assert!(!app.scene.settings.section.enabled, "a document starts whole");
        app.run(Command::ToggleSection);
        assert!(app.scene.settings.section.enabled);
        assert!(
            (app.scene.settings.section.offset - middle).abs() < 1e-9,
            "the plane opened at {} rather than the model's middle at {middle}",
            app.scene.settings.section.offset
        );

        // Moving it is not an edit: nothing about the model changed, so there
        // is nothing for undo to take back.
        let revision = app.history.revision();
        app.set_section_offset(middle + 5.0);
        assert_eq!(app.history.revision(), revision, "sliding the section wrote an undo step");
        assert!(!app.unsaved(), "sliding the section marked the document as changed");

        app.run(Command::ToggleSection);
        assert!(!app.scene.settings.section.enabled, "the switch does not switch off");
        assert!((app.scene.settings.section.offset - middle - 5.0).abs() < 1e-9, "the plane forgot where it stood");
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
            // The generation moves on with the result, exactly as it does when
            // the worker hands one back: everything that watches for "the model
            // changed" watches this, so a helper that left it alone would be a
            // helper nothing noticed.
            self.evaluation_generation += 1;
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
    #[test]
    fn a_file_dialog_does_not_hold_up_the_frame_and_never_answers_silently() {
        // The dialog used to be called straight from `App::update` and waited on
        // the calling thread, so a portal that was slow, absent or confused took
        // the whole application down with it -- the last frame stayed on screen
        // with its hover states frozen, and the process had to be killed. It
        // waits on a thread now and is picked up by `poll_file_prompt`.
        //
        // Driven through a channel of this test's own rather than a real
        // dialog: what is under test is that the answer arrives on a later
        // frame and that no outcome is silent.
        let mut app = app_in(temp_config_dir("file-prompt"));
        let (tx, rx) = std::sync::mpsc::channel();
        let chosen = std::rc::Rc::new(std::cell::RefCell::new(None));
        let sink = chosen.clone();
        app.file_prompt = Some(FilePrompt {
            what: "Save as",
            answer: rx,
            then: Box::new(move |_app, path| *sink.borrow_mut() = Some(path)),
            started: std::time::Instant::now(),
        });

        // Nothing yet, and nothing blocking: the frame goes on without an answer.
        app.poll_file_prompt();
        assert!(app.file_prompt.is_some(), "the prompt was given up on before it answered");
        assert!(chosen.borrow().is_none());

        tx.send(Some(PathBuf::from("/tmp/simple3d-test.simple3d"))).expect("the prompt is listening");
        app.poll_file_prompt();
        assert!(app.file_prompt.is_none(), "the answered prompt was not cleared");
        assert_eq!(chosen.borrow().as_deref(), Some(Path::new("/tmp/simple3d-test.simple3d")));

        // A dialog that answers with nothing says so. Silence there was the
        // other half of the bug: Ctrl+S on an unsaved document produced no
        // dialog and no message, which is indistinguishable from a cancel.
        let (tx, rx) = std::sync::mpsc::channel();
        app.file_prompt = Some(FilePrompt {
            what: "Save as",
            answer: rx,
            then: Box::new(|_app, _path| panic!("a cancelled dialog must not act")),
            started: std::time::Instant::now(),
        });
        tx.send(None).expect("the prompt is listening");
        app.poll_file_prompt();
        assert!(app.file_prompt.is_none());
        assert!(app.status_text().contains("cancelled"), "a cancelled dialog said nothing: {}", app.status_text());
    }

    /// The selection outline draws the shape a node evaluates to, and stops
    /// there. It used to expand to the node *and all its descendants*, so a
    /// group's operands were outlined alongside its result: a difference's
    /// cutter as two rims hanging in mid-air beside the solid, an
    /// intersection's whole uncut box as a cage around the small lens it
    /// leaves, a pattern's source child standing where no copy of it does.
    #[test]
    fn a_selected_group_is_outlined_as_its_result_not_as_its_operands() {
        let mut app = app_in(temp_config_dir("outline-group"));
        let root = app.scene.root();
        let group = app.scene.add_group(GroupOp::Difference, root, 0);
        let plate = app.scene.add_primitive("plate", group, 0).expect("the plate is in the registry");
        let cutter = app.scene.add_primitive("box", group, 1).expect("the box is in the registry");
        app.reevaluate_for_test();
        let (plate_lo, plate_hi) = app.evaluated.node_meshes[&plate].bounds().expect("the plate has bounds");
        let (cut_lo, cut_hi) = app.evaluated.node_meshes[&cutter].bounds().expect("the cutter has bounds");
        assert!(cut_lo.z < plate_lo.z && cut_hi.z > plate_hi.z, "this test needs a cutter taller than the plate");

        app.select_only(group);
        app.renderable_key = u64::MAX;
        app.refresh_node_renderables();
        let outlined: Vec<NodeId> = app.node_renderables.keys().copied().collect();
        assert_eq!(outlined, vec![group], "the operands were outlined too");

        let (lo, hi) = app.node_renderables[&group].mesh.bounds().expect("the group outlines nothing");
        assert!(
            lo.z >= plate_lo.z - 1e-6 && hi.z <= plate_hi.z + 1e-6,
            "the outline reaches {lo:?}..{hi:?}, past the plate the cut left -- it is drawing the cutter"
        );
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

        app.outliner_drag = Some(Carried::Rows(third));
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

        app.outliner_drag = Some(Carried::Rows(group));
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

        // Issues 35 and 85: a drag through the picker paints on every frame it
        // moves, and each of those frames used to be able to take a slot --
        // which is how the row ended up holding eight shades of black. Nothing
        // is remembered until the picker is put away, however long the drag
        // takes or how often it pauses: a visit to the picker is one choice.
        for step in 0..40_u8 {
            app.paint_from_picker(&[id], [0x10 + step, 0x40, 0x90]);
            // The undo history's coalescing window ages out mid-drag on any
            // drag slower than a second, which is what used to split one visit
            // into a swatch per pause.
            app.history.close();
        }
        assert_eq!(
            app.custom_recent_colours(),
            vec![[0x77, 0x11, 0x22], [0x2E, 0x9A, 0xFF]],
            "the picker put a colour on the row before it was closed"
        );
        app.picker_closed();
        assert_eq!(
            app.custom_recent_colours(),
            vec![[0x37, 0x40, 0x90], [0x77, 0x11, 0x22], [0x2E, 0x9A, 0xFF]],
            "a single drag through the picker filled the recent row"
        );
        // Closing it again is not a second choice.
        app.picker_closed();
        assert_eq!(app.custom_recent_colours().len(), 3);

        // A second visit to the picker is a second choice.
        app.paint_from_picker(&[id], [0x01, 0x02, 0x03]);
        app.picker_closed();
        assert_eq!(app.custom_recent_colours()[..2], [[0x01, 0x02, 0x03], [0x37, 0x40, 0x90]]);

        // Issue 85: a shade of a colour already on the row is that colour, and
        // takes its slot rather than a slot of its own -- both on the way in,
        // and on the way out for a row an older version filled with them.
        app.paint_from_picker(&[id], [0x05, 0x06, 0x07]);
        app.picker_closed();
        assert_eq!(app.custom_recent_colours()[..2], [[0x05, 0x06, 0x07], [0x37, 0x40, 0x90]]);
        app.settings.recent_colours = (0..8).map(|n| [n, n, n]).collect();
        assert_eq!(
            app.custom_recent_colours(),
            vec![[0, 0, 0]],
            "a row an older version filled with shades of one black still shows eight of them"
        );

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
    fn a_measure_click_never_catches_what_the_body_hides() {
        // The bug the screencast showed: aimed anywhere at the top face of a
        // box, the tool jumped to corners and edges on the far side of it. They
        // are nowhere near the pointer in the model, but a solid projects its own
        // back over its front, so their projections land in the middle of the
        // face being pointed at -- and nothing there is drawn, which is what made
        // the catch look random.
        let mut app = headless_app();
        let root = app.scene.root();
        for id in app.scene.descendants(root) {
            app.scene.remove(id);
        }
        let id = app.scene.add_primitive("box", root, 0).unwrap();
        // Looking well down on the box, so its underside projects across its top.
        app.scene.camera.pitch = 45.0;
        app.reevaluate_for_test();
        let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
        let (lo, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();

        // The far bottom corner is one of them, and it used to be what a pointer
        // three quarters of the way across the top face caught.
        let buried = Vec3::new(lo.x, hi.y, lo.z);
        assert!(!app.in_clear_view(&view, buried), "this corner is not hidden in this view; pick another");

        // Every point of the top face, and nothing caught anywhere on it is a
        // thing the frame does not show.
        for row in 1..8 {
            for column in 1..8 {
                let at = Vec3::new(
                    lo.x + (hi.x - lo.x) * row as f64 / 8.0,
                    lo.y + (hi.y - lo.y) * column as f64 / 8.0,
                    hi.z,
                );
                let (screen, _) = view.project(at).unwrap();
                let caught = app.measure_point_at(&view, screen).expect("the top face is under the cursor");
                assert!(
                    (caught.at - buried).length() > 1e-6,
                    "aiming at {at:?} on the top face caught the corner buried behind it"
                );
                assert!(
                    app.in_clear_view(&view, caught.at),
                    "aiming at {at:?} caught {caught:?}, which the body hides"
                );
            }
        }
    }

    #[test]
    fn wireframe_hides_nothing_and_so_withholds_nothing() {
        // The far side of a body is drawn in wireframe exactly like the near
        // side -- that is the point of the mode -- so a corner around the back is
        // a corner on screen there, and catching it is aiming at what is drawn
        // rather than at what is not.
        let mut app = headless_app();
        let root = app.scene.root();
        for id in app.scene.descendants(root) {
            app.scene.remove(id);
        }
        let id = app.scene.add_primitive("box", root, 0).unwrap();
        app.scene.camera.pitch = 45.0;
        app.reevaluate_for_test();
        let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
        let (lo, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();

        let buried = Vec3::new(lo.x, hi.y, lo.z);
        let (screen, _) = view.project(buried).unwrap();
        assert!(!app.in_clear_view(&view, buried));
        let shaded = app.measure_point_at(&view, screen).unwrap();
        assert!((shaded.at - buried).length() > 1e-6, "a shaded body handed out the corner behind it");

        app.settings.display_mode = DisplayMode::Wireframe;
        let wire = app.measure_point_at(&view, screen).expect("the corner is under the cursor");
        assert_eq!(wire.kind, Some(crate::snap::FeatureKind::Vertex), "caught {wire:?}");
        assert!((wire.at - buried).length() < 1e-6, "caught {:?}, not the corner {buried:?}", wire.at);
    }

    #[test]
    fn a_measure_click_catches_the_mark_a_principal_plane_leaves_on_a_body() {
        // The line the renderer draws across the solid where a plane through the
        // origin cuts it is a line on screen like any other, and "how far along
        // this face is zero" is the measurement it exists to be read for. Until
        // now nothing was there to catch, and the pointer fell through it.
        let mut app = headless_app();
        let root = app.scene.root();
        for id in app.scene.descendants(root) {
            app.scene.remove(id);
        }
        let id = app.scene.add_primitive("box", root, 0).unwrap();
        // Off the origin on both of the ground's axes, so the mark on the top
        // face runs nowhere near that face's centre and only the mark can answer.
        app.scene.get_mut(id).unwrap().position = Vec3::new(5.0, 5.0, 0.0);
        app.reevaluate_for_test();
        let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
        let (_, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();

        // On the top face, on the line the x = 0 plane leaves across it, and well
        // clear of every corner, edge and centre of that face.
        let on_mark = Vec3::new(0.0, 10.0, hi.z);
        let (screen, _) = view.project(on_mark).unwrap();
        let caught = app.measure_point_at(&view, screen).expect("the top face is under the cursor");
        assert_eq!(caught.kind, Some(crate::snap::FeatureKind::PlaneMark), "caught {caught:?}");
        // Caught *on* the plane, not merely near it: that exactness is the whole
        // use of the mark.
        assert!(caught.at.x.abs() < 1e-6, "{:?} is off the x = 0 plane", caught.at);
        assert!((caught.at - on_mark).length() < 0.3, "caught {:?}, not the point pointed at", caught.at);

        // A little to the side of the mark there is no line to catch, and the
        // click lands on whatever is really there instead.
        let beside = app.measure_point_at(&view, screen + egui::vec2(30.0, 0.0)).unwrap();
        assert_ne!(beside.kind, Some(crate::snap::FeatureKind::PlaneMark), "the catch reached far past the mark");

        // The mark follows the switch of the axis it is *drawn as*, and the
        // switch for the marks themselves. Neither drawn nor caught.
        //
        // This one is left by the plane perpendicular to X, which is drawn in
        // Y's green and answers to the Y box: X's and Y's marks are exchanged on
        // purpose (`snap::MARK_AXIS`, issue 75).
        app.scene.settings.axes_visible[0] = false;
        let other = app.measure_point_at(&view, screen).unwrap();
        assert_eq!(other.kind, Some(crate::snap::FeatureKind::PlaneMark), "the wrong switch took this mark away");
        app.scene.settings.axes_visible[0] = true;
        app.scene.settings.axes_visible[1] = false;
        let hidden = app.measure_point_at(&view, screen).unwrap();
        assert_ne!(hidden.kind, Some(crate::snap::FeatureKind::PlaneMark), "a mark of a hidden plane was caught");
        app.scene.settings.axes_visible[1] = true;
        app.scene.settings.plane_marks = false;
        let off = app.measure_point_at(&view, screen).unwrap();
        assert_ne!(off.kind, Some(crate::snap::FeatureKind::PlaneMark), "a mark that is switched off was caught");

        // And there is no surface for one in wireframe, where none is drawn.
        app.scene.settings.plane_marks = true;
        app.settings.display_mode = DisplayMode::Wireframe;
        let wire = app.measure_point_at(&view, screen).unwrap();
        assert_ne!(wire.kind, Some(crate::snap::FeatureKind::PlaneMark), "a mark was caught where none is drawn");
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

    /// Issue 67: the tool's window is resizable, so what is in it has to fit
    /// whatever width the window is dragged to.
    ///
    /// Its three columns were three fixed widths -- 170 for the shelf, 300 for the
    /// stages and whatever was left for the picture -- which is 640-odd pixels of
    /// content whether or not the window has them. Dragged narrow, the picture was
    /// squeezed to nothing and the rest ran off the right-hand edge. The columns
    /// are shares of the room now, and give themselves up in order when there is
    /// not enough of it.
    ///
    /// The check is that nothing is laid out wider than the room it was given, at
    /// each of the widths where the layout changes its mind and either side of
    /// them, and it covers the button row too: that row fills from the right, so
    /// what overflows it runs off the *left* edge, which is where "Name" went.
    ///
    /// It stops at the window's own minimum size rather than going down to
    /// nothing. Below that there is no layout to find -- two buttons and a field
    /// are wider than that on their own -- which is why the window is not allowed
    /// to be dragged there.
    #[test]
    fn the_pattern_tool_fits_whatever_width_its_window_is_given() {
        let mut app = headless_app();
        app.open_pattern_tool();
        assert_eq!(app.modal, Modal::PatternKind, "the tool did not open, so this measures nothing");
        app.reevaluate_for_test();

        for width in [1400.0_f32, 900.0, 820.0, 640.0, 560.0, 470.0, 400.0, 360.0, 330.0, 320.0] {
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 520.0))),
                ..Default::default()
            };
            let (mut room, mut used) = (0.0_f32, 0.0_f32);
            let (mut row_room, mut row_used) = (0.0_f32, 0.0_f32);
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    room = ui.max_rect().width();
                    crate::pattern_tool::body(&mut app, ui);
                    used = ui.min_rect().width();
                    ui.separator();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        row_room = ui.max_rect().width();
                        crate::pattern_tool::actions(&mut app, ui);
                        row_used = ui.min_rect().width();
                    });
                });
            });
            // Half a pixel of slack: a rule and its spacing are rounded, and this
            // is looking for columns that do not fit, not for rounding.
            assert!(used <= room + 0.5, "at {room} px of room the tool laid out {used} px of content");
            assert!(row_used <= row_room + 0.5, "at {row_room} px of room the button row took {row_used} px");
        }
    }

    /// The split tool's window is resizable too, and it has the same two-column
    /// problem: the numbers beside a picture of the cells, and below the width
    /// both need, the picture is what gives way (issue 82).
    #[test]
    fn the_split_tool_fits_whatever_width_its_window_is_given() {
        let mut app = headless_app();
        app.open_split_tool();
        assert!(app.split_tool.is_some(), "the tool did not open, so this measures nothing");

        for kind in simple3d_geom::tiling::CellKind::ALL {
            app.split_tool.as_mut().unwrap().plan.passes[0].kind = kind;
            for width in [1400.0_f32, 900.0, 680.0, 560.0, 480.0, 400.0, 360.0, 320.0] {
                let ctx = egui::Context::default();
                crate::theme::apply(&ctx);
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 420.0))),
                    ..Default::default()
                };
                let (mut room, mut used) = (0.0_f32, 0.0_f32);
                let (mut row_room, mut row_used) = (0.0_f32, 0.0_f32);
                let _ = ctx.run(input, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        room = ui.max_rect().width();
                        crate::split_tool::body(&mut app, ui);
                        used = ui.min_rect().width();
                        ui.separator();
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            row_room = ui.max_rect().width();
                            crate::split_tool::actions(&mut app, ui);
                            row_used = ui.min_rect().width();
                        });
                    });
                });
                assert!(used <= room + 0.5, "{kind:?} at {room} px of room laid out {used} px of content");
                assert!(row_used <= row_room + 0.5, "at {row_room} px of room the button row took {row_used} px");
            }
        }
        // Drawing the tool must not have cut anything: nothing is cut until
        // Split is pressed.
        assert!(app.primary().is_some_and(|id| !app.scene.node(id).is_split()));
    }

    #[test]
    fn the_pattern_tools_stages_keep_one_width_however_wide_the_window_is() {
        // Widening the window widens the picture and nothing else. The stages
        // are labelled fields with a natural width, and stretching them with the
        // window put a hundred pixels of nothing between every name and its
        // number; the viewport is the half that is worth more the bigger it is.
        let mut app = headless_app();
        app.open_pattern_tool();
        assert_eq!(app.modal, Modal::PatternKind, "the tool did not open, so this measures nothing");
        app.reevaluate_for_test();

        // Every width at which both columns fit, which is where the promise
        // holds; below it the stages give width up rather than the picture
        // vanishing, and that is a different rule.
        let mut measured: Vec<(f32, egui::Rect)> = Vec::new();
        for width in [640.0_f32, 900.0, 1200.0, 1600.0, 2400.0] {
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 640.0))),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| crate::pattern_tool::body(&mut app, ui));
            });
            let grip = crate::panel_properties::grip_id("tool:1 Copies");
            let rect = ctx.read_response(grip).expect("the first stage's copies field is drawn").rect;
            measured.push((width, rect));
        }
        let (first_width, first) = measured[0];
        for (width, rect) in &measured {
            assert!(
                (rect.width() - first.width()).abs() < 1.0 && (rect.right() - first.right()).abs() < 1.0,
                "at {width} px the stage field is {:?}, against {first:?} at {first_width} px",
                rect
            );
        }
    }

    /// Reported: "why does the preview change when i change the values? this
    /// should not happen, the preview should be under user control."
    ///
    /// It framed itself again whenever the rule started laying its copies out
    /// somewhere else, so every number typed moved the camera: a picture turned
    /// and zoomed to look at one end of a run jumped back to the whole of it on
    /// the next keystroke.
    ///
    /// **This reverses an earlier fix**, whose test this replaces. That one was
    /// for "the preview does not show what the stages do" -- the camera was
    /// pointed once, when the tool opened, so adding a stage laid copies out
    /// past the edges of the picture. Reframing on every change answered it and
    /// took the camera off the user to do it. The picture going off the edges is
    /// now the expected thing, and the Frame button is the answer to it: it is
    /// one click, it says what it does, and it leaves the choice where it
    /// belongs. Do not put the automatic reframing back.
    #[test]
    fn editing_the_rule_leaves_the_previews_camera_alone() {
        let mut app = headless_app();
        app.open_pattern_tool();
        let pattern = app.pattern_tool.expect("the tool opened on a pattern");
        app.reevaluate_for_test();

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let draw = |app: &mut App| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1100.0, 700.0))),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| crate::pattern_tool::body(app, ui));
            });
        };
        // One frame to let it point itself at the pattern, which is the one
        // time it may.
        draw(&mut app);
        // Then the user looks where they want to look.
        app.pattern_preview_camera.yaw += 40.0;
        app.pattern_preview_camera.pitch -= 15.0;
        app.pattern_preview_camera.distance *= 0.35;
        draw(&mut app);
        let looked = app.pattern_preview_camera;

        // Now the rule lays out a great deal more, the way typing does.
        if let Some(params) = app.scene.get_mut(pattern).and_then(|n| n.params_mut()) {
            params.insert("stages".to_string(), simple3d_core::primitive::ParamValue::Count(3));
        }
        app.reevaluate_for_test();
        draw(&mut app);
        assert_eq!(app.pattern_preview_camera, looked, "editing the rule moved the preview's camera");

        // And Frame is still how the whole of it is asked for back.
        app.reset_pattern_preview();
        draw(&mut app);
        assert_ne!(app.pattern_preview_camera, looked, "Frame left the picture where the user had put it");
        let (lo, hi) = app.evaluated.node_world_bounds[&pattern];
        let rect = ctx.read_response(crate::pattern_tool::preview_id()).expect("the preview was not drawn").rect;
        let view = crate::view::View::new(app.pattern_preview_camera, rect);
        for corner in [
            Vec3::new(lo.x, lo.y, lo.z),
            Vec3::new(hi.x, lo.y, lo.z),
            Vec3::new(lo.x, hi.y, lo.z),
            Vec3::new(hi.x, hi.y, lo.z),
            Vec3::new(lo.x, lo.y, hi.z),
            Vec3::new(hi.x, lo.y, hi.z),
            Vec3::new(lo.x, hi.y, hi.z),
            Vec3::new(hi.x, hi.y, hi.z),
        ] {
            let (at, _) = view.project(corner).expect("an orthographic projection always lands");
            assert!(rect.contains(at), "after Frame the corner {corner:?} is drawn at {at:?}, outside {rect:?}");
        }
    }

    /// The cross that drops a stage ends where the stage's own fields end, and
    /// is big enough to aim at.
    ///
    /// Asked for after it sat eight pixels right of every field under it -- a
    /// stage's name line ran to the edge of the column while a property row is
    /// pinned `EDGE_PAD` inside it -- and at egui's small-button size, 18.5 by
    /// 14, which is a small target for the one control in the tool that throws
    /// work away.
    ///
    /// Both edges are read back from the frame rather than worked out here:
    /// what is being held is that they go on coming from the same place.
    #[test]
    fn the_cross_that_drops_a_stage_lines_up_with_the_fields_under_it() {
        let mut app = headless_app();
        app.open_pattern_tool();
        let pattern = app.pattern_tool.expect("the tool opened on a pattern");
        // The cross is only on the last stage, and only when there is more than
        // one -- the others are what it repeats.
        if let Some(params) = app.scene.get_mut(pattern).and_then(|n| n.params_mut()) {
            params.insert("stages".to_string(), simple3d_core::primitive::ParamValue::Count(2));
        }
        app.reevaluate_for_test();

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| crate::pattern_tool::body(&mut app, ui));
        });

        let cross = ctx.read_response(crate::pattern_tool::drop_stage_id()).expect("the cross was not drawn").rect;
        let field = ctx
            .read_response(crate::panel_properties::grip_id("tool:2 Copies"))
            .expect("the stage's own field was not drawn")
            .rect;
        assert!(
            (cross.right() - field.right()).abs() < 0.5,
            "the cross ends at {} and the field under it at {}",
            cross.right(),
            field.right()
        );
        // Twice the fourteen pixels egui's small button comes out at, and
        // square: it is a mark rather than a word.
        assert!(
            cross.width() >= 28.0 && cross.height() >= 28.0,
            "the cross is {:?}, which is not twice the small button it was",
            cross.size()
        );
    }

    /// Reported: "the divider auto shrinks over time".
    ///
    /// It did, by eight pixels a frame, until it hit its own minimum. egui
    /// remembers a panel by the rectangle its *content* filled rather than by
    /// the panel's own, and a property row pins its right edge `EDGE_PAD` inside
    /// the room it is given -- so every frame handed the column back eight
    /// pixels narrower and it kept the smaller number.
    ///
    /// Nothing but running frames finds this: one frame is correct, and so is
    /// every part of it. Forty frames is what it takes.
    #[test]
    fn the_pattern_tools_divider_stays_where_it_is_put() {
        let mut app = headless_app();
        app.open_pattern_tool();
        app.reevaluate_for_test();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut seen = Vec::new();
        for _ in 0..40 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 640.0))),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| crate::pattern_tool::body(&mut app, ui));
            });
            seen.push(app.settings.pattern_stages_width);
        }
        assert_eq!(seen.first(), seen.last(), "the divider drifted: {seen:?}");
    }

    /// Reported: "the preview does not render anything".
    ///
    /// It rendered nothing because it was never drawn. With `embed_dialogs` on
    /// -- the setting that keeps the application off a second window system
    /// surface, and the way out of the freeze in issue 64 -- a dialog is an
    /// `egui::Window`, and the window was given no size, so it sized itself to
    /// what was in it. The tool's body lays itself out against the room it is
    /// *given*, and what it was given was the auto-sized default: 362 px, under
    /// what the two columns need, at which the tool gives the picture up and
    /// draws the numbers alone.
    ///
    /// The second half of the same fault is the height. The buttons were
    /// stacked under a body that fills whatever room it has, so once the window
    /// had a width worth having, it grew by the height of the button row every
    /// frame -- 907 px on an 880 px screen, with the buttons off the bottom of
    /// it.
    ///
    /// Asked of the frame rather than of the layout arithmetic: the headless
    /// context hands every dialog back as `Embedded`, so running `App::ui`
    /// against it draws the dialog the way that setting does, and the context
    /// is then asked where the preview and the window itself ended up.
    #[test]
    fn the_pattern_tools_preview_is_drawn_when_the_dialog_is_embedded() {
        let mut app = headless_app();
        app.open_pattern_tool();
        assert_eq!(app.modal, Modal::PatternKind, "the tool did not open, so this measures nothing");
        app.reevaluate_for_test();

        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 880.0));
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        // Several frames: a window that grows to fit its contents does it one
        // frame at a time, and one frame of it looks like a rounding error.
        for _ in 0..4 {
            let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            let _ = ctx.run(input, |ctx| app.ui(ctx));
        }

        let preview = ctx
            .read_response(crate::pattern_tool::preview_id())
            .expect("the preview was not drawn in the embedded dialog")
            .rect;
        assert!(
            preview.width() >= 220.0 && preview.height() >= 220.0,
            "the preview was drawn at {preview:?}, which is a stamp rather than a viewport"
        );
        let window = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("Custom pattern kind")))
            .expect("the embedded dialog was not drawn");
        assert!(
            screen.contains_rect(window),
            "the embedded dialog is {window:?}, which runs off the {screen:?} it is drawn on"
        );
    }

    #[test]
    fn the_pattern_tools_preview_turns_without_turning_the_viewport() {
        // The preview is a viewport now rather than a scatter of dots, and the
        // point of giving it a camera of its own is that turning the pattern
        // round to look at what a rule made must not move the view behind the
        // window -- which is where the shape is actually being modelled.
        let mut app = headless_app();
        app.open_pattern_tool();
        assert_eq!(app.modal, Modal::PatternKind, "the tool did not open, so this measures nothing");
        app.reevaluate_for_test();
        app.frame_pattern_preview(1.0);

        // It opens looking from where the viewport looks, so the picture in the
        // window and the one behind it agree about which way round the shape is.
        assert_eq!(app.pattern_preview_camera.yaw, app.scene.camera.yaw, "the preview opened at another angle");
        assert_eq!(app.pattern_preview_camera.pitch, app.scene.camera.pitch);

        let viewport_camera = app.scene.camera;
        let view = crate::view::View::new(app.pattern_preview_camera, app.viewport_rect);
        crate::panel_viewport::apply_gesture(
            &mut app.pattern_preview_camera,
            crate::panel_viewport::Gesture::Orbit,
            egui::vec2(60.0, 0.0),
            &view,
        );
        assert_ne!(app.pattern_preview_camera.yaw, viewport_camera.yaw, "the preview did not turn");
        assert_eq!(app.scene.camera, viewport_camera, "turning the preview turned the viewport with it");
    }

    /// The step vector a linear pattern is currently laid out along.
    fn linear_run(app: &App, pat: NodeId) -> Vec3 {
        use simple3d_core::primitive::ParamsExt;
        let params = app.scene.node(pat).params().unwrap();
        Vec3::new(params.num("step_x"), params.num("step_y"), params.num("step_z"))
    }

    /// The labels of the grips a pattern offers right now.
    fn grip_labels(app: &App, pat: NodeId) -> Vec<&'static str> {
        app.pattern_grips(pat).into_iter().map(|g| g.label).collect()
    }

    #[test]
    fn a_linear_patterns_spacing_is_laid_out_by_a_handle_that_follows_the_kind() {
        use simple3d_core::primitive::ParamValue;
        let free = gizmo::Mods { free: true, ..Default::default() };
        let mut app = headless_app();
        app.run(Command::Pattern);
        let pat = app.primary().unwrap();
        // A grip is placed in the node's own world frame, which comes from the
        // last evaluation, so a pattern has to have been evaluated once before it
        // offers any.
        app.reevaluate_for_test();
        // Linear by default: a grip for the spacing between copies, and one for
        // how many there are.
        assert_eq!(grip_labels(&app, pat), vec!["Spacing", "Copies"]);
        app.set_pattern_grip(pat, "Spacing", 84.0, free);
        // The spacing grip sits at the *last* copy, so the distance it is dragged
        // to is divided across the gaps: 84 over two gaps is a 42 mm step.
        assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(42.0)));
        app.reevaluate_for_test();
        draw_one_frame(&mut app);

        // A kind with nothing to lay out -- a mirror, which is a plane and two
        // copies -- offers no grips at all.
        app.scene.get_mut(pat).unwrap().params_mut().unwrap().insert("kind".into(), ParamValue::Choice(3));
        assert!(grip_labels(&app, pat).is_empty());

        // Negative drags cannot push the step below zero.
        app.scene.get_mut(pat).unwrap().params_mut().unwrap().insert("kind".into(), ParamValue::Choice(0));
        app.set_pattern_grip(pat, "Spacing", -5.0, free);
        assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(0.0)));
    }

    #[test]
    fn every_kind_that_places_copies_offers_grips_to_place_them_with() {
        // Issue 67: the pattern was to have "a creation tool of its own for
        // laying one out rather than only a list of numbers in the property
        // editor". Only the linear and grid kinds ever had a viewport handle;
        // the four others -- circular and mirror among the three the issue names,
        // and the helix and spiral it asks for besides -- had none at all.
        use simple3d_core::primitive::ParamValue;
        let mut app = headless_app();
        app.run(Command::Pattern);
        let pat = app.primary().unwrap();
        app.reevaluate_for_test();
        let wanted: [(u32, &[&str]); 6] = [
            (0, &["Spacing", "Copies"]),
            (1, &["Column spacing", "Columns", "Row spacing", "Rows", "Layers"]),
            (2, &["Radius", "Span"]),
            (3, &[]),
            (4, &["Radius", "Rise", "Copies", "Turn per copy"]),
            (5, &["Start radius", "Radius per copy", "Copies", "Turn per copy"]),
        ];
        for (kind, labels) in wanted {
            app.scene.get_mut(pat).unwrap().params_mut().unwrap().insert("kind".into(), ParamValue::Choice(kind));
            assert_eq!(grip_labels(&app, pat), labels.to_vec(), "kind {kind}");
            app.reevaluate_for_test();
            draw_one_frame(&mut app);
        }
    }

    #[test]
    fn dragging_the_copies_grip_lays_out_how_many_there_are() {
        // The other half of laying a pattern out by eye: the count follows the
        // pointer, at the spacing already set, instead of being typed.
        use simple3d_core::primitive::ParamsExt;
        let free = gizmo::Mods { free: true, ..Default::default() };
        let mut app = headless_app();
        app.run(Command::Pattern);
        let pat = app.primary().unwrap();
        app.reevaluate_for_test();
        let step = linear_run(&app, pat).length();
        assert!(step > 1e-9);

        // The grip is one step past the last copy, so dragging it to five steps
        // out asks for five copies.
        app.set_pattern_grip(pat, "Copies", step * 5.0, free);
        assert_eq!(app.scene.node(pat).params().unwrap().int("count"), 5);
        // A part-step lands on the nearer whole copy rather than on a fraction.
        app.set_pattern_grip(pat, "Copies", step * 2.4, free);
        assert_eq!(app.scene.node(pat).params().unwrap().int("count"), 2);
        // Dragged back past the origin it stops at one copy -- the original --
        // rather than at none or at a negative count.
        app.set_pattern_grip(pat, "Copies", -step * 4.0, free);
        assert_eq!(app.scene.node(pat).params().unwrap().int("count"), 1);
        // And the whole drag is one undo step, however many frames it took.
        app.reevaluate_for_test();
        draw_one_frame(&mut app);
    }

    #[test]
    fn a_rings_span_is_laid_out_by_carrying_its_grip_round() {
        // A ring has no outward run to drag copies along, so what lays it out is
        // its radius and the arc its copies fill.
        use simple3d_core::primitive::{ParamValue, ParamsExt};
        let free = gizmo::Mods { free: true, ..Default::default() };
        let mut app = headless_app();
        app.run(Command::Pattern);
        let pat = app.primary().unwrap();
        app.scene.get_mut(pat).unwrap().params_mut().unwrap().insert("kind".into(), ParamValue::Choice(2));
        app.reevaluate_for_test();

        app.set_pattern_grip(pat, "Radius", 45.0, free);
        assert!((app.scene.node(pat).params().unwrap().num("circ_radius") - 45.0).abs() < 1e-9);
        app.set_pattern_grip(pat, "Span", 90.0, free);
        assert!((app.scene.node(pat).params().unwrap().num("circ_span") - 90.0).abs() < 1e-9);
        // The span grip rides outside the copies, so a full turn does not put it
        // on top of the radius grip, where neither could be picked out.
        let grips = app.pattern_grips(pat);
        let radius = grips.iter().find(|g| g.label == "Radius").unwrap().at;
        let span = grips.iter().find(|g| g.label == "Span").unwrap().at;
        app.set_pattern_grip(pat, "Span", 360.0, free);
        assert!((radius - span).length() > 1.0, "the span grip sits on the radius grip");
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
    fn a_pattern_made_empty_is_sized_by_the_first_shape_added_into_it() {
        // The pattern *tool* measures the shapes it wraps, so Ctrl+Shift+P on a
        // 20 mm box gives a 30 mm step. A pattern made with nothing selected
        // has nothing to measure yet and kept the stock 20 mm, so a 20 mm shape
        // added into it afterwards was repeated at exactly its own width and
        // the copies fused into one bar instead of standing clear.
        let mut app = app_in(temp_config_dir("pattern-sized-on-add"));
        app.clear_selection();
        app.run(Command::Pattern);
        let pat = app.primary().expect("the pattern is selected");
        assert_eq!(app.scene.node(pat).params(), Some(&simple3d_core::pattern::default_params()));

        // Through the palette's own path -- a click on a tile with the empty
        // pattern selected -- rather than the outliner's row menu.
        app.add_node(Some("box"), GroupOp::Union);
        let params = app.scene.node(pat).params().expect("the pattern kept its parameters");
        assert_ne!(params, &simple3d_core::pattern::default_params(), "the pattern kept the stock 20 mm step");

        // And what it evaluates to is three shapes standing clear, not one bar.
        app.reevaluate_for_test();
        let (lo, hi) = app.evaluated.node_meshes[&pat].bounds().expect("the pattern evaluated to nothing");
        assert!(hi.x - lo.x > 60.0, "the copies fused: the run is only {} mm across", hi.x - lo.x);
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
        app.reevaluate_for_test();
        assert!((linear_run(&app, pat).length() - 50.0).abs() < 1e-9, "a 3-4-5 run");

        // Two gaps between three copies, so a grip dragged to 200 sets a run of
        // 100.
        app.set_pattern_grip(pat, "Spacing", 200.0, free);
        let step = linear_run(&app, pat);
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
        app.reevaluate_for_test();
        app.scene.settings.snap_step = 1.0;
        // Two gaps: the distance is rounded, and the step is what it divides to.
        app.set_pattern_grip(pat, "Spacing", 14.0718, gizmo::Mods::default());
        assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(7.0)));
        // Shift is the coarse step everywhere else, and here too.
        app.set_pattern_grip(pat, "Spacing", 44.0, gizmo::Mods { coarse: true, ..Default::default() });
        assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(20.0)));
    }

    #[test]
    fn an_empty_pattern_can_be_added_the_way_a_group_is() {
        // Both Add menus offer a pattern beside the group operators (issue 67).
        // It is the opposite gesture to Ctrl+Shift+P, which wraps the selection:
        // this always makes an empty one to fill afterwards, whatever is
        // selected.
        let mut app = headless_app();
        let plate = app.primary().unwrap();
        let root = app.scene.root();

        // From a plain row: beside it, not inside it, and left selected.
        app.add_pattern_at(plate);
        let beside = app.primary().unwrap();
        assert!(app.scene.node(beside).is_pattern());
        assert!(app.scene.node(beside).children.is_empty(), "it wrapped the selection instead of being empty");
        assert_eq!(app.scene.node(beside).parent, Some(root));

        // From a pattern's own row: into it, the same rule Add already follows.
        app.add_pattern_at(beside);
        let inside = app.primary().unwrap();
        assert_eq!(app.scene.node(inside).parent, Some(beside));

        // And the menu bar's Add, which uses the document's insertion point.
        app.select_only(plate);
        app.add_pattern();
        let added = app.primary().unwrap();
        assert!(app.scene.node(added).is_pattern());
        assert!(app.scene.node(added).children.is_empty());
        // One undo step takes it back, and the plate is untouched by all of it.
        app.run(Command::Undo);
        assert!(!app.scene.contains(added));
        assert!(app.scene.contains(plate));
    }

    #[test]
    fn a_pattern_made_empty_is_measured_the_moment_it_gains_a_shape() {
        // Issue 67, from the open list: the tool sizes the spacing to the shapes
        // it wraps, but a pattern made with nothing selected has nothing to
        // measure and kept the stock 20 mm step -- exactly the width of the stock
        // box -- so a box dropped into it afterwards was repeated face to face.
        use simple3d_core::primitive::ParamsExt;
        let mut app = headless_app();
        app.clear_selection();
        app.run(Command::Pattern);
        let pat = app.primary().unwrap();
        assert!(app.scene.node(pat).children.is_empty(), "the tool made a pattern with something in it");
        let stock = app.scene.node(pat).params().unwrap().num("step_x");

        app.add_node_at(pat, Some("box"), GroupOp::Union);
        let step = app.scene.node(pat).params().unwrap().num("step_x");
        let width = simple3d_core::eval::subtree_bounds(&app.scene, app.scene.node(pat).children[0])
            .map(|(lo, hi)| hi.x - lo.x)
            .unwrap();
        assert!(step > width, "a {width}mm shape is repeated at {step}mm, so its copies touch or overlap");
        assert!((step - stock).abs() > 1e-9, "the spacing was left at the stock number");
        app.reevaluate_for_test();
        assert!(app.evaluated.errors.is_empty(), "{:?}", app.evaluated.errors);

        // But numbers the user has settled are never overwritten: a second shape
        // dropped in leaves the spacing exactly as it stands.
        app.add_node_at(pat, Some("box"), GroupOp::Union);
        assert!((app.scene.node(pat).params().unwrap().num("step_x") - step).abs() < 1e-9);
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
            let step = linear_run(&app, pat);
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
    fn a_face_resize_snaps_the_face_it_pulls_onto_another_body() {
        // Issue 68 asked for snapping "during drags ... not only to the grid
        // increment it currently snaps to". A resize is a drag, and it snapped
        // only to the grid: pulling a plate out until it meets the block beside
        // it is the same gesture as sliding it there and wants the same answer.
        let mut app = app_in(temp_config_dir("snap-resize"));
        let root = app.scene.root();
        let a = app.scene.add_primitive("box", root, 0).unwrap();
        let b = app.scene.add_primitive("box", root, 1).unwrap();
        // Half a millimetre off the grid, so the grid step alone cannot reach it
        // and only a snap can put the two faces together.
        app.scene.get_mut(b).unwrap().position = Vec3::new(43.5, 0.0, 0.0);
        app.select_only(a);
        app.run(Command::ModeResize);
        app.history.clear();
        app.reevaluate_for_test();

        // A corner on B's near face, on the plane a plate pulled out along +X
        // has to reach. A corner rather than the face centre behind it, because
        // a snap takes only what the picture shows and that face is turned away
        // from the camera -- the same rule the measure tool follows.
        let (lo, hi) = app.evaluated.node_meshes[&b].bounds().unwrap();
        let face = Vec3::new(lo.x, lo.y, hi.z);

        app.settings.geometry_snap = SnapMode::Always;
        app.snap_requested = true;
        drag_gesture(&mut app, a, Handle::ResizeFace(0, true), face, 3);
        app.reevaluate_for_test();

        // A's +X face now sits exactly on B's -X face: the two bodies meet,
        // rather than stopping a millimetre short or running into each other.
        let (_, hi) = app.evaluated.node_meshes[&a].bounds().unwrap();
        assert!((hi.x - lo.x).abs() < 1e-6, "the pulled face landed at {} rather than on {}", hi.x, lo.x);
        // B's face is at 33.5, which a drag rounded to the 1 mm step can never
        // land on: reaching it is the snap and nothing else.
        assert!((hi.x - 33.5).abs() < 1e-6, "{hi:?}");
    }

    #[test]
    fn a_resize_without_snapping_asked_for_keeps_to_the_grid() {
        let mut app = app_in(temp_config_dir("snap-resize-off"));
        let root = app.scene.root();
        let a = app.scene.add_primitive("box", root, 0).unwrap();
        let b = app.scene.add_primitive("box", root, 1).unwrap();
        app.scene.get_mut(b).unwrap().position = Vec3::new(43.5, 0.0, 0.0);
        app.select_only(a);
        app.run(Command::ModeResize);
        app.history.clear();
        app.reevaluate_for_test();
        let (lo, _) = app.evaluated.node_meshes[&b].bounds().unwrap();

        app.settings.geometry_snap = SnapMode::Never;
        app.snap_requested = false;
        drag_gesture(&mut app, a, Handle::ResizeFace(0, true), Vec3::new(lo.x, 0.0, 0.0), 3);
        app.reevaluate_for_test();
        let (_, hi) = app.evaluated.node_meshes[&a].bounds().unwrap();
        assert!((hi.x - lo.x).abs() > 0.4, "the resize snapped to the face with snapping off: {hi:?}");
    }

    #[test]
    fn a_snapped_drag_never_catches_a_feature_round_the_back_of_its_own_body() {
        // The measure tool takes only what the picture shows; a drag did not,
        // and a corner on the far side of a solid projects into the middle of
        // the face in front of it. Aiming at that face made the body jump onto a
        // corner nothing on screen had drawn.
        let mut app = app_in(temp_config_dir("snap-hidden"));
        let root = app.scene.root();
        let a = app.scene.add_primitive("box", root, 0).unwrap();
        let b = app.scene.add_primitive("box", root, 1).unwrap();
        app.scene.get_mut(b).unwrap().position = Vec3::new(43.0, 0.0, 0.0);
        app.select_only(a);
        app.history.clear();
        app.reevaluate_for_test();
        let view = app.current_view();

        // Every catchable feature of B, and the ones the picture actually shows.
        let features = crate::snap::features_of(&app.evaluated.node_meshes[&b]);
        let hidden: Vec<Vec3> =
            features.iter().filter(|f| !app.in_clear_view(&view, f.point)).map(|f| f.point).collect();
        assert!(!hidden.is_empty(), "this view shows every corner of the box, so there is nothing to hide");

        // Pointing straight at a hidden corner catches something else, or
        // nothing -- never the corner behind the solid.
        for point in hidden {
            let cursor = view.project(point).unwrap().0;
            if let Some((caught, _)) = app.nearest_feature_excluding(&view, cursor, &[a]) {
                assert!(
                    (caught.point - point).length() > 1e-6,
                    "a drag caught {point:?}, which is round the back of the body"
                );
            }
        }
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

    // -- the bodies and commands issue 83 added ------------------------------

    #[test]
    fn converting_to_a_mesh_keeps_the_node_and_loses_the_recipe() {
        let mut app = headless_app();
        let root = app.scene.root();
        let group = app.scene.add_group(GroupOp::Difference, root, 0);
        app.scene.add_primitive("box", group, 0).unwrap();
        let hole = app.scene.add_primitive("cylinder", group, 1).unwrap();
        app.scene.get_mut(hole).unwrap().position = Vec3::new(4.0, 0.0, 0.0);
        app.scene.get_mut(group).unwrap().name = "Drilled".into();
        app.select_only(group);
        app.reevaluate_for_test();

        app.run(Command::ConvertToMesh);
        assert!(app.scene.node(group).is_mesh(), "it is still a group");
        assert_eq!(app.scene.node(group).name, "Drilled", "the node was replaced rather than converted");
        assert!(app.scene.node(group).children.is_empty(), "the operands outlived the body they made");
        assert!(app.scene.node(group).mesh().unwrap().triangle_count() > 0);
        assert_eq!(app.selection, vec![group], "the converted node should stay selected");

        // And it is one undo step that puts the whole thing back.
        app.run(Command::Undo);
        assert!(app.scene.node(group).is_group());
        assert_eq!(app.scene.node(group).children.len(), 2);
    }

    #[test]
    fn converting_something_that_is_already_a_mesh_says_so_rather_than_working() {
        let mut app = headless_app();
        let id = app.primary().unwrap();
        app.run(Command::ConvertToMesh);
        let before = app.history.undo_len();
        app.run(Command::ConvertToMesh);
        assert_eq!(app.history.undo_len(), before, "converting a mesh recorded an undo step");
        let _ = id;
    }

    #[test]
    fn joining_the_pieces_back_together_brings_the_original_object_back() {
        // The other half of issue 82: a split is reversible, and what comes back
        // is the recipe -- the difference with both its operands and their
        // parameters -- not the triangles the pieces are.
        let mut app = headless_app();
        let root = app.scene.root();
        let group = app.scene.add_group(GroupOp::Difference, root, 0);
        app.scene.get_mut(group).unwrap().name = "Cut plate".into();
        let plate = app.scene.add_primitive("box", group, 0).unwrap();
        app.scene.get_mut(plate).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(80.0));
        let knife = app.scene.add_primitive("box", group, 1).unwrap();
        {
            let node = app.scene.get_mut(knife).unwrap();
            node.params_mut().unwrap().insert("width".into(), ParamValue::Length(6.0));
            node.params_mut().unwrap().insert("depth".into(), ParamValue::Length(200.0));
            node.params_mut().unwrap().insert("height".into(), ParamValue::Length(200.0));
        }
        app.select_only(group);
        app.reevaluate_for_test();
        let before = app.evaluated.mesh.bounds().unwrap();

        split_with(&mut app, simple3d_geom::tiling::Tiling { size: 40.0, ..Default::default() });
        let split = app.primary().expect("the split is selected");
        assert!(app.scene.node(split).is_split());
        assert!(app.scene.node(split).children.len() >= 2, "the cut left the shape in one piece");

        app.run(Command::Rejoin);
        let back = app.primary().expect("the restored object is selected");
        assert_eq!(app.scene.node(back).name, "Cut plate");
        assert_eq!(app.scene.node(back).group_op(), Some(GroupOp::Difference), "it came back as something else");
        assert_eq!(app.scene.node(back).children.len(), 2, "the operands did not come back");
        let sizes: Vec<f64> =
            app.scene.node(back).children.iter().map(|&c| app.scene.node(c).params().unwrap().num("width")).collect();
        assert_eq!(sizes, vec![80.0, 6.0], "the operands came back with different dimensions");
        app.reevaluate_for_test();
        let after = app.evaluated.mesh.bounds().unwrap();
        assert!((before.0 - after.0).length() < 1e-3, "the shape moved: {before:?} -> {after:?}");
        assert!((before.1 - after.1).length() < 1e-3, "the shape moved: {before:?} -> {after:?}");
    }

    #[test]
    fn the_pieces_can_be_moved_as_one_and_joined_back_where_they_now_stand() {
        // Moving the split moves the object that comes out of it: the transform
        // belongs to the node standing in the tree, not to the recipe it holds.
        let mut app = headless_app();
        let root = app.scene.root();
        let group = app.scene.add_group(GroupOp::Union, root, 0);
        for i in 0..2 {
            let id = app.scene.add_primitive("box", group, i).unwrap();
            app.scene.get_mut(id).unwrap().position = Vec3::new(i as f64 * 60.0, 0.0, 0.0);
        }
        app.select_only(group);
        app.reevaluate_for_test();

        split_with(&mut app, simple3d_geom::tiling::Tiling { size: 40.0, ..Default::default() });
        let split = app.primary().unwrap();
        app.scene.get_mut(split).unwrap().position = Vec3::new(0.0, 0.0, 25.0);
        app.scene.get_mut(split).unwrap().name = "Renamed".into();
        app.reevaluate_for_test();
        let moved = app.evaluated.mesh.bounds().unwrap();

        app.run(Command::Rejoin);
        let back = app.primary().unwrap();
        assert_eq!(app.scene.node(back).position, Vec3::new(0.0, 0.0, 25.0), "the object went back to where it was");
        assert_eq!(app.scene.node(back).name, "Renamed", "the name the row now carries was not kept");
        app.reevaluate_for_test();
        let after = app.evaluated.mesh.bounds().unwrap();
        assert!((moved.0 - after.0).length() < 1e-3, "joining moved the shape: {moved:?} -> {after:?}");
        assert!((moved.1 - after.1).length() < 1e-3, "joining moved the shape: {moved:?} -> {after:?}");
    }

    /// Run a split the way the application does: open the tool, choose a
    /// pattern, press Split and wait for the thread to hand its pieces back.
    /// The cutting is on a thread precisely so the interface does not wait for
    /// it, so a test has to.
    fn split_with(app: &mut App, tiling: simple3d_geom::tiling::Tiling) {
        use simple3d_geom::tiling::SplitPlan;
        app.run(Command::SplitIntoPieces);
        app.split_tool.as_mut().expect("the tool opened on the selection").plan = SplitPlan::of(tiling);
        app.start_split();
        let deadline = Instant::now() + Duration::from_secs(60);
        while app.split_job.is_some() {
            app.poll_split();
            assert!(Instant::now() < deadline, "the split never finished");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn splitting_a_shape_cuts_it_into_a_piece_per_cell() {
        // The half of issue 82 that cuts rather than separates: the default
        // plate is 40 x 20, so 10 mm squares through it are eight pieces, and
        // the eight of them are the plate.
        let mut app = headless_app();
        let plate = app.primary().unwrap();
        app.scene.get_mut(plate).unwrap().name = "Deck".into();
        app.reevaluate_for_test();
        let before = app.evaluated.mesh.bounds().unwrap();

        split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });

        let split = app.primary().expect("the split is selected");
        assert!(app.scene.node(split).is_split(), "the pieces did not go under a split");
        assert_eq!(app.scene.node(split).name, "Deck", "the split is not named for what it was cut from");
        assert_eq!(app.scene.node(split).children.len(), 8, "a 40 x 20 plate in 10 mm squares is eight pieces");
        for &child in &app.scene.node(split).children {
            assert!(app.scene.node(child).is_mesh());
            assert!(app.scene.node(child).mesh().unwrap().triangle_count() > 0);
        }
        // The pattern is kept on the split, which is what lets the panel say
        // what was done and the tool open again on it.
        let tiling = app.scene.node(split).split_plan().expect("the pattern was not kept").first();
        assert_eq!(tiling.kind, simple3d_geom::tiling::CellKind::Squares);
        assert_eq!(tiling.size, 10.0);
        // And the pieces are exactly where the shape was.
        app.reevaluate_for_test();
        let after = app.evaluated.mesh.bounds().unwrap();
        assert!((before.0 - after.0).length() < 1e-3, "the pieces moved: {before:?} -> {after:?}");
        assert!((before.1 - after.1).length() < 1e-3, "the pieces moved: {before:?} -> {after:?}");
    }

    #[test]
    fn cutting_a_split_again_changes_the_pattern_rather_than_splitting_the_split() {
        // Opening the tool on a split offers the pattern it was cut with, and
        // cutting again replaces its pieces: the shape it was made from is
        // still the shape it was made from, so one Join back together is still
        // enough however many patterns have been tried.
        let mut app = headless_app();
        split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
        let split = app.primary().unwrap();
        assert_eq!(app.scene.node(split).children.len(), 8);

        app.run(Command::SplitIntoPieces);
        let offered = app.split_tool.as_ref().expect("the tool opened on the split").plan.first();
        assert_eq!(offered.size, 10.0, "the tool did not open on the pattern the split was cut with");
        app.cancel_split_tool();

        split_with(&mut app, simple3d_geom::tiling::Tiling { size: 20.0, ..Default::default() });
        let again = app.primary().unwrap();
        assert_eq!(again, split, "cutting again made a different node");
        assert!(app.scene.node(again).is_split());
        assert_eq!(app.scene.node(again).children.len(), 2, "a 40 x 20 plate in 20 mm squares is two pieces");
        for &child in &app.scene.node(again).children {
            assert!(app.scene.node(child).is_mesh(), "a piece of the old pattern was left behind");
        }
        let original = app.scene.node(again).split_original().expect("what it was cut from");
        assert_eq!(original.type_id, "plate", "cutting again lost the shape it was made from");

        app.run(Command::Rejoin);
        let back = app.primary().unwrap();
        assert_eq!(app.scene.node(back).params().unwrap().num("width"), 40.0, "the plate did not come back whole");
    }

    #[test]
    fn a_cell_bigger_than_the_shape_leaves_the_document_alone() {
        let mut app = headless_app();
        let before = app.history.undo_len();
        split_with(&mut app, simple3d_geom::tiling::Tiling { size: 500.0, ..Default::default() });
        assert_eq!(app.history.undo_len(), before, "a split that made one piece recorded an undo step");
        assert!(app.primary().is_some_and(|id| !app.scene.node(id).is_split()), "a one-piece split was made anyway");
        assert!(app.status_text().contains("one piece"), "{}", app.status_text());
    }

    #[test]
    fn a_split_dropped_because_the_shape_changed_under_it_leaves_the_document_alone() {
        // The cutting runs on a thread, so the shape it was cutting can be
        // edited before the pieces land. Pieces of a shape that no longer
        // exists are not an edit anybody asked for.
        let mut app = headless_app();
        let plate = app.primary().unwrap();
        app.run(Command::SplitIntoPieces);
        app.split_tool.as_mut().unwrap().plan =
            simple3d_geom::tiling::SplitPlan::of(simple3d_geom::tiling::Tiling { size: 5.0, ..Default::default() });
        app.start_split();
        app.scene.get_mut(plate).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(90.0));
        let before = app.history.undo_len();
        let deadline = Instant::now() + Duration::from_secs(60);
        while app.split_job.is_some() {
            app.poll_split();
            assert!(Instant::now() < deadline, "the split never finished");
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!app.scene.node(plate).is_split(), "pieces of the old shape were applied to the new one");
        assert_eq!(app.history.undo_len(), before, "a dropped split recorded an undo step");
        assert!(app.status_text().contains("changed while it was being cut"), "{}", app.status_text());
    }

    #[test]
    fn the_pattern_a_split_was_cut_with_survives_saving_and_loading() {
        let mut app = headless_app();
        split_with(
            &mut app,
            simple3d_geom::tiling::Tiling {
                kind: simple3d_geom::tiling::CellKind::Hexagons,
                size: 12.0,
                layer: 2.0,
                ..Default::default()
            },
        );
        let text = simple3d_core::project::to_string(&app.scene);
        let scene = simple3d_core::project::from_str(&text).expect("a scene with a cut-up shape loads");
        let split = scene.depth_first().into_iter().find(|&id| scene.node(id).is_split()).expect("the split is there");
        let tiling = scene.node(split).split_plan().expect("the pattern was not written to the file").first();
        assert_eq!(tiling.kind, simple3d_geom::tiling::CellKind::Hexagons);
        assert_eq!(tiling.size, 12.0);
        assert_eq!(tiling.layer, 2.0);
    }

    /// Two cuts at once, end to end and through the document: the pieces are
    /// what both grids leave, and the plan that made them is what the tool
    /// opens on again (issue 82).
    #[test]
    fn a_shape_is_cut_by_every_cut_of_the_plan_and_the_plan_is_kept() {
        use simple3d_geom::tiling::{SplitPlan, Tiling};
        let mut app = headless_app();
        // The starting plate is 40 x 20 x 4. Squares of 20 through Z are two
        // columns; slabs of 10 through X cut each of those in two across.
        let plan = SplitPlan {
            passes: vec![
                Tiling { size: 20.0, axis: 2, ..Tiling::default() },
                Tiling { size: 10.0, axis: 0, ..Tiling::default() },
            ],
        };
        app.run(Command::SplitIntoPieces);
        app.split_tool.as_mut().expect("the tool opened on the selection").plan = plan.clone();
        app.start_split();
        let deadline = Instant::now() + Duration::from_secs(60);
        while app.split_job.is_some() {
            app.poll_split();
            assert!(Instant::now() < deadline, "the split never finished");
            std::thread::sleep(Duration::from_millis(2));
        }

        let split = app.primary().expect("the split is selected");
        assert_eq!(app.scene.node(split).children.len(), 4, "two cuts across each other are four pieces");
        assert_eq!(app.scene.node(split).split_plan(), Some(&plan), "the plan the pieces were cut by was not kept");

        // Through the file, and back out again as the plan it was.
        let text = simple3d_core::project::to_string(&app.scene);
        let scene = simple3d_core::project::from_str(&text).expect("a scene cut by two cuts loads");
        let saved = scene.depth_first().into_iter().find(|&id| scene.node(id).is_split()).expect("the split is there");
        assert_eq!(scene.node(saved).split_plan(), Some(&plan));

        // And the tool opens on both cuts rather than on the first of them.
        app.run(Command::SplitIntoPieces);
        assert_eq!(app.split_tool.as_ref().expect("the tool opened on the split").plan, plan);
    }

    /// A piece is named after the shape it came out of and numbered in a series
    /// of its own (issue 82).
    ///
    /// They used to be "Plate 1" and up, which is the series the *objects* use:
    /// eighty pieces took eighty numbers out of it, and the next plate the user
    /// added came out as "Plate 81".
    #[test]
    fn pieces_are_named_apart_from_the_objects_they_came_from() {
        let mut app = headless_app();
        split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
        let split = app.primary().unwrap();
        let names: Vec<String> =
            app.scene.node(split).children.iter().map(|&c| app.scene.node(c).name.clone()).collect();
        let base = app.scene.node(split).name.clone();
        assert_eq!(names.first().map(String::as_str), Some(format!("{base} Piece 1").as_str()));
        assert_eq!(names.last().map(String::as_str), Some(format!("{base} Piece {}", names.len()).as_str()));

        // And the next object of that kind is the second one, not the ninth.
        let root = app.scene.root();
        let another = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
        assert_eq!(app.scene.node(another).name, format!("{base} 2"), "the pieces ate the objects' numbering");
    }

    #[test]
    fn a_split_is_one_row_in_the_outliner_however_many_pieces_it_holds() {
        // The whole of why a collection exists: a hexagon tiling over a plate is
        // thousands of pieces, and thousands of rows is a tree nobody can find
        // anything in (issue 82).
        let mut app = headless_app();
        split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
        let split = app.primary().unwrap();
        assert_eq!(app.scene.node(split).children.len(), 8);

        let rows = crate::panel_outliner::visible_rows(&app);
        assert!(rows.contains(&split), "the collection itself has no row");
        for &piece in &app.scene.node(split).children {
            assert!(!rows.contains(&piece), "a piece was drawn in the tree");
        }
    }

    #[test]
    fn ticked_pieces_are_extracted_into_rows_of_their_own() {
        let mut app = headless_app();
        split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
        let split = app.primary().unwrap();
        let pieces = app.scene.node(split).children.clone();

        // Nothing ticked is a warning rather than an edit.
        let before = app.history.undo_len();
        app.extract_ticked_pieces(split);
        assert_eq!(app.history.undo_len(), before, "extracting nothing recorded an undo step");

        app.tick_piece(pieces[0], false);
        app.tick_piece(pieces[3], true);
        app.extract_ticked_pieces(split);
        let rows = crate::panel_outliner::visible_rows(&app);
        assert!(rows.contains(&pieces[0]) && rows.contains(&pieces[3]), "the extracted pieces got no rows");
        assert!(!rows.contains(&pieces[1]), "a piece nobody asked for was extracted too");
        assert!(app.scene.is_collection(split), "extracting two of eight dissolved the collection");
        // The ticks survive the extraction, so Put back is the way straight
        // back: clearing them left that button greyed out the moment anything
        // had been extracted, and the way back was to find the same pieces in
        // the list and tick them again.
        assert_eq!(app.piece_ticks.len(), 2, "the extraction cleared the ticks");

        // And they fold back in, without having to be found again.
        app.return_ticked_pieces(split);
        let rows = crate::panel_outliner::visible_rows(&app);
        assert!(!rows.contains(&pieces[0]) && !rows.contains(&pieces[3]), "the pieces kept their rows");

        // One undo per step, and the first one puts both rows away again.
        app.run(Command::Undo);
        app.run(Command::Undo);
        assert!(app.scene.row_children(split).is_empty(), "undo left the pieces in the tree");
    }

    #[test]
    fn extracting_every_piece_asks_before_it_empties_the_collection() {
        // It is the one step that is not reversible by the feature itself: with
        // nothing left inside it the collection is a union group, and the shape
        // it was cut from goes with it (issue 82).
        let mut app = headless_app();
        split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
        let split = app.primary().unwrap();
        let pieces = app.scene.node(split).children.clone();

        // Ticking every one of them is the same question, however it is asked.
        for &piece in &pieces {
            app.tick_piece(piece, true);
        }
        let before = app.history.undo_len();
        app.extract_ticked_pieces(split);
        assert_eq!(app.modal, Modal::ConfirmExtractAll, "emptying the collection went through unasked");
        assert_eq!(app.history.undo_len(), before, "it edited the document before the question was answered");
        assert!(app.scene.is_collection(split));

        app.extract_all_pieces(split);
        assert!(!app.scene.is_collection(split), "the collection survived being emptied");
        assert_eq!(app.scene.node(split).group_op(), Some(GroupOp::Union), "what is left is not a union group");
        assert_eq!(app.scene.node(split).name, "Plate", "the group is not named what the collection was");
        let rows = crate::panel_outliner::visible_rows(&app);
        assert!(pieces.iter().all(|p| rows.contains(p)), "the pieces did not become ordinary rows");

        // And one undo puts the collection back, recipe and all.
        app.run(Command::Undo);
        assert!(app.scene.node(split).is_split());
        assert!(app.scene.node(split).split_original().is_some());
    }

    #[test]
    fn a_tick_belongs_to_the_collection_it_was_made_in() {
        // Ticks are not a selection, and they must not outlive the panel that
        // shows them: extracting into a collection the user has moved on from is
        // an edit somewhere they are not looking.
        let mut app = headless_app();
        split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
        let split = app.primary().unwrap();
        let piece = app.scene.node(split).children[0];
        app.tick_piece(piece, false);
        assert_eq!(app.listed_collection(), Some(split));

        let root = app.scene.root();
        app.select_only(root);
        assert!(app.piece_ticks.is_empty(), "the ticks survived the selection moving off the collection");
        assert_eq!(app.listed_collection(), None);
    }

    #[test]
    fn clicking_a_ticked_piece_again_unticks_it() {
        let mut app = headless_app();
        split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
        let split = app.primary().unwrap();
        let pieces = app.scene.node(split).children.clone();

        app.tick_piece(pieces[0], false);
        assert_eq!(app.piece_ticks.len(), 1);
        app.tick_piece(pieces[0], false);
        assert!(app.piece_ticks.is_empty(), "a second click on the same piece left it ticked");
        // A plain click replaces what was ticked; Ctrl adds to it.
        app.tick_piece(pieces[0], false);
        app.tick_piece(pieces[1], false);
        assert_eq!(app.piece_ticks.len(), 1, "a plain click added rather than replacing");
        app.tick_piece(pieces[2], true);
        assert_eq!(app.piece_ticks.len(), 2);
    }

    #[test]
    fn a_preview_only_reaches_the_viewport_while_a_tool_is_open() {
        // The setting says what the viewport does *while a preview is up*, and
        // nothing at all otherwise: it is not a second way to turn the grid off.
        let mut app = headless_app();
        app.scene.settings.preview_viewport = simple3d_core::scene::PreviewViewport::PreviewOnly;
        assert_eq!(app.preview_subject(), None, "something claims to be previewing with no tool open");

        let plate = app.primary().unwrap();
        app.reevaluate_for_test();
        app.open_split_tool();
        assert_eq!(app.preview_subject(), Some(plate));

        // A shape deleted under the tool takes the preview with it rather than
        // leaving the viewport emptied for an object that is gone.
        app.scene.remove(plate);
        assert_eq!(app.preview_subject(), None);
    }

    #[test]
    fn what_the_viewport_does_under_a_preview_is_saved_with_the_document() {
        // It is a document setting, not a preference: which of the grid, the
        // axes and the rest of the scene is in the way is a property of what is
        // being modelled (issue 82).
        use simple3d_core::scene::PreviewViewport;
        let mut app = headless_app();
        assert_eq!(app.scene.settings.preview_viewport, PreviewViewport::NoChange, "the default is not no change");
        // The default is absent from the file, so a project that never touched
        // this still diffs cleanly against one written before it existed.
        assert!(!simple3d_core::project::to_string(&app.scene).contains("preview_viewport"));

        app.scene.settings.preview_viewport = PreviewViewport::PreviewOnly;
        let text = simple3d_core::project::to_string(&app.scene);
        let back = simple3d_core::project::from_str(&text).expect("it loads");
        assert_eq!(back.settings.preview_viewport, PreviewViewport::PreviewOnly);
    }

    #[test]
    fn each_preview_mode_hides_exactly_what_it_names() {
        use simple3d_core::scene::PreviewViewport::*;
        // Read as a table, because the five are only ever right together: a
        // mode that hides one thing too many is a viewport with the ground gone
        // for no reason the user asked for.
        for (mode, grid, axes, others) in [
            (NoChange, true, true, true),
            (HideAxes, true, false, true),
            (HideGrid, false, true, true),
            (HideGridAndAxes, false, false, true),
            (PreviewOnly, false, false, false),
        ] {
            assert_eq!(mode.keeps_grid(), grid, "{mode:?} grid");
            assert_eq!(mode.keeps_axes(), axes, "{mode:?} axes");
            assert_eq!(mode.keeps_other_bodies(), others, "{mode:?} other bodies");
        }
    }

    #[test]
    fn the_previewed_object_has_a_renderable_even_when_it_is_not_selected() {
        // "Only what is previewed" draws that object as the model, so it needs
        // a renderable of its own -- and the selection can move on to something
        // else while the tool is open, which is what used to take it away.
        let mut app = headless_app();
        let plate = app.primary().unwrap();
        app.reevaluate_for_test();
        app.open_split_tool();
        let root = app.scene.root();
        app.select_only(root);
        app.refresh_node_renderables();
        assert!(app.node_renderables.contains_key(&plate), "the previewed object has nothing to draw");
    }

    #[test]
    fn the_split_tool_rebakes_when_the_shape_changes_under_it() {
        // The window is not modal, so the shape it is cutting can be edited
        // while it is open -- and a plan drawn over the shape as it was is a
        // plan of cuts that will not fall there (issue 82).
        let mut app = headless_app();
        let plate = app.primary().unwrap();
        app.reevaluate_for_test();
        app.open_split_tool();
        let was = app.split_tool.as_ref().expect("the tool opened").bounds;

        app.scene.get_mut(plate).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(120.0));
        app.reevaluate_for_test();
        app.refresh_split_tool();
        let now = app.split_tool.as_ref().expect("the tool is still open").bounds;
        assert!((now.1.x - was.1.x).abs() > 1.0, "the tool is still drawing the shape as it was: {was:?} -> {now:?}");

        // And a shape deleted under it closes the tool rather than leaving a
        // window open on nothing.
        app.scene.remove(plate);
        app.refresh_split_tool();
        assert!(app.split_tool.is_none(), "the tool stayed open on an object that is gone");
    }

    #[test]
    fn joining_something_that_was_never_split_says_so_rather_than_working() {
        let mut app = headless_app();
        let before = app.history.undo_len();
        app.run(Command::Rejoin);
        assert_eq!(app.history.undo_len(), before, "joining a shape that is not a split recorded an undo step");
        assert!(app.status_text().contains("split into pieces"), "{}", app.status_text());
    }

    #[test]
    fn a_split_survives_saving_and_loading_with_the_object_it_was_made_from() {
        // The split is a body of its own in the project file (format 3), and the
        // recipe it holds has to come back with it or the break stops being
        // reversible the moment the file is closed.
        let mut app = headless_app();
        let root = app.scene.root();
        let group = app.scene.add_group(GroupOp::Union, root, 0);
        for i in 0..2 {
            let id = app.scene.add_primitive("box", group, i).unwrap();
            app.scene.get_mut(id).unwrap().position = Vec3::new(i as f64 * 60.0, 0.0, 0.0);
        }
        app.select_only(group);
        app.reevaluate_for_test();
        split_with(&mut app, simple3d_geom::tiling::Tiling { size: 40.0, ..Default::default() });
        let pieces = app.scene.node(app.primary().unwrap()).children.len();

        let text = simple3d_core::project::to_string(&app.scene);
        let scene = simple3d_core::project::from_str(&text).expect("a scene with a split loads");
        let split = scene.depth_first().into_iter().find(|&id| scene.node(id).is_split()).expect("the split is there");
        assert_eq!(scene.node(split).children.len(), pieces, "the pieces did not survive the file");
        let original = scene.node(split).split_original().expect("the object it was made from");
        assert_eq!(original.type_id, "group");
        assert_eq!(original.children.len(), 2, "the operands were not written to the file");

        // And it still joins back together after the round trip.
        let mut reopened = headless_app();
        reopened.scene = scene;
        let restored = reopened.scene.restore_split(split).expect("the recipe rebuilds");
        assert_eq!(reopened.scene.node(restored).group_op(), Some(GroupOp::Union));
        assert_eq!(reopened.scene.node(restored).children.len(), 2);
    }
}
