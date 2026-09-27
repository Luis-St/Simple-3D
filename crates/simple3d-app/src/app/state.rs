//! All application state in one place; the methods acting on it live in the sibling modules.

use super::*;
use crate::gizmo::{Drag, Handle, Mode};
use crate::render::Renderable;
use crate::ui::{self, FieldBuffers};
use crate::view::CameraMove;
use crate::worker::{EvalWorker, ExportJob, ImportJob, SplitJob};
use simple3d_core::clipboard::Clip;
use simple3d_core::config::{AppSettings, Side};
use simple3d_core::eval::Evaluated;
use simple3d_core::keymap::{Chord, Command, Keymap};
use simple3d_core::library;
use simple3d_core::scene::{NodeId, Scene};
use simple3d_core::undo::History;
use simple3d_export::Format;
use simple3d_geom::Vec3;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

pub struct App {
    pub scene: Scene,
    pub history: History,
    /// Every open document, one per tab (issue 61). The entry at `active` is a stand-in: the live
    /// document is held on this struct and written back on tab switch (`crate::tabs`).
    pub tabs: Vec<crate::tabs::Document>,
    pub active: usize,
    /// The project's other components and which are open (issue 113); see `crate::components`.
    pub project: crate::components::Project,
    /// A pending component question: an undo that would remove a component, or a deletion.
    pub(crate) component_ask: Option<crate::components::ComponentAsk>,
    /// This window's id (issue 107), assigned by the shell and never reused.
    pub window_id: u64,
    /// Whether this is the root viewport's window, the only one that records geometry and writes
    /// settings.
    pub(crate) root_window: bool,
    /// What this window asks the shell to do after the frame; windows never act on each other
    /// directly (see [`crate::shell`]).
    pub(crate) window_request: Option<crate::shell::WindowRequest>,
    /// The other windows as the shell last saw them, as tab send and drop targets.
    pub(crate) other_windows: Vec<crate::shell::OtherWindow>,
    /// Whether any other window has unsaved changes, so the quit question covers the application.
    pub(crate) unsaved_elsewhere: bool,
    /// This window's desktop rect; `None` on Wayland, where tabs cannot be dropped onto it.
    pub(crate) window_rect: Option<egui::Rect>,
    /// The tab row's rect, to tell a tab dragged along it from one pulled off.
    pub(crate) strip_rect: egui::Rect,
    /// The tab being dragged, if any (issue 107).
    pub tab_drag: Option<crate::tabs::TabDrag>,
    /// Whether the pointer was on the tab row last frame; how a window claims a tab dropped
    /// over it (see `crate::shell`).
    pub(crate) pointer_on_strip: bool,
    pub settings: AppSettings,
    /// The settings as on disk, so changes can be detected and written.
    pub(super) persisted_settings: AppSettings,
    /// When settings were last written, throttling writes during a scrub.
    pub(super) settings_written: Option<Instant>,
    pub keymap: Keymap,

    /// The selection in click order; the last entry is the primary one.
    pub selection: Vec<NodeId>,
    /// Collection pieces ticked in its panel (issue 82). Kept out of `selection` deliberately:
    /// selecting a piece would swap the panel away from the list. Cleared when the selection leaves
    /// the collection.
    pub(crate) piece_ticks: std::collections::BTreeSet<NodeId>,
    /// The anchor for Shift+click range selection: the last row clicked without Shift (issue 60).
    pub(crate) selection_anchor: Option<NodeId>,
    /// The row the last outliner click landed on. egui detects double clicks by timing alone, so
    /// a rename checks both clicks were on the same row (issue 59).
    pub(crate) outliner_last_click: Option<NodeId>,
    pub clipboard: Option<Clip>,
    /// Text for the desktop clipboard on the next frame. `egui-winit` only reports Ctrl+V when the
    /// system clipboard holds text, so copying must write something there.
    pub(crate) clipboard_text: Option<String>,
    /// The file dialog in flight, if any. See [`FilePrompt`].
    pub(crate) file_prompt: Option<FilePrompt>,

    pub worker: EvalWorker,
    pub evaluated: Evaluated,
    /// Bumped per new evaluation, so caches know to rebuild.
    pub evaluation_generation: u64,
    /// Frame the scene on the first evaluation with bounds. Never set for a loaded project, whose
    /// saved camera must be kept.
    pub(crate) frame_when_evaluated: bool,
    pub(crate) dirty: bool,

    pub scene_renderable: Renderable,
    pub node_renderables: BTreeMap<NodeId, std::sync::Arc<Renderable>>,
    pub(crate) renderable_key: u64,

