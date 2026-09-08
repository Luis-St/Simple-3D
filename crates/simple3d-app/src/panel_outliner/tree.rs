//! The tree of rows, and which of them are on screen.

use super::*;
use crate::app::{App, Carried, DropTarget};
use crate::theme::{self, token};
use simple3d_core::scene::NodeId;

/// The outliner's contents, without a dock around them: the dock owns the
/// header bar and decides which side of the window this is on.
pub fn show_inside(app: &mut App, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(theme::metric::GAP, 2.0);
    // The count sits at the foot of the panel: appearing and disappearing above
    // the tree pushed every row down a line the moment anything was selected,
    // so the row under the pointer was no longer the row that had been clicked.
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        selection_line(app, ui);
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| tree(app, ui));
    });
}

pub(crate) fn tree(app: &mut App, ui: &mut egui::Ui) {
    confirm_strip(app, ui);

    let dragging = app.outliner_drag;
    // The whole load, worked out once a frame: every row it holds leaves the
    // tree, the legality of a drop is judged against all of it, and the ghost
    // says how much is on the pointer (issue 43). A shape dragged out of the
    // palette carries no rows at all -- it is not in the tree yet -- which is
    // why "is a drag running" is asked of `dragging` and never of this.
    let carried: Vec<NodeId> = match dragging {
        Some(Carried::Rows(source)) => app.dragged_nodes(source),
        _ => Vec::new(),
    };
    app.drop_target = None;
    let ctx = ui.ctx().clone();
    let (area, restore) = theme::list_scroll_area(ui);
    // Named, because an automatic id is a count of the widgets drawn before it
    // -- and the "N selected" line above appears and disappears with the
    // selection, which would renumber every row inside on the frame a selection
    // changed.
    //
    // It scrolls sideways as well as down: a name is a name, so a narrow panel
    // is a reason to reach the rest of it, not to break it over two lines that
    // a 22 px row then cuts in half (issue 50).
    area.scroll([true, true]).id_salt("outliner-tree").show(ui, |ui| {
        ui.set_style(restore);
        ui.add_space(2.0);
        let ids = visible_rows(app);
        // Every row is as wide as the widest one, so the eye and the operator
        // badge stay in a column and the selection tint covers a whole row.
        let width = ids.iter().map(|&id| row_width(app, ui, id)).fold(ui.available_width(), f32::max);
        for &id in &ids {
            // The list was taken before any of it was drawn, and a row can
            // delete nodes while the loop is still running: its own context menu
            // does, and a pattern is not a group, so deleting one asks nothing
            // and goes at once -- taking its children with it. Those children
            // are the very next entries here, and drawing one crashed the
            // application on the row after the delete.
            //
            // Skipped rather than re-derived: the rows of a subtree that is
            // already gone have nothing to draw, and the next frame's list is
            // right without anything being rebuilt mid-frame.
            if !app.scene.contains(id) {
                continue;
            }
            // What is being dragged stays in the tree, drawn as a shadow of
            // itself: taking the rows out re-flowed everything below them the
            // moment a drag started, so the gaps the drop line points at moved
            // out from under the pointer as it was being aimed. Left in place
            // and faded, the tree holds still, the shadow says where the load
            // came from and the slab on the pointer says it is held (issue 46).
            let shadowed = carried.iter().any(|&source| id == source || app.scene.is_ancestor_of(source, id));
            row(app, ui, id, &carried, dragging.is_some(), shadowed, width);
        }
        // Dropping in the empty space below the tree means "at the end of the
        // root", which is otherwise awkward to reach.
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, ui.available_height().max(24.0)), egui::Sense::hover());
        // `contains_pointer`, not `hovered`: egui reserves hovering for a frame
        // where nothing is being dragged, which is every frame of a drag but the
        // one it ends on. Asking the wrong question is why the drop indicator
        // only ever appeared for the single frame of the release, if at all
        // (issue 43).
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

/// How wide a row has to be to show everything on it: the indent, the twisty
/// and glyph columns, the name at its natural width, whatever badges it wears,
/// and the eye at the right edge.
///
/// The tree takes the widest of these rather than the panel's width, which is
/// what lets a name be read in full in a narrow panel by scrolling to it
/// (issue 50).
pub(crate) fn row_width(app: &App, ui: &egui::Ui, id: NodeId) -> f32 {
    let node = app.scene.node(id);
    let name = ui.fonts(|fonts| {
        fonts.layout_no_wrap(node.name.clone(), egui::FontId::proportional(theme::font::VALUE), token::TEXT_HI).size().x
    });
    // 6 px of margin, the indent, the twisty, the type glyph, the name, then
    // one column per badge and the eye's 28 px at the right edge.
    let badges = operator_badge(&app.scene, id).is_some() as i32 + app.evaluated.error_for(id).is_some() as i32;
    6.0 + app.scene.depth(id) as f32 * 12.0 + 14.0 + 18.0 + name + badges as f32 * 20.0 + 28.0
}

/// The rows the tree shows: depth-first, minus everything under a group that
/// has been collapsed.
///
/// A collapsed group's children are not merely hidden here -- they are not
/// drawn at all -- so nothing below one can be clicked, dropped on or moved by
/// accident while it is shut.
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
    // `row_children` rather than `children`: a collection keeps its pieces
    // inside itself and is one row however many thousand it is in, so only the
    // ones extracted from it are walked (issue 82).
    for child in app.scene.row_children(id) {
        push_visible(app, child, out);
    }
}
