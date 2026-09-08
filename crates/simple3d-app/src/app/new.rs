//! Starting the application up, and the scene it opens on.

use super::*;
use crate::gizmo::Mode;
use crate::render::Renderable;
use crate::ui::{self, FieldBuffers};
use crate::worker::EvalWorker;
use simple3d_core::config::{self};
use simple3d_core::scene::{Camera, Scene};
use simple3d_core::undo::History;
use simple3d_export::Format;
use std::collections::BTreeMap;
use std::path::PathBuf;

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
}
