//! The window's own title bar: the buttons, the drag, and the resize edges.
//!
//! Wayland compositors in the GNOME lineage draw nothing for a client, so a
//! window has a title bar only if it draws one. winit will draw an Adwaita
//! imitation, but it is a rough one -- a thin flat strip with small square
//! glyphs -- and it sits badly beside the real thing. So the decorations are
//! off and this module draws the bar instead, which also means the buttons can
//! be made to look like the ones the host desktop uses rather than like a
//! third convention belonging to neither.
//!
//! What "native" means is different on each desktop, and the difference is not
//! decoration for its own sake: it is where the eye expects to click. GNOME
//! puts round buttons with gaps between them at the end of a tall bar; Windows
//! puts flat full-height rectangles hard against the corner, with the close
//! button turning red. Both are here, chosen at compile time.

use crate::theme::{self, token};
use eframe::egui;

/// Which desktop's conventions to follow. Compile-time: a binary is built for
/// one platform and the two conventions do not mix on one machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Chrome {
    /// GNOME and the rest of the Adwaita lineage.
    Gnome,
    /// Windows 10 and 11.
    Windows,
}

pub(crate) const CHROME: Chrome = if cfg!(target_os = "windows") { Chrome::Windows } else { Chrome::Gnome };

/// How tall the bar is. GNOME header bars are tall enough to hold controls;
/// the Windows caption is the height of its buttons and no more.
pub(crate) const fn bar_height() -> f32 {
    match CHROME {
        Chrome::Gnome => 40.0,
        Chrome::Windows => 32.0,
    }
}

/// How far the window's corners are rounded. GNOME rounds every window it
/// shows; a square-cornered one reads as a screenshot pasted onto the desktop.
/// Windows leaves its corners to the compositor, which rounds them itself.
pub(crate) const fn corner_radius() -> u8 {
    match CHROME {
        Chrome::Gnome => 12,
        Chrome::Windows => 0,
    }
}

/// Whether the window asks to be transparent. Only the corners need it -- what
/// is rounded away has to show the desktop rather than black -- so it is turned
/// on exactly where corners are rounded.
pub(crate) const fn wants_transparency() -> bool {
    corner_radius() > 0
}

/// The grab band along the window's edges, in points. Wider than it looks:
/// hitting a one-pixel border with a pointer is a game, not an interface.
const RESIZE_BAND: f32 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Button {
    Minimise,
    Maximise,
    Close,
}

/// The window buttons, laid out from the right in the host desktop's order and
/// shape. Returns the width they took, so the caller knows how much of the bar
/// is spoken for.
pub(crate) fn window_buttons(ui: &mut egui::Ui, maximized: bool) {
    // Right to left, because both conventions anchor the buttons to the corner
    // and grow inwards: close is always the outermost.
    for button in [Button::Close, Button::Maximise, Button::Minimise] {
        draw_button(ui, button, maximized);
    }
}

fn draw_button(ui: &mut egui::Ui, button: Button, maximized: bool) {
    let (size, rounding) = match CHROME {
        // A circle in a tall bar, with air around it.
        Chrome::Gnome => (egui::vec2(24.0, 24.0), 12.0),
        // Flush to the corner, the full height of the caption, no gap.
        Chrome::Windows => (egui::vec2(46.0, bar_height()), 0.0),
    };
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let hovered = response.hovered();
    let held = response.is_pointer_button_down_on();

    let painter = ui.painter();
    let background = match (CHROME, button, hovered, held) {
        (_, _, false, _) => match CHROME {
            // GNOME's buttons are visible when idle: a faint filled circle.
            Chrome::Gnome => token::SURFACE_3,
            // Windows' are not: the glyph sits on the caption until pointed at.
            Chrome::Windows => egui::Color32::TRANSPARENT,
        },
        // The one place the two agree to differ loudly: closing is dangerous on
        // Windows and says so, and is just another button on GNOME.
        (Chrome::Windows, Button::Close, true, held) => {
            if held {
                egui::Color32::from_rgb(0xB0, 0x27, 0x1B)
            } else {
                egui::Color32::from_rgb(0xC4, 0x2B, 0x1C)
            }
        }
        (_, _, true, true) => token::SURFACE_2.gamma_multiply(2.2),
        (_, _, true, false) => match CHROME {
            Chrome::Gnome => token::SURFACE_3.gamma_multiply(1.6),
            Chrome::Windows => token::SURFACE_3,
        },
    };
    if background != egui::Color32::TRANSPARENT {
        painter.rect_filled(rect, rounding, background);
    }

    // Bright at rest, not grey: Adwaita's symbols read as the same weight as
    // the window title beside them, and a dim glyph is the giveaway that a
    // title bar was drawn by something that is guessing.
    let tint = match (CHROME, button, hovered) {
        (Chrome::Windows, Button::Close, true) => egui::Color32::WHITE,
        _ => token::TEXT_HI,
    };
    glyph(painter, rect, button, maximized, tint);

    if response.clicked() {
        let command = match button {
            Button::Minimise => egui::ViewportCommand::Minimized(true),
            Button::Maximise => egui::ViewportCommand::Maximized(!maximized),
            Button::Close => egui::ViewportCommand::Close,
        };
        ui.ctx().send_viewport_cmd(command);
    }
}

