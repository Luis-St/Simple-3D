//! The second tab row: the components of the project on screen.

use super::*;
use crate::app::App;
use crate::icon::{self, Glyph};
use crate::tabs::strip::{paint_tab, plus, plus_glyph, TabLook};
use crate::theme::{self, metric, token};

/// Whether the second row is drawn: only once a project in this window has a non-root component.
pub fn wanted(app: &App) -> bool {
    app.project.uses_components()
        || app.tabs.iter().enumerate().any(|(index, doc)| index != app.active && doc.project.uses_components())
}

/// One entry in the list of every component.
struct Listed {
    id: ComponentId,
    name: String,
    /// Whether it is the component on screen.
    active: bool,
    /// Whether it has a tab.
    open: bool,
    unsaved: bool,
}

/// What the row was asked to do, carried out after drawing.
enum Ask {
    Pick(ComponentId),
    Close(ComponentId),
    Rename(ComponentId),
    Delete(ComponentId),
    New,
}

/// The row of open components: the root first, the others as opened, then the full list.
pub fn show(app: &mut App, ctx: &egui::Context) {
    if !wanted(app) {
        return;
    }
    let active = app.project.active;
    let tabs: Vec<(ComponentId, String, bool)> = app
        .project
        .open
        .iter()
        .map(|&id| {
            let unsaved = if id == active {
                app.history.revision() != app.saved_revision
            } else {
                app.project.get(id).is_some_and(Component::unsaved)
            };
            (id, app.component_label(id).unwrap_or_default(), unsaved)
        })
        .collect();
    let listed: Vec<Listed> = app
        .project
        .components
        .iter()
        .map(|c| {
            let open = tabs.iter().find(|(id, ..)| *id == c.id);
            Listed {
                id: c.id,
                name: app.component_label(c.id).unwrap_or_default(),
                active: c.id == active,
                open: open.is_some(),
                unsaved: open.map_or_else(|| c.unsaved(), |(.., unsaved)| *unsaved),
            }
        })
        .collect();
    let mut asked: Option<Ask> = None;

    let frame = egui::Frame::NONE.fill(token::SURFACE_1);
    egui::TopBottomPanel::top("component-tabs").frame(frame).exact_height(metric::TAB_BAR).show(ctx, |ui| {
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.max_rect().bottom() - 0.5,
            egui::Stroke::new(1.0_f32, token::SURFACE_3),
        );
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(1.0, 0.0);
            for (id, name, unsaved) in &tabs {
                tab(ui, *id, name, *unsaved, *id == active, &mut asked);
            }
            if plus(ui, "New component") {
                asked = Some(Ask::New);
            }
            list(ui, &listed, &mut asked);
        });
    });

    match asked {
        Some(Ask::Pick(id)) => app.activate_component(id),
        Some(Ask::Close(id)) => app.close_component(id),
        Some(Ask::Rename(id)) => {
            app.activate_component(id);
            let root = app.scene.root();
            app.rename = Some((root, app.scene.node(root).name.clone()));
        }
        Some(Ask::Delete(id)) => app.ask_delete_component(id),
        Some(Ask::New) => app.new_component(),
        None => {}
    }
}

/// A component tab's id, for tests.
pub(crate) fn tab_id(id: ComponentId) -> egui::Id {
    egui::Id::new(("component-tab", id))
}

/// One component's tab: like a document tab but quieter, closable except for the root.
fn tab(ui: &mut egui::Ui, id: ComponentId, name: &str, unsaved: bool, active: bool, asked: &mut Option<Ask>) {
    let closable = id != ROOT_COMPONENT;
    let look = TabLook { min: 80.0, max: 200.0, closable, active, carried: false, sense: egui::Sense::click() };
    let (response, close) = paint_tab(ui, tab_id(id), name, unsaved, look);
    let closed = close.is_some_and(|close| close.clicked()) || (closable && response.middle_clicked());
    let hover = match (id == ROOT_COMPONENT, unsaved) {
        (true, _) => format!("{name}: the root component, which is the project itself"),
        (false, true) => format!("{name} \u{2022} unsaved changes"),
        (false, false) => name.to_string(),
    };
    let response = response.on_hover_text(hover);
    if closed {
        *asked = Some(Ask::Close(id));
    } else if response.clicked() {
        *asked = Some(Ask::Pick(id));
    }

    response.context_menu(|ui| {
        ui.add(egui::Label::new(theme::hint(name)).selectable(false));
        ui.separator();
        if ui.button("Rename").clicked() {
            *asked = Some(Ask::Rename(id));
            ui.close();
        }
        if closable {
            if ui.button("Close").on_hover_text("Put the tab away; the component stays in the project").clicked() {
                *asked = Some(Ask::Close(id));
                ui.close();
            }
            ui.separator();
            if ui.button("Delete the component\u{2026}").clicked() {
                *asked = Some(Ask::Delete(id));
                ui.close();
            }
        }
    });
}

