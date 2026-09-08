//! Everything the running application holds, in one place.
//!
//! The methods that act on it are in the modules beside this one, grouped by
//! what they are for; the fields are here so the whole of the state can be
//! read at once.

use super::*;
use crate::gizmo::{Drag, Handle, Mode};
use crate::render::Renderable;
use crate::ui::{self, FieldBuffers};
use crate::view::CameraMove;
use crate::worker::{EvalWorker, ExportJob, SplitJob};
use simple3d_core::clipboard::Clip;
use simple3d_core::config::{AppSettings, Side};
use simple3d_core::eval::Evaluated;
use simple3d_core::keymap::{Chord, Command, Keymap};
use simple3d_core::library;
use simple3d_core::scene::{Camera, NodeId, Scene};
use simple3d_core::undo::History;
use simple3d_export::Format;
use simple3d_geom::Vec3;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

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
    pub(super) persisted_settings: AppSettings,
    /// When they were last written, so a scrubbed number -- which changes on
    /// every frame of the drag -- costs one write a moment rather than one a
    /// frame.
    pub(super) settings_written: Option<Instant>,
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
    pub(super) picker_colour: Option<[u8; 3]>,

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
    pub(super) snap_sources: Vec<Vec3>,
    /// Each body's snap features, kept between frames and keyed on the identity
    /// of the mesh they were found on. See `App::snaps_of`.
    pub(super) snap_features: std::cell::RefCell<std::collections::HashMap<NodeId, CachedSnaps>>,
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
    pub(super) config_dir: PathBuf,

    /// The message the fade clock is running for, so any assignment to `status`
    /// anywhere restarts it without having to remember to.
    pub(super) last_status: Status,
    /// The title the window is already wearing. `Context::send_viewport_cmd`
    /// requests a repaint for every command it is handed, so sending the title
    /// unconditionally each frame asked for the next frame each frame and the
    /// application never went idle. Only a title that changed is sent.
    pub(super) last_title: String,
    /// True while a run of held-down nudge keys is coalescing into one undo step.
    pub(super) nudging: bool,
    /// Set once a quit has been confirmed, so the event loop can close the window.
    pub(super) quit_now: bool,
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
