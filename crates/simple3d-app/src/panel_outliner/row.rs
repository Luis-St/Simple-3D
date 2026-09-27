//! One outliner row: its name, badges and buttons.

use super::*;
use crate::app::{App, Carried};
use crate::icon::{self, Glyph};
use crate::theme::{self, metric, token};
use simple3d_core::scene::NodeId;

/// A stable id per row. The context menu is looked up by id on the next frame, and its selecting
/// the row renumbered auto ids, so the menu closed before it was seen.
pub fn row_id(id: NodeId) -> egui::Id {
    egui::Id::new(("outliner-row", id))
}

/// The opacity of a row while it is being dragged.
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
    // A shadowed row is only a picture of where the drag came from, and takes no input.
    let response =
        ui.interact(rect, row_id(id), if shadowed { egui::Sense::hover() } else { egui::Sense::click_and_drag() });

    // Every selected row is drawn the same; highlighting the latest made a multi-selection look
    // like one (issue 45).
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

    // Visibility at the right edge, always drawn, dimmed when hidden.
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

    // The twisty column is reserved on every row so glyphs and names stay aligned.
    let twisty_rect = egui::Rect::from_min_size(egui::pos2(x, rect.top() + 4.0), egui::Vec2::splat(14.0));
    // A collection with nothing extracted has no rows under it and so no twisty (issue 82).
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

    // Rename in place owns the rest of the row while open.
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

    // The operator badge is right-aligned so badges line up down the tree.
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
    // Not wrapped, since rows have a fixed height; the tree is widened and scrolls instead (issue 50).
    let galley = painter.layout_no_wrap(name.clone(), egui::FontId::proportional(theme::font::VALUE), text_colour);
    // Clipped at the badge column, so a name growing during rename never runs under a badge.
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

    // The eye and the twisty take their own clicks without selecting the row.
    if !shadowed && !eye.hovered() && !twisty_hovered {
        // Read before being overwritten below, since a double click's second click reports both
        // `clicked` and `double_clicked` in one frame.
        let same_row_again = app.outliner_last_click == Some(id);
        if response.clicked() {
            app.outliner_last_click = Some(id);
            let modifiers = ui.input(|i| i.modifiers);
            // Shift extends from the anchor, Ctrl (Cmd on macOS) toggles, a plain click restarts (issue 60).
            if modifiers.shift {
                let rows = visible_rows(app);
                app.select_range_to(id, &rows);
            } else if modifiers.command {
                app.toggle_selected(id);
            } else {
                app.select_only(id);
            }
        }
        // Both clicks must be on this row, since egui detects double clicks by timing alone (issue 59);
        // a modifier means selection building, not rename. A component's root is renamable once there
        // are several components (issue 113).
        let renamable = !is_root || app.project.uses_components();
        if response.double_clicked() && same_row_again && renamable && !ui.input(|i| i.modifiers.any()) {
            app.rename = Some((id, name.clone()));
        }
        if response.drag_started() && !is_root {
            app.outliner_drag = Some(Carried::Rows(id));
        }
    }

    if !shadowed {
        context_menu(app, &response, id, is_root);
    }

    // Drop indicator on the row under the pointer (issue 43). Each row's band extends half the
    // spacing each way so the indicator does not blink out over gaps (issue 48).
    let half_gap = ui.spacing().item_spacing.y / 2.0;
    let band = rect.expand2(egui::vec2(0.0, half_gap));
    if dragging && !shadowed && ui.rect_contains_pointer(band) {
        let pointer = ui.input(|i| i.pointer.hover_pos()).unwrap_or(band.center());
        let fraction = ((pointer.y - band.top()) / band.height().max(1.0)).clamp(0.0, 1.0);
        let root = app.scene.root();
        // "Open" means rows are drawn under this one; for a collection only extracted pieces (issue 82).
        let open = !app.collapsed.contains(&id) && !app.scene.row_children(id).is_empty();
        if let Some(target) = drop_position(&app.scene, id, fraction, root, open) {
            if drop_is_legal(&app.scene, carried, &target) {
                app.drop_target = Some(target);
                let stroke = egui::Stroke::new(2.0_f32, token::ACCENT);
                match target.into {
                    // Into a group: the whole row is lit, distinct from a between-siblings line.
                    Some(_) => {
                        painter.rect_filled(rect, 2.0, token::ACCENT.gamma_multiply(0.22));
                        painter.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
                    }
                    // Between siblings: a line in the gap, indented to the landing depth, with a marker.
                    None => {
                        // The side is read from the target, not re-guessed from the pointer: differing thresholds drew
                        // the line on the wrong side.
                        let own = app.scene.node(target.parent).children.iter().position(|&c| c == id);
                        // Landing as this group's first child draws the line under its row (issue 49).
                        let after = target.parent == id || own.is_some_and(|index| target.index > index);
                        let y = gap_line_y(rect, half_gap * 2.0, after);
                        // One level in from `target.parent`.
                        let left = rect.left() + 6.0 + (app.scene.depth(target.parent) + 1) as f32 * 12.0;
                        painter.hline(left..=rect.right(), y, stroke);
                        painter.circle_filled(egui::pos2(left + 1.0, y), 3.0, token::ACCENT);
                    }
                }
            } else {
                // A cycle is shown as refused rather than discovered on release.
                painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0_f32, token::DANGER), egui::StrokeKind::Inside);
            }
        }
    }
}