/// The arrow listing every component, to open, delete or create one. Painted because the UI
/// font has no small triangles.
fn list(ui: &mut egui::Ui, listed: &[Listed], asked: &mut Option<Ask>) {
    let size = egui::vec2(28.0, ui.available_height());
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&response));
    let colour = if response.hovered() || open { token::TEXT_HI } else { token::TEXT_LO };
    if response.hovered() || open {
        ui.painter().rect_filled(rect, 0.0, token::SURFACE_2);
    }
    theme::twisty(ui.painter(), rect.center(), true, colour);
    let response = response.on_hover_text(format!("Every component ({})", listed.len()));
    egui::Popup::menu(&response).show(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 1.0);
        // Sized to the longest name within reason, so delete buttons line up and long names are cut.
        let font = egui::FontId::proportional(theme::font::VALUE);
        let widest = listed
            .iter()
            .map(|entry| {
                ui.fonts(|fonts| fonts.layout_no_wrap(entry.name.clone(), font.clone(), token::TEXT_HI).size().x)
            })
            .fold(0.0_f32, f32::max);
        let width = (widest + LIST_ROOM).clamp(220.0, 360.0);

        let count = if listed.len() == 1 { "1 component".to_string() } else { format!("{} components", listed.len()) };
        ui.add(egui::Label::new(theme::hint(count)).selectable(false));
        ui.add_space(3.0);
        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            for entry in listed {
                list_row(ui, entry, width, asked);
            }
        });
        ui.add_space(2.0);
        ui.separator();
        if new_row(ui, width) {
            *asked = Some(Ask::New);
        }
        if asked.is_some() {
            ui.close();
        }
    });
}

/// Room beside a list row's name for the glyph, marks and delete button.
const LIST_ROOM: f32 = 96.0;

/// A component's list row id, for tests.
pub(crate) fn list_row_id(id: ComponentId) -> egui::Id {
    egui::Id::new(("component-list-row", id))
}

/// One component in the list: the row opens it, the bin deletes it. The active one is lit, and
/// open ones are brighter than closed ones.
fn list_row(ui: &mut egui::Ui, entry: &Listed, width: f32, asked: &mut Option<Ask>) {
    const BIN: f32 = 20.0;
    let root = entry.id == ROOT_COMPONENT;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, metric::ROW + 2.0), egui::Sense::hover());
    let row = ui.interact(rect, list_row_id(entry.id), egui::Sense::click());
    let bin_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 4.0 - BIN * 0.5, rect.center().y),
        egui::Vec2::splat(BIN),
    );
    let bin = (!root).then(|| ui.interact(bin_rect, list_row_id(entry.id).with("delete"), egui::Sense::click()));
    let bin_hovered = bin.as_ref().is_some_and(egui::Response::hovered);

    let painter = ui.painter();
    let radius = egui::CornerRadius::same(3);
    if row.hovered() || bin_hovered {
        painter.rect_filled(rect, radius, token::SURFACE_3);
    } else if entry.active {
        painter.rect_filled(rect, radius, token::SURFACE_2);
    }
    if entry.active {
        painter.vline(rect.left() + 1.0, rect.y_range().shrink(4.0), egui::Stroke::new(2.0_f32, token::ACCENT));
    }

    let glyph_colour = if entry.active { token::ACCENT } else { token::TEXT_LO };
    let glyph_rect =
        egui::Rect::from_center_size(egui::pos2(rect.left() + 16.0, rect.center().y), egui::Vec2::splat(12.0));
    icon::draw(painter, glyph_rect, Glyph::Component, glyph_colour);

    let text_colour = if entry.active || entry.open || row.hovered() { token::TEXT_HI } else { token::TEXT_LO };
    let mut job = egui::text::LayoutJob::default();
    job.append(&entry.name, 0.0, egui::TextFormat::simple(egui::FontId::proportional(theme::font::VALUE), text_colour));
    let marks = match (root, entry.unsaved) {
        (true, true) => "root \u{2022}",
        (true, false) => "root",
        (false, true) => "\u{2022}",
        (false, false) => "",
    };
    if !marks.is_empty() {
        job.append(
            marks,
            6.0,
            egui::TextFormat::simple(egui::FontId::proportional(theme::font::SMALL), token::TEXT_LO),
        );
    }
    job.wrap.max_width = (bin_rect.left() - 6.0 - (rect.left() + 30.0)).max(8.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    painter.galley(egui::pos2(rect.left() + 30.0, rect.center().y - galley.size().y * 0.5), galley, text_colour);

    if let Some(bin) = bin {
        // Quiet until the row is hovered, red only on the bin itself.
        let colour = if bin.hovered() {
            painter.rect_filled(bin_rect, radius, token::SURFACE_2);
            token::DANGER
        } else if row.hovered() {
            token::TEXT_LO
        } else {
            token::TEXT_LO.gamma_multiply(0.45)
        };
        icon::draw(painter, bin_rect.shrink(4.0), Glyph::Delete, colour);
        if bin.on_hover_text("Delete the component, and every place it is used\u{2026}").clicked() {
            *asked = Some(Ask::Delete(entry.id));
            return;
        }
    }

    // Only the root has a tooltip, which would otherwise cover the next row while scanning the list.
    let row =
        if root { row.on_hover_text("The root component is the project itself, and cannot be deleted") } else { row };
    if row.clicked() {
        *asked = Some(Ask::Pick(entry.id));
    }
}

/// The list's last row, like the plus at the end of the tabs.
fn new_row(ui: &mut egui::Ui, width: f32) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, metric::ROW + 2.0), egui::Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, egui::CornerRadius::same(3), token::SURFACE_3);
    }
    let colour = if response.hovered() { token::TEXT_HI } else { token::TEXT_LO };
    plus_glyph(painter, egui::pos2(rect.left() + 16.0, rect.center().y), colour);
    painter.text(
        egui::pos2(rect.left() + 30.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "New component",
        egui::FontId::proportional(theme::font::VALUE),
        colour,
    );
    response.on_hover_text("Make a new, empty component and open it in a tab").clicked()
}
