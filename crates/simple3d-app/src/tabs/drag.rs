//! Dragging a tab off its row, and onto another window's (issue 107).
//!
//! Two gestures, one drag. A tab pulled off the row of tabs and let go opens in
//! a window of its own; a tab -- or a whole window's row, dragged by the empty
//! space beside the tabs -- let go over another window's row of tabs moves
//! there, and a window whose last document leaves is closed behind it.
//!
//! The second gesture needs to know where the other windows are, and that is
//! not something every window system will say. X11 and Windows answer; Wayland
//! does not, and will not: a client there is never told where it is on screen,
//! so a pointer that has left this window cannot be placed against another
//! window's row of tabs by arithmetic, and the drop has nothing to aim at. That
//! is why the same two moves are on the tab's own menu, which needs no
//! coordinates at all and is the way this works on a Wayland desktop -- see
//! `strip::menu`.

use crate::app::App;
use crate::shell::{OtherWindow, WindowRequest};
use crate::theme::{self, token};

/// A tab being carried: which one, and where the pointer has taken it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabDrag {
    /// The tab under the pointer when the drag started. For a whole-window drag
    /// it is the one on screen, which is what the ghost shows.
    pub tab: usize,
    /// Whether the whole window is being carried rather than one tab: the drag
    /// that starts on the empty part of the row.
    pub whole_window: bool,
    /// Where the pointer is, in this window's own coordinates.
    ///
    /// Kept here frame by frame rather than read at the moment of release,
    /// because a pointer that has been dragged outside the window is one the
    /// toolkit may already have reported as gone -- and the position it was last
    /// seen at is exactly what says the tab was pulled out.
    pub pos: egui::Pos2,
}

/// What letting go of a drag would do with the tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabDrop {
    /// Leave the tabs as they are: the pointer never left the row.
    Stay,
    /// Open it in a window of its own.
    NewWindow,
    /// Move it into the window with this id.
    Into(u64),
    /// Let go outside this window, with no way of telling what it was let go
    /// over: hold it out and let whichever window the pointer turns up in take
    /// it. See `Shell::resolve_offer`.
    Offer,
}

/// How far off the row a drag has to be before releasing it takes the tab out of
/// the window.
///
/// Most of a tab's own height: far enough that sliding along the row, or
/// overshooting it by a few pixels on the way to the close cross, is not a
/// window being opened -- and near enough that pulling a tab away is one
/// movement rather than a journey.
pub const PULL_OUT: f32 = 20.0;

/// Where a drag released at `pointer` would put what it is carrying.
///
/// `pointer`, `strip` and `contents` are in this window's coordinates; `origin`
/// is where this window's contents start on the desktop, and the strips in
/// `others` are on the desktop as well. A window system that does not say where
/// its windows are gives `None` for the origin, and then only the two gestures
/// that are about this window alone can be resolved.
///
/// Another window is only ever dropped on from *outside* this one. Windows
/// overlap, and two windows of the same application overlap along their rows of
/// tabs in particular: without this, dragging a tab along its own row while
/// another window happened to sit under that row would have moved the document
/// into the window nobody could see.
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
            // Where the window system says where windows are, the drop is
            // settled here and now: this is where the pointer is on the
            // desktop, and that is whose row of tabs it is over.
            Some(origin) => {
                let on_screen = pointer + origin.to_vec2();
                let onto = others.iter().find(|other| other.strip.is_some_and(|strip| strip.contains(on_screen)));
                if let Some(other) = onto {
                    return TabDrop::Into(other.id);
                }
            }
            // Where it does not -- Wayland, which tells a client nothing about
            // where it is -- the tab is held out instead, and the window the
            // pointer turns up over claims it. The pointer only reaches that
            // window once the button is up, because until then this window
            // holds it, so the question can only be asked after the drop.
            None if !others.is_empty() => return TabDrop::Offer,
            None => {}
        }
    }
    // A whole window has nowhere else to go: the gesture that carries one is
    // only ever about putting it into another window.
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

/// Follow the drag, say on screen what letting go would do, and do it when the
/// button comes up. Called once a frame after the row has drawn, the way a dock
/// drag is resolved after both docks have.
pub fn resolve_drag(app: &mut App, ctx: &egui::Context) {
    let Some(mut drag) = app.tab_drag else { return };
    if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
        drag.pos = pos;
    }
    app.tab_drag = Some(drag);
    // The tab may have gone while the drag ran -- closed from a menu, or moved
    // by the window at the other end of it.
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

/// The tab under the pointer while it is being carried, and a line saying what
/// would become of it. Drawn in the foreground layer so it is over the viewport
/// the pointer is usually above by then.
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
    /// Where this window's row of tabs is on the desktop, for another window to
    /// drop a tab onto. `None` wherever the window system does not say where a
    /// window is, which is every Wayland compositor.
    pub(crate) fn strip_on_screen(&self) -> Option<egui::Rect> {
        let origin = self.window_rect?.min;
        Some(self.strip_rect.translate(origin.to_vec2()))
    }
}
