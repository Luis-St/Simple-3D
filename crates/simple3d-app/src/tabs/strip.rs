//! The row of tabs itself.

use super::drag::TabDrag;
use crate::app::App;
use crate::shell::{OtherWindow, WindowRequest};
use crate::theme::{self, metric, token};

/// The row of open documents above the viewport. Hand-drawn so the active tab joins the workspace.
///
/// Also where documents leave the window: dragged off, onto another window's row, or the whole
/// window via the empty space (issue 107). Drags are resolved in [`super::drag`]; this only
/// reports what was asked.
pub fn show(app: &mut App, ctx: &egui::Context) {
    // Read up front so the menus below need no second borrow of the app.
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
            if plus(ui, "New document") {
                asked = Some(Ask::New);
            }
            // The rest of the row: dragging it carries the whole window, right-click offers the same.
            rest_of_the_row(ui, active, &others, &mut asked);
        });
    });
    // The row's rect, for drag measurement and as a drop target for other windows.
    app.strip_rect = panel.response.rect;
    app.window_rect = ctx.input(|i| i.viewport().inner_rect);
    // Whether the pointer is on it, to claim a tab another window holds out (issue 107).
    app.pointer_on_strip = ctx.input(|i| i.pointer.latest_pos()).is_some_and(|pos| app.strip_rect.contains(pos));

    match asked {
        // After drawing, so closing a tab cannot renumber ones still being drawn.
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

/// What the row was asked to do, carried out after drawing.
enum Ask {
    Pick(usize),
    Close(usize),
    New,
    Drag(TabDrag),
    Detach(usize),
    MoveTab(usize, u64),
    MoveAll(u64),
}

/// A tab's id, stable so tests can find it.
pub(crate) fn tab_id(index: usize) -> egui::Id {
    egui::Id::new(("tab", index))
}

/// The id of the row's empty part.
pub(crate) fn rest_id() -> egui::Id {
    egui::Id::new("tab-strip-rest")
}

/// One tab: its response (for the menu) and what was done to it.
fn tab(
    ui: &mut egui::Ui,
    index: usize,
    name: &str,
    unsaved: bool,
    active: bool,
    carried: bool,
) -> (egui::Response, Option<Ask>) {
    // Click to pick, drag to move between windows; egui tells them apart by travel.
    let look = TabLook { min: 96.0, max: 220.0, closable: true, active, carried, sense: egui::Sense::click_and_drag() };
    let (response, close) = paint_tab(ui, tab_id(index), name, unsaved, look);
    let closed = close.is_some_and(|close| close.clicked());
    let response =
        response.on_hover_text(if unsaved { format!("{name} \u{2022} unsaved changes") } else { name.to_string() });

    let hit = if closed || response.middle_clicked() {
        Some(Ask::Close(index))
    } else if response.drag_started() {
        let pos = response.interact_pointer_pos().unwrap_or_else(|| response.rect.center());
        Some(Ask::Drag(TabDrag { tab: index, whole_window: false, pos }))
    } else if response.clicked() {
        Some(Ask::Pick(index))
    } else {
        None
    };
    (response, hit)
}

/// How [`paint_tab`] draws a tab: its width range, whether it has a close cross, and its state.
pub(crate) struct TabLook {
    pub min: f32,
    pub max: f32,
    pub closable: bool,
    pub active: bool,
    /// Being dragged out of the row.
    pub carried: bool,
    pub sense: egui::Sense,
}

/// Paint a tab of a strip: the tab's response and, if closable, its close cross's.
pub(crate) fn paint_tab(
    ui: &mut egui::Ui,
    id: egui::Id,
    name: &str,
    unsaved: bool,
    look: TabLook,
) -> (egui::Response, Option<egui::Response>) {
    const CLOSE: f32 = 16.0;

    let font = egui::FontId::proportional(theme::font::VALUE);
    let label = if unsaved { format!("{name} \u{2022}") } else { name.to_string() };
    let text_width = ui.fonts(|fonts| fonts.layout_no_wrap(label.clone(), font.clone(), token::TEXT_HI).size().x);
    let room = if look.closable { CLOSE + 24.0 } else { 20.0 };
    let width = (text_width + room).clamp(look.min, look.max.min(ui.available_width().max(look.min)));
    let height = ui.available_height();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let response = ui.interact(rect, id, look.sense);

    let fill = if look.carried {
        token::SURFACE_3
    } else if look.active {
        token::SURFACE_0B
    } else if response.hovered() {
        token::SURFACE_2
    } else {
        token::SURFACE_1
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    if look.active {
        // Lit top edge and no bottom rule: the active tab is part of the workspace.
        ui.painter().hline(rect.x_range(), rect.top() + 1.0, egui::Stroke::new(2.0_f32, token::ACCENT));
    } else {
        ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0_f32, token::SURFACE_3));
    }
    ui.painter().vline(rect.right() - 0.5, rect.y_range(), egui::Stroke::new(1.0_f32, token::SURFACE_0));

    let close_rect =
        egui::Rect::from_center_size(egui::pos2(rect.right() - 13.0, rect.center().y), egui::Vec2::splat(CLOSE));
    let text_right = if look.closable { close_rect.left() } else { rect.right() - 4.0 };
    let text_colour = if look.active { token::TEXT_HI } else { token::TEXT_LO };
    let mut job = egui::text::LayoutJob::simple_singleline(label, font, text_colour);
    job.wrap.max_width = (text_right - rect.left() - 16.0).max(8.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    ui.painter().galley(egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y * 0.5), galley, text_colour);

    if !look.closable {
        return (response, None);
    }
    let close = ui.interact(close_rect, id.with("close"), egui::Sense::click());
    if close.hovered() {
        ui.painter().rect_filled(close_rect, 3.0, token::SURFACE_3);
    }
    cross(ui.painter(), close_rect.center(), if close.hovered() { token::TEXT_HI } else { token::TEXT_LO });
    (response, Some(close))
}

/// The tab's menu: sending a document to another window where drags cannot find it
/// (see [`super::drag`]), and for those who would not drag.
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

/// The plus at the end of the row: a new document, or a new component on the component row.
pub(crate) fn plus(ui: &mut egui::Ui, hover: &str) -> bool {
    let size = egui::vec2(28.0, ui.available_height());
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let colour = if response.hovered() { token::TEXT_HI } else { token::TEXT_LO };
    if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, token::SURFACE_2);
    }
    plus_glyph(ui.painter(), rect.center(), colour);
    response.on_hover_text(hover).clicked()
}

/// The plus sign, painted like [`cross`].
pub(crate) fn plus_glyph(painter: &egui::Painter, centre: egui::Pos2, colour: egui::Color32) {
    let stroke = egui::Stroke::new(1.4_f32, colour);
    painter.hline(centre.x - 5.0..=centre.x + 5.0, centre.y, stroke);
    painter.vline(centre.x, centre.y - 5.0..=centre.y + 5.0, stroke);
}

/// The close cross, painted so it does not depend on the font and sits centred.
pub(crate) fn cross(painter: &egui::Painter, centre: egui::Pos2, colour: egui::Color32) {
    let r = 3.5;
    let stroke = egui::Stroke::new(1.3_f32, colour);
    painter.line_segment([centre + egui::vec2(-r, -r), centre + egui::vec2(r, r)], stroke);
    painter.line_segment([centre + egui::vec2(r, -r), centre + egui::vec2(-r, r)], stroke);
}
