//! The row of tabs itself.

use super::drag::TabDrag;
use crate::app::App;
use crate::shell::{OtherWindow, WindowRequest};
use crate::theme::{self, metric, token};

/// The row of open documents: the top of the workspace, between the docks and
/// over the viewport, so a tab sits above the model it holds.
///
/// Drawn by hand rather than out of widgets so a tab can be a shape -- the
/// active one lit along its top edge and joined to the workspace below it --
/// which is what makes the row readable at a glance.
///
/// The row is also where a document leaves the window: a tab dragged off it
/// opens in a window of its own, a tab dropped on another window's row moves
/// there, and the empty space beside the tabs carries the whole window the same
/// way (issue 107). What the drag does is resolved in [`super::drag`] once the
/// row has drawn; nothing here changes a window, it only says what was asked.
pub fn show(app: &mut App, ctx: &egui::Context) {
    // Everything the row draws is read off the application here, so the menus
    // below can be written without borrowing it a second time.
    let summaries: Vec<(String, bool)> = (0..app.tab_count()).map(|index| app.tab_summary(index)).collect();
    let others = app.other_windows.clone();
    let active = app.active;
    let carrying = app.tab_drag;
    let mut asked: Option<Ask> = None;

    let frame = egui::Frame::NONE.fill(token::SURFACE_1);
    let panel = egui::TopBottomPanel::top("tabs").frame(frame).exact_height(metric::TAB_BAR).show(ctx, |ui| {
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.max_rect().bottom() - 0.5,
            egui::Stroke::new(1.0_f32, token::SURFACE_3),
        );
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(1.0, 0.0);
            for (index, (name, unsaved)) in summaries.iter().enumerate() {
                let carried = carrying.is_some_and(|drag| !drag.whole_window && drag.tab == index);
                let (response, hit) = tab(ui, index, name, *unsaved, index == active, carried);
                if hit.is_some() {
                    asked = hit;
                }
                tab_menu(&response, index, name, summaries.len(), &others, &mut asked);
            }
            if plus(ui) {
                asked = Some(Ask::New);
            }
            // Whatever is left of the row after the tabs. Dragging it carries
            // the whole window to another window's row, which is the gesture
            // that merges two windows; right-clicking it offers the same move
            // without a drag.
            rest_of_the_row(ui, active, &others, &mut asked);
        });
    });
    // Where the row is, for a drag to be measured against and for another
    // window to drop a tab onto.
    app.strip_rect = panel.response.rect;
    app.window_rect = ctx.input(|i| i.viewport().inner_rect);
    // And whether the pointer is on it, for a tab another window is holding out
    // to be claimed by this one (issue 107).
    app.pointer_on_strip = ctx.input(|i| i.pointer.latest_pos()).is_some_and(|pos| app.strip_rect.contains(pos));

    match asked {
        // After the row, so closing a tab cannot renumber the ones still being
        // drawn.
        Some(Ask::Pick(index)) => app.activate_tab(index),
        Some(Ask::Close(index)) => app.close_tab(index),
        Some(Ask::New) => app.run(simple3d_core::keymap::Command::New),
        Some(Ask::Drag(drag)) => app.tab_drag = Some(drag),
        Some(Ask::Detach(index)) => app.window_request = Some(WindowRequest::Detach(index)),
        Some(Ask::MoveTab(index, to)) => app.window_request = Some(WindowRequest::MoveTab(index, to)),
        Some(Ask::MoveAll(to)) => app.window_request = Some(WindowRequest::MoveAll(to)),
        None => {}
    }
}

/// What the row was asked to do, carried out after it has finished drawing.
enum Ask {
    Pick(usize),
    Close(usize),
    New,
    Drag(TabDrag),
    Detach(usize),
    MoveTab(usize, u64),
    MoveAll(u64),
}

/// The grip a tab is: named rather than taken from where the tab happens to sit,
/// so a test can find the tab it means to drag.
pub(crate) fn tab_id(index: usize) -> egui::Id {
    egui::Id::new(("tab", index))
}

/// The grip the empty part of the row is.
pub(crate) fn rest_id() -> egui::Id {
    egui::Id::new("tab-strip-rest")
}

