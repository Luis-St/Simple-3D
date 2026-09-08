//! One row: its name, its badges and its buttons.

use super::*;
use crate::app::{App, Carried};
use crate::icon::{self, Glyph};
use crate::theme::{self, metric, token};
use simple3d_core::scene::NodeId;

/// A row's own id, rather than one counted off the widgets drawn before it.
///
/// A right-click opens the context menu by writing the row's id into egui's
/// memory and reading it back on the *next* frame -- and the menu's first act is
/// to select the row it was opened on. With an automatic id, that selection
/// renumbered the row (the header above it appears, the properties panel fills)
/// and the menu was looked up under an id nothing had drawn, so it closed again
/// before it was ever seen. The row is the same row whatever else is on screen,
/// and now says so.
pub fn row_id(id: NodeId) -> egui::Id {
    egui::Id::new(("outliner-row", id))
}

/// How much of itself a row keeps while it is being dragged. Faint enough to
/// read as held rather than as sitting there, solid enough to still name what
/// is on the pointer.
pub(crate) const DRAG_SHADOW: f32 = 0.38;

pub(crate) fn row(
    app: &mut App,
    ui: &mut egui::Ui,
    id: NodeId,
    carried: &[NodeId],
    dragging: bool,
    shadowed: bool,
    width: f32,
) {
    let depth = app.scene.depth(id);
    let node = app.scene.node(id);
    let name = node.name.clone();
    let visible = node.visible;
    let is_group = node.is_group();
    let is_root = id == app.scene.root();
    let selected = app.is_selected(id);
    let failed = app.evaluated.error_for(id).is_some();
    let badge = operator_badge(&app.scene, id);

    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, metric::ROW), egui::Sense::hover());
    // A shadowed row is a picture of where the load came from and nothing else:
    // it cannot be clicked, hovered, renamed, dropped on or dragged again while
    // it is in the air.
    let response =
        ui.interact(rect, row_id(id), if shadowed { egui::Sense::hover() } else { egui::Sense::click_and_drag() });

    // Background: selection is an accent tint with a bar at the left edge, and
    // every selected row is drawn the same. Lighting the most recent one
    // brighter than the rest made a multi-selection look like one row selected
    // and the others merely marked, when they are all equally selected and
    // everything acts on all of them (issue 45).
    let mut painter = ui.painter().clone();
    if shadowed {
        painter.multiply_opacity(DRAG_SHADOW);
    }
    if selected {
        painter.rect_filled(rect, 0.0, token::ACCENT.gamma_multiply(0.13));
        painter.rect_filled(
            egui::Rect::from_min_size(rect.left_top(), egui::vec2(2.0, rect.height())),
            0.0,
            token::ACCENT.gamma_multiply(0.5),
        );
    } else if response.hovered() && !shadowed {
        painter.rect_filled(rect, 0.0, token::SURFACE_3);
    }

    // Visibility lives at the right edge, always drawn, dimmed rather than
    // hidden when the node is not visible.
    let eye_rect =
        egui::Rect::from_min_size(egui::pos2(rect.right() - 22.0, rect.top() + 3.0), egui::Vec2::splat(16.0));
    let eye = ui.interact(
        eye_rect,
        ui.id().with((id, "eye")),
        if shadowed { egui::Sense::hover() } else { egui::Sense::click() },
    );
    if !is_root {
        let colour = if eye.hovered() && !shadowed {
            token::TEXT_HI
        } else if visible {
            token::TEXT_LO
        } else {
            token::TEXT_LO.gamma_multiply(0.45)
        };
        icon::draw(&painter, eye_rect.shrink(1.0), if visible { Glyph::Eye } else { Glyph::EyeOff }, colour);
        if eye.clicked() && !shadowed {
            app.edit("Toggle visibility", None);
            if let Some(node) = app.scene.get_mut(id) {
                node.visible = !visible;
            }
        }
    }

    let mut x = rect.left() + 6.0 + depth as f32 * 12.0;

    // The twisty, for anything that holds children. Its column is reserved on
    // every row, childless ones included, so the glyphs and names below a
    // group still line up under the ones above it.
    let twisty_rect = egui::Rect::from_min_size(egui::pos2(x, rect.top() + 4.0), egui::Vec2::splat(14.0));
    // A collection with nothing extracted from it has no rows under it and so
    // no twisty, however many pieces it holds (issue 82).
    let has_children = !app.scene.row_children(id).is_empty();
    let mut collapsed = app.collapsed.contains(&id);
    let mut twisty_hovered = false;
    if has_children {
        let twisty = ui.interact(
            twisty_rect,
            ui.id().with((id, "twisty")),
            if shadowed { egui::Sense::hover() } else { egui::Sense::click() },
        );
        twisty_hovered = twisty.hovered() && !shadowed;
        theme::twisty(
            &painter,
            twisty_rect.center(),
            !collapsed,
            if twisty.hovered() { token::TEXT_HI } else { token::TEXT_LO },
        );
        if twisty.clicked() && !shadowed {
            collapsed = !collapsed;
            app.set_collapsed(id, collapsed);
        }
    }
    x += 14.0;

    // Type glyph: a bracket for a group, a solid mark for a shape.
    let glyph_rect = egui::Rect::from_min_size(egui::pos2(x, rect.top() + 4.0), egui::Vec2::splat(14.0));
    let glyph = node_glyph(app.scene.node(id));
    let glyph_colour = if failed {
        token::DANGER
    } else if !visible {
        token::TEXT_LO.gamma_multiply(0.5)
    } else if selected {
        token::ACCENT
    } else {
        token::TEXT_LO
    };
    icon::draw(&painter, glyph_rect, glyph, glyph_colour);
    x += 18.0;

    // Rename in place owns the rest of the row while it is open.
    if let Some((rename_id, buffer)) = &mut app.rename {
        if *rename_id == id {
            let field = egui::Rect::from_min_max(
                egui::pos2(x, rect.top() + 1.0),
                egui::pos2(eye_rect.left() - 4.0, rect.bottom() - 1.0),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(field));
            let response =
                child.add(egui::TextEdit::singleline(buffer).desired_width(f32::INFINITY).font(egui::TextStyle::Body));
            response.request_focus();
            if response.lost_focus() || child.input(|i| i.key_pressed(egui::Key::Enter)) {
                let new_name = buffer.trim().to_string();
                app.rename = None;
                if !new_name.is_empty() && new_name != name {
                    app.edit("Rename", None);
                    if let Some(node) = app.scene.get_mut(id) {
                        node.name = new_name;
                    }
                }
            }
            return;
        }
    }

    // The operator badge sits between the name and the eye, right-aligned, so a
    // column of badges lines up down the tree.
    let mut name_right = eye_rect.left() - 6.0;
    if let Some((mark, subtracted)) = badge {
        let colour = if subtracted { token::DANGER } else { token::TEXT_LO.gamma_multiply(0.8) };
        let badge_rect =
            egui::Rect::from_min_size(egui::pos2(name_right - 14.0, rect.top() + 4.0), egui::Vec2::splat(14.0));
        icon::draw(&painter, badge_rect, mark, colour);
        name_right = badge_rect.left() - 6.0;
    }
    if failed {
        let warn = egui::Rect::from_min_size(egui::pos2(name_right - 14.0, rect.top() + 4.0), egui::Vec2::splat(14.0));
        icon::draw(&painter, warn, Glyph::Warning, token::DANGER);
        name_right = warn.left() - 6.0;
    }

    let text_colour = if failed {
        token::DANGER
    } else if !visible {
        token::TEXT_LO.gamma_multiply(0.6)
    } else if selected {
        token::TEXT_HI
    } else {
        token::TEXT_HI.gamma_multiply(0.85)
    };
    // Laid out without wrapping: the row is a fixed 22 px, so a wrapped name
    // is a name with its second half cut off. The tree is made wide enough for
    // the longest one instead, and scrolls sideways to reach it (issue 50).
    let galley = painter.layout_no_wrap(name.clone(), egui::FontId::proportional(theme::font::VALUE), text_colour);
    // Clipped at the badge column rather than wrapped, so that the name never
    // runs under a badge even on the frame a rename makes it longer than the
    // width the tree was measured for.
    let mut text = painter.clone();
    text.set_clip_rect(
        painter.clip_rect().intersect(egui::Rect::from_min_max(rect.left_top(), egui::pos2(name_right, rect.bottom()))),
    );
    text.galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, text_colour);

    let response = if shadowed {
        response
    } else {
        response.on_hover_text(hover_text(
            app,
            id,
            is_group,
            app.scene.node(id).group_op(),
            app.scene.difference_base(id),
        ))
    };

    // The eye and the twisty own their own clicks: pressing either must not
    // also select the row.
    if !shadowed && !eye.hovered() && !twisty_hovered {
        // Whether the click before this one was on this same row. Read before
        // it is overwritten below, because the second click of a double click
        // reports both `clicked` and `double_clicked` on the one frame.
        let same_row_again = app.outliner_last_click == Some(id);
        if response.clicked() {
            app.outliner_last_click = Some(id);
            let modifiers = ui.input(|i| i.modifiers);
            // Shift extends from the anchor over the rows on screen, Ctrl (Cmd
            // on macOS) adds or removes the one row, and a plain click starts
            // again from it -- the three modes every list has (issue 60).
            if modifiers.shift {
                let rows = visible_rows(app);
                app.select_range_to(id, &rows);
            } else if modifiers.command {
                app.toggle_selected(id);
            } else {
                app.select_only(id);
            }
        }
        // Both clicks have to have been on this row: egui reports a double
        // click from the delay alone, so without this a quick click on one row
        // followed by a click on another opened a rename on the second one
        // (issue 59). A modifier means the two clicks were building a
        // selection, which is not a request to rename anything either.
        if response.double_clicked() && same_row_again && !is_root && !ui.input(|i| i.modifiers.any()) {
            app.rename = Some((id, name.clone()));
        }
        if response.drag_started() && !is_root {
            app.outliner_drag = Some(Carried::Rows(id));
        }
    }

    if !shadowed {
        context_menu(app, &response, id, is_root);
    }

    // Drop indicator: which group would take the load, or which gap it would
    // land in. Both are drawn on the row under the pointer, because that is
    // where the pointer is looking (issue 43).
    // The band a row answers for is its rect grown by half the spacing above and
    // below, so the strip between two rows belongs to one of them rather than to
    // neither: read off the row alone, the indicator blinked out every time the
    // pointer crossed a gap (issue 48).
    let half_gap = ui.spacing().item_spacing.y / 2.0;
    let band = rect.expand2(egui::vec2(0.0, half_gap));
    if dragging && !shadowed && ui.rect_contains_pointer(band) {
        let pointer = ui.input(|i| i.pointer.hover_pos()).unwrap_or(band.center());
        let fraction = ((pointer.y - band.top()) / band.height().max(1.0)).clamp(0.0, 1.0);
        let root = app.scene.root();
        // "Open" means rows are drawn under this one, which for a collection is
        // the extracted pieces alone -- one holding thousands with none of them
        // extracted has nothing under it and its gap still means "beside"
        // (issue 82).
        let open = !app.collapsed.contains(&id) && !app.scene.row_children(id).is_empty();
        if let Some(target) = drop_position(&app.scene, id, fraction, root, open) {
            if drop_is_legal(&app.scene, carried, &target) {
                app.drop_target = Some(target);
                let stroke = egui::Stroke::new(2.0_f32, token::ACCENT);
                match target.into {
                    // Into a group: the whole row is lit, not merely outlined,
                    // so "inside this one" and "next to it" cannot be confused
                    // at a glance.
                    Some(_) => {
                        painter.rect_filled(rect, 2.0, token::ACCENT.gamma_multiply(0.22));
                        painter.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
                    }
                    // Between siblings: a line in the gap it would land in,
                    // indented to the depth it would land at and tipped with a
                    // marker, so the line belongs to a level and not merely to
                    // a pair of rows.
                    None => {
                        // Which side of this row the line goes on is read back
                        // out of the target, not guessed from the pointer again:
                        // the two used different thresholds, and the band
                        // between them drew the line under a row the drop was
                        // going above.
                        let own = app.scene.node(target.parent).children.iter().position(|&c| c == id);
                        // Landing as this group's own first child is a line
                        // under its row, not over it: the row is the parent of
                        // the gap, not a sibling beside it (issue 49).
                        let after = target.parent == id || own.is_some_and(|index| target.index > index);
                        let y = gap_line_y(rect, half_gap * 2.0, after);
                        // A child of `target.parent` sits one level in from it,
                        // which is the level the line has to sit at.
                        let left = rect.left() + 6.0 + (app.scene.depth(target.parent) + 1) as f32 * 12.0;
                        painter.hline(left..=rect.right(), y, stroke);
                        painter.circle_filled(egui::pos2(left + 1.0, y), 3.0, token::ACCENT);
                    }
                }
            } else {
                // Cycles are prevented, and the indicator says so rather than
                // letting the user find out on release.
                painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0_f32, token::DANGER), egui::StrokeKind::Inside);
            }
        }
    }
}
