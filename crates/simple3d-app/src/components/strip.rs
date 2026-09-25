//! The second row of tabs: the components of the project on screen.

use super::*;
use crate::app::App;
use crate::theme::{self, metric, token};

/// Whether the second row is drawn at all: only once a project open in this
/// window has a component beyond its root. Until then every project is the one
/// tree it always was, and the window looks as it always did.
pub fn wanted(app: &App) -> bool {
    app.project.uses_components()
        || app.tabs.iter().enumerate().any(|(index, doc)| index != app.active && doc.project.uses_components())
}

/// One entry in the list of every component.
struct Listed {
    id: ComponentId,
    name: String,
    /// Why it cannot be placed in the component on screen, if it cannot.
    refusal: Option<String>,
}

/// What the row was asked to do, carried out after it has finished drawing.
enum Ask {
    Pick(ComponentId),
    Close(ComponentId),
    Rename(ComponentId),
    Place(ComponentId),
    Delete(ComponentId),
    New,
}

/// The row of the project's open components, under the row of projects: the
/// root component first and always there, the others as they were opened, and
/// at the end the list of every component the project has.
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
        .map(|c| Listed {
            id: c.id,
            name: app.component_label(c.id).unwrap_or_default(),
            refusal: app.can_integrate(c.id).err(),
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
            if crate::tabs::strip::plus(ui, "New component") {
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
        Some(Ask::Place(id)) => app.integrate_component(id),
        Some(Ask::Delete(id)) => app.ask_delete_component(id),
        Some(Ask::New) => app.new_component(),
        None => {}
    }
}

/// The grip a component's tab is, so a test can find it.
pub(crate) fn tab_id(id: ComponentId) -> egui::Id {
    egui::Id::new(("component-tab", id))
}

/// One component's tab. Drawn like a document's, a little quieter, with a
/// close cross on every tab but the root's.
fn tab(ui: &mut egui::Ui, id: ComponentId, name: &str, unsaved: bool, active: bool, asked: &mut Option<Ask>) {
    const MIN: f32 = 80.0;
    const MAX: f32 = 200.0;
    const CLOSE: f32 = 16.0;
    let closable = id != ROOT_COMPONENT;

    let font = egui::FontId::proportional(theme::font::VALUE);
    let label = if unsaved { format!("{name} \u{2022}") } else { name.to_string() };
    let text_width = ui.fonts(|fonts| fonts.layout_no_wrap(label.clone(), font.clone(), token::TEXT_HI).size().x);
    let room = if closable { CLOSE + 24.0 } else { 20.0 };
    let width = (text_width + room).clamp(MIN, MAX.min(ui.available_width().max(MIN)));
    let height = ui.available_height();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let response = ui.interact(rect, tab_id(id), egui::Sense::click());

    let fill = if active {
        token::SURFACE_0B
    } else if response.hovered() {
        token::SURFACE_2
    } else {
        token::SURFACE_1
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    if active {
        ui.painter().hline(rect.x_range(), rect.top() + 1.0, egui::Stroke::new(2.0_f32, token::ACCENT));
    } else {
        ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0_f32, token::SURFACE_3));
    }
    ui.painter().vline(rect.right() - 0.5, rect.y_range(), egui::Stroke::new(1.0_f32, token::SURFACE_0));

    let close_rect =
        egui::Rect::from_center_size(egui::pos2(rect.right() - 13.0, rect.center().y), egui::Vec2::splat(CLOSE));
    let text_right = if closable { close_rect.left() } else { rect.right() - 4.0 };
    let text_colour = if active { token::TEXT_HI } else { token::TEXT_LO };
    let mut job = egui::text::LayoutJob::simple_singleline(label, font, text_colour);
    job.wrap.max_width = (text_right - rect.left() - 16.0).max(8.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    ui.painter().galley(egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y * 0.5), galley, text_colour);

    let mut closed = false;
    if closable {
        let close = ui.interact(close_rect, tab_id(id).with("close"), egui::Sense::click());
        if close.hovered() {
            ui.painter().rect_filled(close_rect, 3.0, token::SURFACE_3);
        }
        crate::tabs::strip::cross(
            ui.painter(),
            close_rect.center(),
            if close.hovered() { token::TEXT_HI } else { token::TEXT_LO },
        );
        closed = close.clicked() || response.middle_clicked();
    }
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

/// The arrow at the end of the row, which lists every component of the
/// project: to open one, to place one in the component on screen, or to
/// delete one. Painted rather than typed, since the interface font has no
/// small triangles.
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
        // A grid, so the buttons line up in columns down the list -- and so
        // the menu is as wide as its longest name, not as wide as the screen.
        egui::Grid::new("component-list").num_columns(3).spacing(egui::vec2(8.0, 4.0)).show(ui, |ui| {
            for entry in listed {
                let root = entry.id == ROOT_COMPONENT;
                let label = if root { format!("{} (root)", entry.name) } else { entry.name.clone() };
                if ui.add(egui::Button::new(label).frame(false)).on_hover_text("Open it in a tab").clicked() {
                    *asked = Some(Ask::Pick(entry.id));
                    ui.close();
                }
                let place = ui.add_enabled(entry.refusal.is_none(), egui::Button::new("Place").small());
                let place = match &entry.refusal {
                    Some(why) => place.on_disabled_hover_text(why),
                    None => place.on_hover_text("Place it in the component being edited, where a new shape would go"),
                };
                if place.clicked() {
                    *asked = Some(Ask::Place(entry.id));
                    ui.close();
                }
                if root {
                    ui.label("");
                } else if ui
                    .small_button("Delete")
                    .on_hover_text("Delete the component, and every place it is used")
                    .clicked()
                {
                    *asked = Some(Ask::Delete(entry.id));
                    ui.close();
                }
                ui.end_row();
            }
        });
    });
}