/// One tab. Returns its own response -- the menu hangs off it -- and what was
/// done to it, if anything.
fn tab(
    ui: &mut egui::Ui,
    index: usize,
    name: &str,
    unsaved: bool,
    active: bool,
    carried: bool,
) -> (egui::Response, Option<Ask>) {
    const MIN: f32 = 96.0;
    const MAX: f32 = 220.0;
    const CLOSE: f32 = 16.0;

    let font = egui::FontId::proportional(theme::font::VALUE);
    let label = if unsaved { format!("{name} \u{2022}") } else { name.to_string() };
    let text_width = ui.fonts(|fonts| fonts.layout_no_wrap(label.clone(), font.clone(), token::TEXT_HI).size().x);
    let width = (text_width + CLOSE + 24.0).clamp(MIN, MAX.min(ui.available_width().max(MIN)));
    let height = ui.available_height();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    // Click *and* drag: a tab is picked with one and moved between windows with
    // the other, and egui tells the two apart by how far the pointer travelled.
    let response = ui.interact(rect, tab_id(index), egui::Sense::click_and_drag());

    let fill = if carried {
        token::SURFACE_3
    } else if active {
        token::SURFACE_0B
    } else if response.hovered() {
        token::SURFACE_2
    } else {
        token::SURFACE_1
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    if active {
        // The lit top edge, and no rule along the bottom: the active tab is the
        // workspace's own top, not a button sitting above it.
        ui.painter().hline(rect.x_range(), rect.top() + 1.0, egui::Stroke::new(2.0_f32, token::ACCENT));
    } else {
        ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0_f32, token::SURFACE_3));
    }
    ui.painter().vline(rect.right() - 0.5, rect.y_range(), egui::Stroke::new(1.0_f32, token::SURFACE_0));

    let close_rect =
        egui::Rect::from_center_size(egui::pos2(rect.right() - 13.0, rect.center().y), egui::Vec2::splat(CLOSE));
    let text_colour = if active { token::TEXT_HI } else { token::TEXT_LO };
    let mut job = egui::text::LayoutJob::simple_singleline(label, font, text_colour);
    job.wrap.max_width = (close_rect.left() - rect.left() - 16.0).max(8.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    ui.painter().galley(egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y * 0.5), galley, text_colour);

    let close = ui.interact(close_rect, tab_id(index).with("close"), egui::Sense::click());
    if close.hovered() {
        ui.painter().rect_filled(close_rect, 3.0, token::SURFACE_3);
    }
    cross(ui.painter(), close_rect.center(), if close.hovered() { token::TEXT_HI } else { token::TEXT_LO });
    let response =
        response.on_hover_text(if unsaved { format!("{name} \u{2022} unsaved changes") } else { name.to_string() });

    let hit = if close.clicked() || response.middle_clicked() {
        Some(Ask::Close(index))
    } else if response.drag_started() {
        let pos = response.interact_pointer_pos().unwrap_or_else(|| rect.center());
        Some(Ask::Drag(TabDrag { tab: index, whole_window: false, pos }))
    } else if response.clicked() {
        Some(Ask::Pick(index))
    } else {
        None
    };
    (response, hit)
}

/// The tab's own menu: where a document is sent to another window on a desktop
/// that will not let a drag find one (see [`super::drag`]), and where the move
/// is spelled out for anyone who would never think to drag a tab at all.
fn tab_menu(
    response: &egui::Response,
    index: usize,
    name: &str,
    count: usize,
    others: &[OtherWindow],
    asked: &mut Option<Ask>,
) {
    response.context_menu(|ui| {
        ui.add(egui::Label::new(theme::hint(name)).selectable(false));
        ui.separator();
        if ui
            .add_enabled(count > 1, egui::Button::new("Move to a window of its own"))
            .on_hover_text("Take this document out of the window and open it in one of its own")
            .clicked()
        {
            *asked = Some(Ask::Detach(index));
            ui.close();
        }
        for other in others {
            if ui.button(format!("Move to {}", other.name)).clicked() {
                *asked = Some(Ask::MoveTab(index, other.id));
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Close").clicked() {
            *asked = Some(Ask::Close(index));
            ui.close();
        }
    });
}

/// The empty space after the tabs: the window's own grip.
fn rest_of_the_row(ui: &mut egui::Ui, active: usize, others: &[OtherWindow], asked: &mut Option<Ask>) {
    let room = egui::vec2(ui.available_width().max(1.0), ui.available_height());
    let (rect, _) = ui.allocate_exact_size(room, egui::Sense::hover());
    let response = ui.interact(rect, rest_id(), egui::Sense::click_and_drag());
    if response.drag_started() && !others.is_empty() {
        let pos = response.interact_pointer_pos().unwrap_or_else(|| rect.center());
        *asked = Some(Ask::Drag(TabDrag { tab: active, whole_window: true, pos }));
    }
    if others.is_empty() {
        return;
    }
    response.context_menu(|ui| {
        ui.add(egui::Label::new(theme::hint("This window")).selectable(false));
        ui.separator();
        for other in others {
            if ui.button(format!("Move every document to {}", other.name)).clicked() {
                *asked = Some(Ask::MoveAll(other.id));
                ui.close();
            }
        }
    });
}

/// The button at the end of the row: another document.
pub(crate) fn plus(ui: &mut egui::Ui) -> bool {
    let size = egui::vec2(28.0, ui.available_height());
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let colour = if response.hovered() { token::TEXT_HI } else { token::TEXT_LO };
    if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, token::SURFACE_2);
    }
    let centre = rect.center();
    let stroke = egui::Stroke::new(1.4_f32, colour);
    ui.painter().hline(centre.x - 5.0..=centre.x + 5.0, centre.y, stroke);
    ui.painter().vline(centre.x, centre.y - 5.0..=centre.y + 5.0, stroke);
    response.on_hover_text("New document").clicked()
}

/// The close cross, drawn rather than typed: a glyph would depend on the font
/// having it, and would sit off centre in most that do.
pub(crate) fn cross(painter: &egui::Painter, centre: egui::Pos2, colour: egui::Color32) {
    let r = 3.5;
    let stroke = egui::Stroke::new(1.3_f32, colour);
    painter.line_segment([centre + egui::vec2(-r, -r), centre + egui::vec2(r, r)], stroke);
    painter.line_segment([centre + egui::vec2(r, -r), centre + egui::vec2(-r, r)], stroke);
}
