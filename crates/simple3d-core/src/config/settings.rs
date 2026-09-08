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
    /// How wide the stages column of the custom pattern tool is, which its own
    /// divider drags. Beside the dock widths for the same reason they are here:
    /// it is how one person likes their window and not a property of any model.
    pub pattern_stages_width: f32,
    /// Which panel is in which dock, and what is rolled up. Kept here beside the
    /// dock widths rather than in the project file: it is how one person likes
    /// their window, not a property of the model, and putting it in the document
    /// would mean opening someone else's file rearranged your workspace.
    pub layout: Layout,
    /// Suppress the view cube's camera transition and anything else that moves
    /// on its own.
    pub reduce_motion: bool,
    /// Draw the dialogs inside the main window rather than as windows of the
    /// window system's own.
    ///
    /// Off by default: a dialog is a real window (issue 53), which is what
    /// gives it the desktop's own frame, its place in the window list and the
    /// keyboard handling that goes with those. The switch exists for the one
    /// configuration where a real window is a hazard. eframe renders every
    /// viewport on the winit thread -- `glow_integration` spawns none -- so a
    /// dialog surface whose `eglSwapBuffers` blocks takes the whole application
    /// with it, and on NVIDIA's Wayland EGL a swap waits for a frame callback
    /// the compositor may never send for a second toplevel. Turning this on
    /// means there is no second surface to block on, which is a stronger
    /// guarantee than swap interval 0 and the only one that does not depend on
    /// the driver honouring anything. Newer than the settings file, so an older
    /// one reads as off.
    #[serde(default)]
    pub embed_dialogs: bool,
    pub display_mode: DisplayMode,
    /// Which renderer draws the viewport. Newer than the settings file, so an
    /// older one is read as the CPU renderer -- which is what it was using.
    #[serde(default)]
    pub render_engine: RenderEngine,
    pub show_grid: bool,
    pub show_bounding_box: bool,
    pub handle_frame: HandleFrame,
    /// Where a new shape lands.
    #[serde(default)]
    pub placement: Placement,
    /// When a drag snaps to the geometry of other bodies (issue 68). Newer than
    /// the settings file, so an older one reads as "while a key is held".
    #[serde(default)]
    pub geometry_snap: SnapMode,
    /// Pin what the camera looks at. Pan and the wheel's zoom-about-the-pointer
    /// both move that point, and a view that is being orbited about one place
    /// should not drift off it; locked, the fields that show it are read-only
    /// and the gestures that would move it leave it alone. Newer than the
    /// settings file, so an older one reads as unlocked.
    #[serde(default)]
    pub lock_view_centre: bool,
    /// Rotation snap in degrees. The move and resize step is `SceneSettings`'s
    /// `snap_step`, a project setting rather than a user one.
    pub rotate_snap_deg: f64,
    /// The colours most recently applied, most recent first. Offered beside the
    /// fixed palette wherever a colour can be chosen: the colour a project is
    /// actually painted in is nearly always one already used somewhere else in
    /// it, and finding it again in a picker is a worse job than it looks.
    #[serde(default)]
    pub recent_colours: Vec<[u8; 3]>,
    pub last_export_dir: Option<PathBuf>,
    pub last_export_format: String,
    pub last_export_scale: f64,
    /// How the last export decided its bodies, by `BodyMode::id`. Newer than
    /// the settings file, so absent means the single merged body an export
    /// always used to write.
    #[serde(default)]
    pub last_export_bodies: String,
    /// How a shape was last cut into pieces (issue 82), so the tool opens on
    /// the pattern the user is working in rather than on the default every
    /// time. Beside the export's own last-used settings, and here rather than
    /// in the project for the same reason: it is how one person is working, not
    /// a property of any one model. Newer than the settings file, so an older
    /// one reads as the default tiling -- and a settings file from before a
    /// split could be cut more than once reads as the one cut it holds.
    #[serde(default)]
    pub last_split: simple3d_geom::tiling::SplitPlan,
    pub recent_files: Vec<PathBuf>,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            window_size: [1400.0, 880.0],
            window_maximized: false,
            outliner_width: 260.0,
            properties_width: 320.0,
            pattern_stages_width: 360.0,
            layout: Layout::default(),
            reduce_motion: false,
            embed_dialogs: false,
            display_mode: DisplayMode::ShadedWithEdges,
            render_engine: RenderEngine::Cpu,
            show_grid: true,
            show_bounding_box: false,
            handle_frame: HandleFrame::Object,
            placement: Placement::Origin,
            geometry_snap: SnapMode::default(),
            lock_view_centre: false,
            rotate_snap_deg: 15.0,
            recent_colours: Vec::new(),
            last_export_dir: None,
            // 3MF by default, because it records units.
            last_export_format: "3mf".to_string(),
            last_export_scale: 1.0,
            last_export_bodies: "one".to_string(),
            last_split: simple3d_geom::tiling::SplitPlan::default(),
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

    /// Remember a colour that was just applied, most recent first and without
    /// duplicates, so the row of recent swatches stays short enough to scan.
    ///
    /// A shade of one already on the row counts as that one: the row is eight
    /// colours to click, and two swatches nobody can tell apart are one choice
    /// and a wasted slot (issue 85).
    pub fn remember_colour(&mut self, colour: [u8; 3]) {
        self.recent_colours.retain(|c| !indistinguishable(*c, colour));
        self.recent_colours.insert(0, colour);
        self.recent_colours.truncate(MAX_RECENT_COLOURS);
    }

    pub fn forget_recent(&mut self, path: &Path) {
        self.recent_files.retain(|p| p != path);
    }
}

/// Whether two colours are the same colour to look at, and so the same entry on
/// a row of recent swatches (issue 85).
///
/// Ten of 255 on every channel: a twenty-fifth of the range, which is a shade of
/// a colour rather than another colour. The point is not exactness -- it is that
/// a row offering eight swatches nobody can tell apart offers one thing eight
/// times.
pub fn indistinguishable(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| x.abs_diff(*y) <= COLOUR_TOLERANCE)
}

/// How far apart two colours have to be to be worth two swatches.
pub(crate) const COLOUR_TOLERANCE: u8 = 10;
