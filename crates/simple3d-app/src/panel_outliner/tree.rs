//! The tree of rows, and which of them are on screen.

use super::*;
use crate::app::{App, Carried, DropTarget};
use crate::theme::{self, token};
use simple3d_core::scene::NodeId;

/// The outliner's contents, without the dock around them.
pub fn show_inside(app: &mut App, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(theme::metric::GAP, 2.0);
    // The count sits at the foot: above the tree it shifted every row when the selection changed.
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        selection_line(app, ui);
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| tree(app, ui));
    });
}

pub(crate) fn tree(app: &mut App, ui: &mut egui::Ui) {
    confirm_strip(app, ui);

    let dragging = app.outliner_drag;
    // The whole dragged load, computed once per frame (issue 43). A palette drag carries no rows,
    // which is why drag state is asked of `dragging`, not this.
    let carried: Vec<NodeId> = match dragging {
        Some(Carried::Rows(source)) => app.dragged_nodes(source),
        _ => Vec::new(),
    };
    app.drop_target = None;
    let ctx = ui.ctx().clone();
    let (area, restore) = theme::list_scroll_area(ui);
    // Named, since an auto id would change as the "N selected" line comes and goes. Scrolls sideways
    // so long names are reachable rather than wrapped (issue 50).
    area.scroll([true, true]).id_salt("outliner-tree").show(ui, |ui| {
        ui.set_style(restore);
        ui.add_space(2.0);
        let ids = visible_rows(app);
        // Every row as wide as the widest, so the eye and badges align and the tint covers a full row.
        let widest = ids.iter().map(|&id| row_width(app, ui, id)).fold(0.0_f32, f32::max);
        let width = row_run(ui.available_width(), widest);
        for &id in &ids {
            // A row's context menu can delete nodes mid-loop (a pattern goes at once, with its children),
            // and drawing a deleted row crashed. Skip it; the next frame's list is correct.
            if !app.scene.contains(id) {
                continue;
            }
            // Dragged rows stay in place as faded shadows, so the tree does not reflow under the pointer
            // (issue 46).
            let shadowed = carried.iter().any(|&source| id == source || app.scene.is_ancestor_of(source, id));
            row(app, ui, id, &carried, dragging.is_some(), shadowed, width);
        }
        // The space below the tree drops at the end of the root.
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, drop_zone(ui.available_height())), egui::Sense::hover());
        // `contains_pointer`, not `hovered`: egui withholds hover during a drag, so the indicator only
        // showed on release (issue 43).
        if dragging.is_some() && response.contains_pointer() {
            let root = app.scene.root();
            app.drop_target = Some(DropTarget { parent: root, index: app.scene.node(root).children.len(), into: None });
            ui.painter().hline(rect.x_range(), rect.top() + 1.0, egui::Stroke::new(2.0_f32, token::ACCENT));
        }
    });

    if let Some(load) = dragging {
        drag_ghost(app, &ctx, load, carried.len());
    }

    // Finish the drag on release, wherever the pointer ended up.
    if dragging.is_some() && ctx.input(|i| i.pointer.any_released()) {
        finish_drag(app);
    }
}

/// The width every row is drawn at: the widest row if it overflows, else the viewport less a pixel.
///
/// The missing pixel prevents scrollbar flicker (issue 101): content exactly as wide as the
/// viewport made the horizontal and vertical bars toggle each other every frame.
pub(crate) fn row_run(available: f32, widest: f32) -> f32 {
    if widest > available {
        widest
    } else {
        (available - 1.0).max(0.0)
    }
}

/// The height of the drop strip below the last row: the leftover height, or 24 px once the tree
/// scrolls. A fixed minimum overflowed a fitting tree and fed the loop in `row_run` (issue 101).
pub(crate) fn drop_zone(available: f32) -> f32 {
    if available > 0.0 {
        available
    } else {
        24.0
    }
}

/// The width a row needs to show everything; the tree uses the widest so names can be scrolled
/// to (issue 50).
pub(crate) fn row_width(app: &App, ui: &egui::Ui, id: NodeId) -> f32 {
    let node = app.scene.node(id);
    let name = ui.fonts(|fonts| {
        fonts.layout_no_wrap(node.name.clone(), egui::FontId::proportional(theme::font::VALUE), token::TEXT_HI).size().x
    });
    // Margin, indent, twisty, glyph, name, one column per badge, and the eye's 28 px.
    let badges = operator_badge(&app.scene, id).is_some() as i32 + app.evaluated.error_for(id).is_some() as i32;
    6.0 + app.scene.depth(id) as f32 * 12.0 + 14.0 + 18.0 + name + badges as f32 * 20.0 + 28.0
}

/// The rows the tree shows: depth-first, skipping collapsed groups' descendants, which are not
/// drawn at all and so cannot be hit by accident.
pub fn visible_rows(app: &App) -> Vec<NodeId> {
    let mut out = Vec::new();
    push_visible(app, app.scene.root(), &mut out);
    out
}

pub(crate) fn push_visible(app: &App, id: NodeId, out: &mut Vec<NodeId>) {
    out.push(id);
    if app.collapsed.contains(&id) {
        return;
    }
    // `row_children`: a collection is one row, and only its extracted pieces are walked (issue 82).
    for child in app.scene.row_children(id) {
        push_visible(app, child, out);
    }
}
