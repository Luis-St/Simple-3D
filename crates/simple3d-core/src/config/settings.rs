//! Everything the application remembers between runs.

use super::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub window_size: [f32; 2],
    pub window_maximized: bool,
    pub outliner_width: f32,
    pub properties_width: f32,
    /// Which panel is in which dock, and what is rolled up. A user preference, not project data.
    pub layout: Layout,
    /// Suppress the view cube's camera transition and other self-moving animation.
    pub reduce_motion: bool,
    /// Draw dialogs inside the main window rather than as real windows (issue 53). Off by default.
    ///
    /// eframe renders every viewport on the winit thread, and on NVIDIA's Wayland EGL a second
    /// surface's swap can block forever, freezing the app; this avoids a second surface entirely.
    /// Older settings files read as off.
    #[serde(default)]
    pub embed_dialogs: bool,
    pub display_mode: DisplayMode,
    /// Which renderer draws the viewport; older settings files read as the CPU renderer.
    #[serde(default)]
    pub render_engine: RenderEngine,
    pub show_grid: bool,
    pub show_bounding_box: bool,
    /// Where a new shape lands.
    #[serde(default)]
    pub placement: Placement,
    /// When a drag snaps to other bodies' geometry (issue 68); older files read as "while a key is held".
    #[serde(default)]
    pub geometry_snap: SnapMode,
    /// Pin what the camera looks at, so pan and pointer zoom leave it alone. Older files read as unlocked.
    #[serde(default)]
    pub lock_view_centre: bool,
    /// Where a model opened while running goes (issue 107): a tab or its own window. Older files read
    /// as a tab.
    #[serde(default)]
    pub open_target: OpenTarget,
    /// Rotation snap in degrees; the move step is the project's `SceneSettings::snap_step`.
    pub rotate_snap_deg: f64,
    /// Recently applied colours, most recent first, offered beside the fixed palette.
    #[serde(default)]
    pub recent_colours: Vec<[u8; 3]>,
    pub last_export_dir: Option<PathBuf>,
    /// Where the last import was read from, separate from the export directory. Older files read as
    /// unset, and the dialog starts beside the open project.
    #[serde(default)]
    pub last_import_dir: Option<PathBuf>,
    pub last_export_format: String,
    pub last_export_scale: f64,
    /// Whether a 3MF is written compressed (issue 105). Missing fields default to on, so older
    /// settings files do not keep writing uncompressed packages.
    pub last_export_compress: bool,
    /// The last export's body mode, by `BodyMode::id`; absent means a single merged body.
    #[serde(default)]
    pub last_export_bodies: String,
    /// The last split plan (issue 82), so the tool opens on it; older files read as the default tiling.
    #[serde(default)]
    pub last_split: simple3d_geom::tiling::SplitPlan,
    /// The last simplify settings (issue 106); older files read as the default.
    #[serde(default)]
    pub last_simplify: simple3d_geom::simplify::Simplify,
    /// The last reassembly settings (issue 108); older files read as the default.
    #[serde(default)]
    pub last_reassemble: simple3d_geom::reassemble::Reassemble,
    pub recent_files: Vec<PathBuf>,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            window_size: [1400.0, 880.0],
            window_maximized: false,
            outliner_width: 260.0,
            properties_width: 320.0,
            layout: Layout::default(),
            reduce_motion: false,
            embed_dialogs: false,
            display_mode: DisplayMode::ShadedWithEdges,
            render_engine: RenderEngine::Cpu,
            show_grid: true,
            show_bounding_box: false,
            placement: Placement::Origin,
            geometry_snap: SnapMode::default(),
            lock_view_centre: false,
            open_target: OpenTarget::Tab,
            rotate_snap_deg: 15.0,
            recent_colours: Vec::new(),
            last_export_dir: None,
            last_import_dir: None,
            // 3MF by default, because it records units.
            last_export_format: "3mf".to_string(),
            last_export_scale: 1.0,
            last_export_bodies: "one".to_string(),
            last_export_compress: true,
            last_split: simple3d_geom::tiling::SplitPlan::default(),
            last_simplify: simple3d_geom::simplify::Simplify::default(),
            last_reassemble: simple3d_geom::reassemble::Reassemble::default(),
            recent_files: Vec::new(),
        }
    }
}

impl AppSettings {
    pub fn remember_recent(&mut self, path: &Path) {
        let path = path.to_path_buf();
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(MAX_RECENT);
    }

    /// Remember a just-applied colour, most recent first and without near-duplicates (issue 85).
    pub fn remember_colour(&mut self, colour: [u8; 3]) {
        self.recent_colours.retain(|c| !indistinguishable(*c, colour));
        self.recent_colours.insert(0, colour);
        self.recent_colours.truncate(MAX_RECENT_COLOURS);
    }

    pub fn forget_recent(&mut self, path: &Path) {
        self.recent_files.retain(|p| p != path);
    }
}

/// Whether two colours look the same (within 10 of 255 per channel), so they share a swatch
/// (issue 85).
pub fn indistinguishable(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| x.abs_diff(*y) <= COLOUR_TOLERANCE)
}

/// How far apart two colours must be to get two swatches.
pub(crate) const COLOUR_TOLERANCE: u8 = 10;
