//! The list of pieces inside a collection.

use crate::app::App;
use crate::theme::{self, token};
use simple3d_core::scene::NodeId;

/// A tiling in one line: what shape the cells are, how big, which way they run
/// and whether they were cut into layers as well.
/// A collection's pieces, as a list that can be picked through (issue 82).
///
/// The collection is one row in the outliner however many thousand pieces it
/// holds, so this list is where a piece is reached at all: ticked here or by
/// pointing at it in the viewport, and then extracted -- given a row of its own
/// under the collection -- or put back inside.
///
/// Virtualised, because "however many thousand" is meant literally: a hexagon
/// tiling over a plate is a few thousand pieces, and a list that lays out every
/// name every frame is a panel that stops the application whether or not
/// anybody scrolls it.
pub(crate) fn pieces_list(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let pieces = app.scene.node(id).children.clone();
    if pieces.is_empty() {
        ui.add(egui::Label::new(theme::hint("This holds no pieces at all.")).selectable(false));
        return;
    }
    let extracted = pieces.iter().filter(|&&c| app.scene.node(c).extracted).count();
    ui.add(
        egui::Label::new(theme::hint(match extracted {
            0 => format!("{} pieces, all held inside", pieces.len()),
            n => format!("{} pieces, {n} extracted into the tree", pieces.len()),
        }))
        .selectable(false),
    );
    ui.add_space(theme::metric::GAP);

    let row = theme::metric::ROW;
    // Ten rows of room, or fewer when there are fewer: enough to pick through
    // without the list taking the panel over from the transform below it.
    let rows = pieces.len().min(VISIBLE_PIECE_ROWS);
    egui::Frame::NONE.fill(token::SURFACE_0B).corner_radius(3.0).inner_margin(egui::Margin::same(2)).show(ui, |ui| {
        // The panel's own scroll area, not a bare one: the handle takes its
        // colour from the style, and the default leaves it invisible until it
        // is being dragged -- a list of a thousand pieces with no sign that it
        // scrolls at all.
        let (area, restore) = theme::list_scroll_area(ui);
        area.id_salt(("pieces", id)).max_height(rows as f32 * row).show_rows(ui, row, pieces.len(), |ui, range| {
            ui.set_style(restore);
            ui.set_width(ui.available_width());
            for index in range {
                piece_row(app, ui, pieces[index], row);
            }
        });
    });

    ui.add_space(theme::metric::GAP);
    let ticked = app.piece_ticks.len();
    ui.horizontal_wrapped(|ui| {
        // Select, not tick: the box on a row is what the pointer ticks, and a
        // button called "Tick all" reads as one that ticks every box rather
        // than as one that selects every piece -- which is what it does, and
        // what the viewport shows outlined while it holds.
        if ui.add_enabled(ticked < pieces.len(), egui::Button::new("Select all")).clicked() {
            app.piece_ticks = pieces.iter().copied().collect();
        }
        if ui.add_enabled(ticked > 0, egui::Button::new("Select none")).clicked() {
            app.piece_ticks.clear();
        }
    });
    ui.add_space(theme::metric::GAP);
    let ticked_inside = app.piece_ticks.iter().any(|&c| !app.scene.node(c).extracted);
    let ticked_outside = app.piece_ticks.iter().any(|&c| app.scene.node(c).extracted);
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(ticked_inside, egui::Button::new("Extract"))
            .on_hover_text("Give each ticked piece a row of its own under this collection.")
            .clicked()
        {
            app.extract_ticked_pieces(id);
        }
        if ui
            .add_enabled(ticked_outside, egui::Button::new("Put back"))
            .on_hover_text("Fold the ticked pieces back inside, so they stop taking a row in the tree.")
            .clicked()
        {
            app.return_ticked_pieces(id);
        }
        if ui
            .add_enabled(extracted < pieces.len(), egui::Button::new("Extract all\u{2026}"))
            .on_hover_text("Empty the collection. What is left is an ordinary union group of the pieces.")
            .clicked()
        {
            app.ask_to_extract_all(id);
        }
    });
}

/// How many pieces the list shows without scrolling.
pub(crate) const VISIBLE_PIECE_ROWS: usize = 10;

/// One piece in the list: a tick, its name, and a mark on the ones that have
/// been extracted.
pub(crate) fn piece_row(app: &mut App, ui: &mut egui::Ui, id: NodeId, height: f32) {
    let Some(node) = app.scene.get(id) else { return };
    let (name, extracted, visible) = (node.name.clone(), node.extracted, node.visible);
    let ticked = app.piece_ticks.contains(&id);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click());
    let painter = ui.painter_at(rect);
    if ticked {
        painter.rect_filled(rect, 2.0, token::ACCENT.gamma_multiply(0.13));
    } else if response.hovered() {
        painter.rect_filled(rect, 2.0, token::SURFACE_2);
    }

    let box_rect = egui::Rect::from_min_size(egui::pos2(rect.left() + 4.0, rect.top() + 5.0), egui::Vec2::splat(12.0));
    painter.rect_stroke(
        box_rect,
        2.0,
        egui::Stroke::new(1.0_f32, if ticked { token::ACCENT } else { token::TEXT_LO.gamma_multiply(0.6) }),
        egui::StrokeKind::Inside,
    );
    if ticked {
        painter.rect_filled(box_rect.shrink(3.0), 1.0, token::ACCENT);
    }

    // The mark on an extracted piece is the same one the outliner puts on a
    // split, turned round: it says this one is out in the tree.
    let mut right = rect.right() - 4.0;
    if extracted {
        let mark = egui::Rect::from_min_size(egui::pos2(right - 14.0, rect.top() + 4.0), egui::Vec2::splat(14.0));
        crate::icon::draw(&painter, mark, crate::icon::Glyph::Split, token::TEXT_LO);
        right = mark.left() - 4.0;
    }

    let colour = if !visible {
        token::TEXT_LO.gamma_multiply(0.6)
    } else if ticked {
        token::TEXT_HI
    } else {
        token::TEXT_HI.gamma_multiply(0.85)
    };
    let galley = painter.layout_no_wrap(name, egui::FontId::proportional(theme::font::VALUE), colour);
    let mut text = painter.clone();
    text.set_clip_rect(
        painter.clip_rect().intersect(egui::Rect::from_min_max(rect.left_top(), egui::pos2(right, rect.bottom()))),
    );
    text.galley(egui::pos2(box_rect.right() + 6.0, rect.center().y - galley.size().y / 2.0), galley, colour);

    if response.clicked() {
        let adding = ui.input(|i| i.modifiers.command || i.modifiers.shift);
        app.tick_piece(id, adding);
    }
    response.on_hover_text(if extracted {
        "Extracted: it has a row of its own in the tree."
    } else {
        "Held inside the collection. Tick it and press Extract to give it a row."
    });
}