    pub mode: Mode,
    pub drag: Option<Drag>,
    /// A body released from a GPU-drawn drag, drawn at its drop position until its evaluation
    /// arrives (`App::live_move`); otherwise it jumped back meanwhile.
    pub(crate) settling: Option<NodeId>,
    pub hover_handle: Option<Handle>,
    /// The handle under the pointer at button press, which is what starts a drag.
    pub grabbed: Option<Handle>,
    pub viewport_rect: egui::Rect,
    pub texture: Option<egui::TextureHandle>,
    pub image_key: u64,
    /// The OpenGL context, if any. `None` under the test harness, so tests use the software path.
    pub(crate) gl: Option<std::sync::Arc<eframe::glow::Context>>,
    /// The GPU renderer, built on first use.
    pub(crate) gpu: Option<crate::gpu::Gpu>,
    /// Why the GPU renderer is unavailable, shown beside the engine picker; the CPU is used instead.
    pub(crate) gpu_error: Option<String>,
    /// What the GPU renderer drew this frame.
    pub(crate) gpu_texture: Option<egui::TextureId>,

    pub path: Option<PathBuf>,
    pub(crate) saved_revision: u64,

    /// The colour the open picker has reached, added to the recent row only once it closes
    /// (issue 85), so a drag through the picker is one choice.
    pub(super) picker_colour: Option<[u8; 3]>,

    pub status: Status,
    /// When the current message was set, so it can fade.
    pub status_at: std::time::Instant,
    pub fields: FieldBuffers,
    /// The field label being scrubbed, held here so it survives relayout.
    pub scrub: crate::ui::Scrub,
    pub rename: Option<(NodeId, String)>,
    /// Groups collapsed in the outliner, held here so it survives relayout and can be opened from
    /// the viewport.
    pub collapsed: std::collections::HashSet<NodeId>,
    pub outliner_drag: Option<Carried>,
    pub drop_target: Option<DropTarget>,

    /// The 3D cursor where new shapes land; `None` means the origin.
    pub cursor: Option<Vec3>,
    /// The measure tool (issue 69).
    pub measure: Measure,
    /// Whether snapping is requested this frame (issue 68).
    pub(crate) snap_requested: bool,
    /// The feature the current drag is snapped onto, for the viewport to mark.
    pub snap_indicator: Option<Vec3>,
    /// The plane's offset from the grab point while its grip is dragged (issue 71).
    pub section_grab: Option<f64>,
    /// The section and grip under the pointer, for hover feedback and travel arrows (issue 72).
    pub section_hover: Option<(usize, usize)>,
    /// The section the window is showing and editing.
    pub section_tab: usize,
    /// Whether each section plane cuts the still part of the model, keyed by scene and plane (see
    /// [`crate::section_tool::cut`]); cached since it walks every vertex.
    pub(crate) section_crossing: Vec<(u64, bool)>,
    /// Snap features of the dragged body as offsets from its origin (`App::drag_snap_sources`).
    pub(super) snap_sources: Option<SnapSources>,
    /// Shapes a dragged boolean is drawn from, with their evaluation and whether they have edges
    /// (`App::live_csg`).
    #[allow(clippy::type_complexity)]
    pub(super) csg_leaves:
        std::cell::RefCell<std::collections::HashMap<NodeId, (u64, bool, std::sync::Arc<Renderable>)>>,
    /// Hull data for dragged booleans (`App::csg_hull`).
    pub(super) csg_hulls: std::cell::RefCell<std::collections::HashMap<NodeId, HullCache>>,
    /// Each body's snap features, keyed by mesh identity. See `App::snaps_of`.
    pub(super) snap_features: std::cell::RefCell<std::collections::HashMap<NodeId, CachedSnaps>>,
    /// Snap features being found off the interface thread (`App::warm_snaps`).
    pub(super) snap_warming: Option<SnapWarming>,
    /// A deletion awaiting the outliner's confirmation.
    pub pending_delete: Option<Vec<NodeId>>,
    /// A view change in flight, written into the scene's camera each frame.
    pub camera_move: Option<CameraMove>,
    /// The orientation cube turned by hand; `None` means it follows the camera.
    pub cube_spin: Option<CubeSpin>,
    /// Which panel header is being dragged between docks, and where to.
    pub dock_drag: crate::dock::DockDrag,
    /// Header centres and each dock's rect, collected while docks draw.
    pub dock_headers: Vec<(Side, Vec<f32>)>,
    pub dock_rects: Vec<(Side, egui::Rect)>,

