//! The primitive palette: the shapes, as silhouettes, under the outliner.
//!
//! The Add menu still exists and still lists every one of these, but a menu is
//! two gestures and a read. A grid of silhouettes is one gesture and a glance,
//! which is what the five-second budget for an operation actually needs.

use crate::app::App;
use crate::icon::{self, Glyph};
use crate::theme::{self, token};
use simple3d_core::primitive;
use simple3d_core::scene::GroupOp;

/// Side of one palette tile.
const TILE: f32 = 26.0;

/// How many tiles fit across a palette `width` wide, never fewer than one.
/// Eight is the design's row length; a narrower dock gets fewer rather than a
/// horizontal scrollbar.
pub fn columns(width: f32) -> usize {
    let usable = width - 2.0 * theme::metric::PANEL_PAD;
    ((usable / (TILE + theme::metric::GAP)).floor() as usize).clamp(1, 8)
}

pub fn show_inside(app: &mut App, ui: &mut egui::Ui) {
    // An empty document opens every category: with nothing in the scene, the
    // shapes are the only thing worth reading.
    let empty = app.scene.node(app.scene.root()).children.is_empty();
    ui.spacing_mut().item_spacing = egui::vec2(theme::metric::GAP, theme::metric::GAP);
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        landing_hint(app, ui);
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| shapes(app, ui, empty));
    });
}

/// The shapes themselves, in the space the hint has left.
fn shapes(app: &mut App, ui: &mut egui::Ui, empty: bool) {
    let (area, restore) = theme::list_scroll_area(ui);
    area.show(ui, |ui| {
        ui.set_style(restore);
        ui.add_space(2.0);
        for category in primitive::categories() {
            category_block(app, ui, category, empty);
        }
        saved_block(app, ui);
        ui.add_space(4.0);
    });
}

/// Where a new shape lands, said in words, pinned to the foot of the panel.
///
/// It is drawn before the shapes and from the bottom up, so it keeps its place
/// against the panel's own edge instead of floating directly under the last row
/// of tiles -- a line that moves every time a category is folded is a line
/// nobody reads. The choice it reports lives in the document options, beside
/// the rest of what the document does to a new shape.
fn landing_hint(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(theme::metric::PANEL_PAD);
        ui.add(egui::Label::new(theme::hint(crate::app::insertion_hint(app))).selectable(false));
    });
    ui.add_space(4.0);
}

/// Groups and whole projects the user has kept for reuse. Empty until something
/// has been saved, and then it is the first place to look -- so it goes at the
/// end of the palette, where a growing list can grow.
fn saved_block(app: &mut App, ui: &mut egui::Ui) {
    if app.library.is_empty() {
        return;
    }
    let full = ui.available_width();
    let (bar, _) = ui.allocate_exact_size(egui::vec2(full, 18.0), egui::Sense::hover());
    ui.painter().text(
        egui::pos2(bar.left() + theme::metric::PANEL_PAD + 12.0, bar.center().y),
        egui::Align2::LEFT_CENTER,
        "Saved",
        egui::FontId::proportional(theme::font::SMALL),
        token::TEXT_LO,
    );

    let mut add: Option<usize> = None;
    let mut forget: Option<usize> = None;
    for (index, entry) in app.library.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.add_space(theme::metric::PANEL_PAD);
            let response = ui
                .add(egui::Button::new(theme::value(entry.name.clone())).min_size(egui::vec2(full - 40.0, 20.0)))
                .on_hover_text("Add it to the scene. Right-click to remove it from the palette.");
            if response.clicked() {
                add = Some(index);
            }
            response.context_menu(|ui| {
                if ui.button("Remove from the palette").clicked() {
                    forget = Some(index);
                    ui.close();
                }
            });
        });
    }
    if let Some(index) = add {
        let entry = app.library[index].clone();
        app.add_library_entry(&entry);
    }
    if let Some(index) = forget {
        let entry = app.library[index].clone();
        app.delete_library_entry(&entry);
    }
}

fn category_block(app: &mut App, ui: &mut egui::Ui, category: &'static str, force_open: bool) {
    let id = ui.id().with(("palette", category));
    let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(true) || force_open;

    let full = ui.available_width();
    let (bar, response) = ui.allocate_exact_size(egui::vec2(full, 18.0), egui::Sense::click());
    if response.clicked() && !force_open {
        open = !open;
        ui.data_mut(|d| d.insert_temp(id, open));
    }
    let painter = ui.painter();
    let colour = if response.hovered() { token::TEXT_HI } else { token::TEXT_LO };
    // The twisty is drawn rather than typed: the bundled face has no triangle
    // glyph, and a tofu box in front of every category is worse than none.
    theme::twisty(&painter, egui::pos2(bar.left() + theme::metric::PANEL_PAD + 3.0, bar.center().y), open, colour);
    painter.text(
        egui::pos2(bar.left() + theme::metric::PANEL_PAD + 12.0, bar.center().y),
        egui::Align2::LEFT_CENTER,
        category,
        egui::FontId::proportional(theme::font::SMALL),
        colour,
    );
    if !open {
        return;
    }

    let specs: Vec<&primitive::PrimitiveSpec> = primitive::REGISTRY.iter().filter(|s| s.category == category).collect();
    let per_row = columns(full);
    for chunk in specs.chunks(per_row) {
        ui.horizontal(|ui| {
            ui.add_space(theme::metric::PANEL_PAD - theme::metric::GAP);
            for spec in chunk {
                let hint = crate::app::insertion_hint(app);
                let response = icon::button(ui, Glyph::for_primitive(spec.type_id), TILE, false, true)
                    .on_hover_text(format!("{}\n{hint}", spec.label));
                if response.clicked() {
                    app.add_node(Some(spec.type_id), GroupOp::Union);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_never_asks_for_more_columns_than_the_design_allows() {
        assert_eq!(columns(1000.0), 8);
        assert_eq!(columns(260.0), 8);
    }

    #[test]
    fn a_narrow_dock_gets_fewer_tiles_rather_than_a_scrollbar() {
        assert!(columns(120.0) < 8);
        // Even squeezed to nothing there is always one column, so no shape
        // becomes unreachable.
        assert_eq!(columns(0.0), 1);
        assert_eq!(columns(-50.0), 1);
    }

    #[test]
    fn every_category_in_the_registry_ends_up_on_the_palette() {
        // The palette iterates categories and filters the registry by them; a
        // primitive whose category is not in the list would never be drawn.
        let categories = primitive::categories();
        for spec in primitive::REGISTRY {
            assert!(categories.contains(&spec.category), "{} is in no palette category", spec.type_id);
        }
    }
}
