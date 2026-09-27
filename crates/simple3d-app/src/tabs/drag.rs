//! Dragging a tab off its row, and onto another window's (issue 107).
//!
//! A tab released off the row opens in its own window; a tab (or a whole window, dragged by the
//! row's empty space) released over another window's row moves there. Wayland never reveals
//! window positions, so the tab menu offers the same moves without coordinates (`strip::menu`).

use crate::app::App;
use crate::shell::{OtherWindow, WindowRequest};
use crate::theme::{self, token};

/// A tab being carried: which one, and where the pointer has taken it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabDrag {
    /// The tab under the pointer when the drag started; for a whole-window drag, the one on screen.
    pub tab: usize,
    /// Whether the whole window is carried: a drag started on the row's empty part.
    pub whole_window: bool,
    /// The pointer in this window's coordinates, tracked per frame since the toolkit may already
    /// report a pointer outside the window as gone at release.
    pub pos: egui::Pos2,
}

/// What releasing the drag would do with the tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabDrop {
    /// Nothing: the pointer never left the row.
    Stay,
    /// Open it in its own window.
    NewWindow,
    /// Move it into the window with this id.
    Into(u64),
    /// Released outside with no way to tell over what: hold it out for the window the pointer
    /// reaches. See `Shell::resolve_offer`.
    Offer,
}

/// How far off the row a release must be to take the tab out: most of a tab's height, so
/// sliding along the row or overshooting is not a detach.
pub const PULL_OUT: f32 = 20.0;

/// Where releasing at `pointer` would put what is carried.
///
/// `pointer`, `strip` and `contents` are window coordinates; `origin` and `others` are desktop
/// coordinates, `origin` being `None` where unknown. Other windows are only dropped on from outside
/// this one, since overlapping windows would otherwise capture drags along the own row.
pub fn drop_of(
    pointer: egui::Pos2,
    strip: egui::Rect,
    contents: egui::Rect,
    origin: Option<egui::Pos2>,
    others: &[OtherWindow],
    whole_window: bool,
) -> TabDrop {
    if !contents.contains(pointer) {
        match origin {
            // Window positions known: resolve against the desktop now.
            Some(origin) => {
                let on_screen = pointer + origin.to_vec2();
                let onto = others.iter().find(|other| other.strip.is_some_and(|strip| strip.contains(on_screen)));
                if let Some(other) = onto {
                    return TabDrop::Into(other.id);
                }
            }
            // Positions unknown (Wayland): hold the tab out; the target window can only see the pointer
            // after the button is up.
            None if !others.is_empty() => return TabDrop::Offer,
            None => {}
        }
    }
    // A whole window can only go into another window.
    if whole_window {
        return TabDrop::Stay;
    }
    let off_the_row = pointer.y < strip.top() - PULL_OUT || pointer.y > strip.bottom() + PULL_OUT;
    if off_the_row {
        TabDrop::NewWindow
    } else {
        TabDrop::Stay
    }
}

/// Follow the drag, show what releasing would do, and do it on release. Called after the row
/// has drawn, like dock drags.
pub fn resolve_drag(app: &mut App, ctx: &egui::Context) {
    let Some(mut drag) = app.tab_drag else { return };
    if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
        drag.pos = pos;
    }
    app.tab_drag = Some(drag);
    // The tab may have gone during the drag (closed or moved away).
    if drag.tab >= app.tab_count() {
        app.tab_drag = None;
        return;
    }
    let drop = drop_of(
        drag.pos,
        app.strip_rect,
        ctx.screen_rect(),
        app.window_rect.map(|rect| rect.min),
        &app.other_windows,
        drag.whole_window,
    );
    ghost(app, ctx, drag, drop);
    if ctx.input(|i| !i.pointer.any_down()) {
        app.tab_drag = None;
        app.window_request = match drop {
            TabDrop::Stay => None,
            TabDrop::NewWindow => Some(WindowRequest::Detach(drag.tab)),
            TabDrop::Into(id) if drag.whole_window => Some(WindowRequest::MoveAll(id)),
            TabDrop::Into(id) => Some(WindowRequest::MoveTab(drag.tab, id)),
            TabDrop::Offer if drag.whole_window => Some(WindowRequest::Offer(None)),
            TabDrop::Offer => Some(WindowRequest::Offer(Some(drag.tab))),
        };
    }
}

/// The carried tab under the pointer with a line on what would happen, in the foreground layer.
fn ghost(app: &App, ctx: &egui::Context, drag: TabDrag, drop: TabDrop) {
    let name = if drag.whole_window { app.window_summary() } else { app.tab_summary(drag.tab).0 };
    let says = match drop {
        TabDrop::Stay if drag.whole_window => "Drop on another window's tabs".to_string(),
        TabDrop::Stay => "Pull it off the row for a window of its own".to_string(),
        TabDrop::NewWindow => "A window of its own".to_string(),
        TabDrop::Offer => "Let go over another window's tabs".to_string(),
        TabDrop::Into(id) => {
            let into = app.other_windows.iter().find(|other| other.id == id);
            format!("Into {}", into.map_or_else(|| "the other window".to_string(), |other| other.name.clone()))
        }
    };
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("tab-drag")));
    let title = egui::FontId::proportional(theme::font::VALUE);
    let caption = egui::FontId::proportional(theme::font::SMALL);
    let width = ctx.fonts(|fonts| {
        let name = fonts.layout_no_wrap(name.clone(), title.clone(), token::TEXT_HI).size().x;
        let says = fonts.layout_no_wrap(says.clone(), caption.clone(), token::TEXT_LO).size().x;
        name.max(says) + 20.0
    });
    let rect = egui::Rect::from_min_size(drag.pos + egui::vec2(10.0, 8.0), egui::vec2(width, 38.0));
    painter.rect_filled(rect, 3.0, token::SURFACE_2);
    painter.rect_stroke(
        rect,
        3.0,
        egui::Stroke::new(1.0_f32, if drop == TabDrop::Stay { token::SURFACE_3 } else { token::ACCENT }),
        egui::StrokeKind::Inside,
    );
    painter.text(rect.left_top() + egui::vec2(10.0, 5.0), egui::Align2::LEFT_TOP, name, title, token::TEXT_HI);
    painter.text(rect.left_top() + egui::vec2(10.0, 21.0), egui::Align2::LEFT_TOP, says, caption, token::TEXT_LO);
}

impl App {
    /// This window's tab row on the desktop, for other windows to drop onto; `None` where unknown
    /// (Wayland).
    pub(crate) fn strip_on_screen(&self) -> Option<egui::Rect> {
        let origin = self.window_rect?.min;
        Some(self.strip_rect.translate(origin.to_vec2()))
    }
}