    pub export_job: Option<ExportJob>,
    /// The file being imported on its own thread (issue 105), with the tab it is for.
    pub import_job: Option<ImportJob>,
    /// The split tool's window and its cutting job (issue 82); see [`crate::split_tool`].
    pub split_tool: Option<crate::split_tool::SplitTool>,
    pub split_job: Option<SplitJob>,
    /// The simplify tool's window (issue 106). Its result goes into the document live, so it keeps
    /// the original mesh for Cancel. See [`crate::simplify_tool`].
    pub simplify_tool: Option<crate::simplify_tool::SimplifyTool>,
    /// The reassembly tool's window (issue 108). Its subtree only enters the document on accept, to
    /// avoid rebuilding the outliner live. See [`crate::reassemble_tool`].
    pub reassemble_tool: Option<crate::reassemble_tool::ReassembleTool>,
    /// The saved pattern kind a delete confirmation is about (issue 67).
    pub(crate) confirm_delete_kind: Option<simple3d_core::pattern_library::Entry>,
    /// The collection an extract-all confirmation is about (issue 82).
    pub(crate) confirm_extract: Option<NodeId>,
    /// Position and roll-up state of each in-place popup, by name (issue 82).
    pub(crate) popups: std::collections::HashMap<&'static str, crate::popup::Placement>,
    pub export_format: Format,
    pub export_scale: String,
    pub export_selection_only: bool,
    /// What the objects of an export are (issue 58); offered only for 3MF.
    pub export_bodies: simple3d_export::BodyMode,
    /// Whether a 3MF package is compressed (issue 105).
    pub export_compress: bool,
    /// The export dialog's last count and what it was for, since counting evaluates off-thread.
    pub(crate) export_preview: ExportPreview,

    pub modal: Modal,
    /// The dialog already centred over the main window; it is centred only once.
    pub(crate) dialog_placed: Option<egui::ViewportId>,
    /// The tab a close confirmation is about.
    pub(crate) pending_close: Option<usize>,
    pub error_title: String,
    pub error_detail: String,

    /// The subtree waiting to be saved to the library, and its name.
    pub primitive_clip: Option<Clip>,
    pub primitive_name: String,
    /// The library as last read from disk, re-read only on change.
    pub library: Vec<library::Entry>,

    /// The pattern the custom-kind tool is editing directly (issue 67).
    pub pattern_tool: Option<NodeId>,
    /// The name the rule would be saved under.
    pub pattern_tool_name: String,
    /// Whether the rule has been started (issue 79); until then the tool asks what to start from.
    pub(crate) pattern_tool_started: bool,
    /// The saved kinds, read when the tool opens rather than every frame.
    pub pattern_kinds: Vec<simple3d_core::pattern_library::Entry>,
    /// The pattern whose scatter window is open (issue 79); one at a time.
    pub(crate) noise_popup: Option<NodeId>,
    /// Which tool stages are folded (issue 79).
    pub(crate) pattern_tool_folded: [bool; simple3d_core::pattern::MAX_STAGES],
    /// Scatter parts shown despite being zero, so a part does not vanish while being typed into
    /// (issue 79). For one pattern at a time.
    pub(crate) noise_parts_open: (Option<NodeId>, [bool; crate::noise_popup::Part::COUNT]),
    /// The hovered stage, whose copies the viewport marks.
    pub(crate) pattern_tool_hover: Option<usize>,
    /// Whether saving the rule keeps the pattern's scatter.
    pub(crate) pattern_tool_keep_noise: bool,
    /// Whether the start question was reopened over an existing rule that can be resumed.
    pub(crate) pattern_tool_resumable: bool,

    pub keymap_search: String,
    pub recording: Option<Command>,
    pub keymap_conflict: Option<(Command, Chord, Command)>,
    /// Modifier-only hold tracking (issue 77), since the toolkit reports no key event for them.
    /// Separate states for shortcuts and the keymap recorder.
    pub shortcut_mods: ui::ChordHold,
    pub record_mods: ui::ChordHold,

    /// Where settings and the keymap are read and written; held so tests can use a temp directory.
    pub(super) config_dir: PathBuf,

    /// The message the fade clock runs for, so any change to `status` restarts it.
    pub(super) last_status: Status,
    /// The current window title. Sent only on change, since every viewport command requests a
    /// repaint and the app would never go idle.
    pub(super) last_title: String,
    /// True while held nudge keys coalesce into one undo step.
    pub(super) nudging: bool,
    /// Set once a quit has been confirmed.
    pub(super) quit_now: bool,
    /// Set once closing this window has been confirmed (issue 107).
    pub(super) close_now: bool,
    /// Set once the shell has been told to end the run, so the next close is let through. Without
    /// it the close-button handler cancels the quit's own close.
    pub(crate) leaving: bool,
    /// Whether the unsaved-changes question is about closing this window rather than quitting.
    pub(crate) closing_window: bool,
}

/// The orientation cube turned on its own; stale once `camera` moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubeSpin {
    pub yaw: f64,
    pub pitch: f64,
    pub camera: (f64, f64),
}
