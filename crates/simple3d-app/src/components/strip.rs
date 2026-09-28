//! The second tab row: the components of the project on screen.

use super::*;
use crate::app::App;
use crate::icon::{self, Glyph};
use crate::tabs::strip::{paint_tab, plus, plus_glyph, TabLook};
use crate::theme::{self, metric, token};

mod list;
use list::list;

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
