//! An icon as a button.

use super::*;
use crate::theme::token;
use egui::Vec2;

/// An icon-only button, the tool rail's unit; `active` fills it with the accent so the current tool
/// is unmistakable.
pub fn button(ui: &mut egui::Ui, glyph: Glyph, size: f32, active: bool, enabled: bool) -> egui::Response {
    let id = ui.next_auto_id();
    button_sensing(ui, id, glyph, size, active, enabled, egui::Sense::click())
}

/// The same, also a drag source (palette tiles). A disabled one still senses hover to explain itself;
/// `id` lets tests find a shape's tile.
pub fn button_sensing(
    ui: &mut egui::Ui,
    id: egui::Id,
    glyph: Glyph,
    size: f32,
    active: bool,
    enabled: bool,
    sense: egui::Sense,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let response = ui.interact(rect, id, if enabled { sense } else { egui::Sense::hover() });
    let painter = ui.painter();
    let radius = egui::CornerRadius::same(3);
    let colour = if !enabled {
        token::TEXT_LO.gamma_multiply(0.45)
    } else if active {
        painter.rect_filled(rect, radius, token::ACCENT);
        token::SURFACE_0
    } else if response.hovered() {
        painter.rect_filled(rect, radius, token::SURFACE_3);
        token::TEXT_HI
    } else {
        token::TEXT_LO
    };
    draw(painter, rect.shrink(size * 0.22), glyph, colour);
    response
}

/// The app icon rasterised once and shared by every dialog window.
pub fn shared_icon() -> std::sync::Arc<egui::IconData> {
    static ICON: std::sync::OnceLock<std::sync::Arc<egui::IconData>> = std::sync::OnceLock::new();
    ICON.get_or_init(|| std::sync::Arc::new(app_icon(256))).clone()
}
