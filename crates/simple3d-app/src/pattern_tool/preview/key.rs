//! What the preview was drawn from, so it is not drawn again for nothing.

use crate::app::App;

/// Everything the preview's image is drawn from. Unchanged, and the last one is
/// still what should be on screen.
pub(crate) fn preview_key(app: &App, size: [usize; 2], dark: bool) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    size.hash(&mut hasher);
    dark.hash(&mut hasher);
    app.evaluation_generation.hash(&mut hasher);
    app.renderable_key.hash(&mut hasher);
    (app.settings.display_mode as u8).hash(&mut hasher);
    app.scene.settings.grid_visible.hash(&mut hasher);
    app.scene.settings.grid_spacing.to_bits().hash(&mut hasher);
    app.scene.settings.axes_visible.hash(&mut hasher);
    app.scene.settings.axis_style.hash(&mut hasher);
    app.scene.settings.plane_marks.hash(&mut hasher);
    crate::section_tool::hash_section(&app.scene.settings.section, &mut hasher);
    let camera = app.pattern_preview_camera;
    for value in
        [camera.target.x, camera.target.y, camera.target.z, camera.distance, camera.yaw, camera.pitch, camera.fov_deg]
    {
        value.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}