/// The mark on a button, stroked rather than set in a font: the symbolic icons
/// both desktops use are line drawings, and a text glyph would be at the mercy
/// of whichever font happened to have it.
fn glyph(painter: &egui::Painter, rect: egui::Rect, button: Button, maximized: bool, tint: egui::Color32) {
    // Windows draws a noticeably finer line than GNOME does.
    let width: f32 = match CHROME {
        Chrome::Gnome => 1.6,
        Chrome::Windows => 1.0,
    };
    let stroke = egui::Stroke::new(width, tint);
    let c = rect.center();
    let r: f32 = match CHROME {
        Chrome::Gnome => 4.5,
        Chrome::Windows => 5.0,
    };
    match button {
        Button::Minimise => {
            painter.line_segment([egui::pos2(c.x - r, c.y), egui::pos2(c.x + r, c.y)], stroke);
        }
        Button::Maximise if maximized => {
            // Restore: the two overlapping squares both desktops draw, the back
            // one peeking out at the top right.
            let front =
                egui::Rect::from_min_size(egui::pos2(c.x - r, c.y - r + 2.0), egui::vec2(r * 2.0 - 2.0, r * 2.0 - 2.0));
            painter.rect_stroke(front, 1.0, stroke, egui::StrokeKind::Inside);
            painter.line_segment([egui::pos2(c.x - r + 2.0, c.y - r), egui::pos2(c.x + r, c.y - r)], stroke);
            painter.line_segment([egui::pos2(c.x + r, c.y - r), egui::pos2(c.x + r, c.y + r - 2.0)], stroke);
        }
        Button::Maximise => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(r * 2.0, r * 2.0)),
                1.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        Button::Close => {
            painter.line_segment([egui::pos2(c.x - r, c.y - r), egui::pos2(c.x + r, c.y + r)], stroke);
            painter.line_segment([egui::pos2(c.x + r, c.y - r), egui::pos2(c.x - r, c.y + r)], stroke);
        }
    }
}

/// The empty part of the bar behaves like a title bar: drag it to move the
/// window, double-click it to maximise or restore. Without this the bar would
/// look like a title bar and do nothing, which is worse than not drawing one.
///
/// No right-click window menu: egui 0.32 has no command that asks the
/// compositor for one, and a menu of our own would be a fourth convention
/// belonging to no desktop.
pub(crate) fn title_bar_drag(ctx: &egui::Context, response: &egui::Response, maximized: bool) {
    if response.double_clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    } else if response.dragged() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
}

/// The eight resize grips around the window.
///
/// The compositor drew these when it drew the frame; with the frame gone they
/// have to be put back, or the window can only be resized from a corner nobody
/// can find. Drawn over everything, in a layer of their own, so a panel edge
/// that happens to sit under the window edge does not swallow the drag.
pub(crate) fn resize_edges(ctx: &egui::Context) {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)) {
        return;
    }
    let screen = ctx.screen_rect();
    // Corners first: where two bands overlap the corner has to win, or a
    // diagonal resize would be unreachable.
    let band = RESIZE_BAND;
    let zones: [(egui::Rect, egui::ResizeDirection, egui::CursorIcon); 8] = [
        (
            egui::Rect::from_min_max(screen.left_top(), screen.left_top() + egui::vec2(band, band)),
            egui::ResizeDirection::NorthWest,
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            egui::Rect::from_min_max(
                screen.right_top() - egui::vec2(band, 0.0),
                screen.right_top() + egui::vec2(0.0, band),
            ),
            egui::ResizeDirection::NorthEast,
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            egui::Rect::from_min_max(
                screen.left_bottom() - egui::vec2(0.0, band),
                screen.left_bottom() + egui::vec2(band, 0.0),
            ),
            egui::ResizeDirection::SouthWest,
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            egui::Rect::from_min_max(screen.right_bottom() - egui::vec2(band, band), screen.right_bottom()),
            egui::ResizeDirection::SouthEast,
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            egui::Rect::from_min_max(
                screen.left_top() + egui::vec2(band, 0.0),
                screen.right_top() + egui::vec2(-band, band),
            ),
            egui::ResizeDirection::North,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            egui::Rect::from_min_max(
                screen.left_bottom() + egui::vec2(band, -band),
                screen.right_bottom() - egui::vec2(band, 0.0),
            ),
            egui::ResizeDirection::South,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            egui::Rect::from_min_max(
                screen.left_top() + egui::vec2(0.0, band),
                screen.left_bottom() - egui::vec2(-band, band),
            ),
            egui::ResizeDirection::West,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            egui::Rect::from_min_max(
                screen.right_top() + egui::vec2(-band, band),
                screen.right_bottom() - egui::vec2(0.0, band),
            ),
            egui::ResizeDirection::East,
            egui::CursorIcon::ResizeHorizontal,
        ),
    ];

    egui::Area::new(egui::Id::new("window-resize-edges"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .interactable(true)
        .show(ctx, |ui| {
            for (index, (rect, direction, cursor)) in zones.into_iter().enumerate() {
                let response = ui.interact(rect, egui::Id::new(("window-resize", index)), egui::Sense::drag());
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(cursor);
                }
                if response.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
                }
            }
        });
}

/// Whether the window is maximised, as the backend last reported it.
pub(crate) fn is_maximized(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false))
}

/// The line under the bar. GNOME separates the header from the content with a
/// hairline; Windows does not.
pub(crate) fn header_underline(ui: &egui::Ui, rect: egui::Rect) {
    if CHROME == Chrome::Gnome {
        let y = rect.bottom() - 0.5;
        ui.painter().line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(1.0_f32, theme::token::SURFACE_0),
        );
    }
}
