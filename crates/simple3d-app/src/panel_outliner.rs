//! The left dock: the outliner (spec section 7.2) over the primitive palette.
//!
//! The outliner is the centre of gravity of the application, so it gets the
//! dock's height and the palette gets a fixed strip under it. One row per node,
//! 22 px, and the marks that matter -- visibility, the boolean operator, a
//! failure -- are drawn on every row rather than revealed on hover, because a
//! mark you have to go looking for cannot be scanned.

use crate::app::{App, DropTarget, Status};
use crate::icon::{self, Glyph};
use crate::theme::{self, metric, token};
use simple3d_core::keymap::Keymap;
use simple3d_core::scene::{Colour, GroupOp, NodeId, Scene};

/// Where a pointer at `fraction` down a row would drop: into the row's node, or
/// between it and one of its siblings. Kept separate from the drawing so the rule
/// can be reasoned about (and tested) on its own.
pub fn drop_position(scene: &Scene, over: NodeId, fraction: f32, root: NodeId) -> Option<DropTarget> {
    let node = scene.get(over)?;
    let is_group = node.is_group();
    // The top and bottom fifths of a row mean "beside"; the middle means "into",
    // but only for a group, since nothing else can hold children.
    let before = fraction < 0.25;
    let after = fraction > 0.75;
    if is_group && !before && !after {
        return Some(DropTarget { parent: over, index: scene.node(over).children.len(), into: Some(over) });
    }
    if over == root {
        // The root has no siblings, so any drop on it goes inside.
        return Some(DropTarget { parent: root, index: scene.node(root).children.len(), into: Some(root) });
    }
    let parent = node.parent?;
    let index = scene.node(parent).children.iter().position(|&c| c == over)?;
    Some(DropTarget { parent, index: if after { index + 1 } else { index }, into: None })
}

/// The operator mark a node carries in the tree: its own, for a group, or the
/// one it is subject to, for a child of a difference or intersection. Reading
/// the boolean tree at a glance is the whole reason these are inline rather
/// than in a modifier stack somewhere else.
///
/// The flag says the mark is a *cut*: an operand being removed, drawn in the
/// danger colour, which is the one distinction worth a second colour here.
pub fn operator_badge(scene: &Scene, id: NodeId) -> Option<(Glyph, bool)> {
    // The root is a union of everything by definition; badging it says nothing
    // and puts a mark on the one row that never changes.
    if scene.node(id).parent.is_none() {
        return None;
    }
    if let Some(op) = scene.node(id).group_op() {
        return Some((symbol(op), false));
    }
    let parent = scene.node(id).parent?;
    let op = scene.node(parent).group_op()?;
    match op {
        // The base of a difference is what is being cut, not a cut.
        GroupOp::Difference if scene.difference_base(parent) != Some(id) => Some((symbol(op), true)),
        GroupOp::Intersection => Some((symbol(op), false)),
        _ => None,
    }
}

/// The set-theory symbols would be the obvious mark, but the bundled UI face
/// has no glyph for any of them and would draw three identical tofu boxes.
/// These are the same three shapes the tool rail's boolean buttons carry, which
/// makes the tree and the rail read as one vocabulary.
fn symbol(op: GroupOp) -> Glyph {
    match op {
        GroupOp::Union => Glyph::Union,
        GroupOp::Difference => Glyph::Difference,
        GroupOp::Intersection => Glyph::Intersection,
        GroupOp::Hull => Glyph::Polyhedron,
    }
}

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

/// How much is selected, if anything.
fn selection_line(app: &mut App, ui: &mut egui::Ui) {
    if app.selection.is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        ui.add_space(theme::metric::PANEL_PAD);
        ui.add(egui::Label::new(theme::hint(format!("{} selected", app.selection.len()))).selectable(false));
    });
    ui.add_space(2.0);
}

fn tree(app: &mut App, ui: &mut egui::Ui) {
    confirm_strip(app, ui);

    let dragging = app.outliner_drag;
    // The whole load, worked out once a frame: every row it holds leaves the
    // tree, the legality of a drop is judged against all of it, and the ghost
    // says how much is on the pointer (issue 43).
    let carried: Vec<NodeId> = dragging.map(|source| app.dragged_nodes(source)).unwrap_or_default();
    app.drop_target = None;
    let ctx = ui.ctx().clone();
    let (area, restore) = theme::list_scroll_area(ui);
    // Named, because an automatic id is a count of the widgets drawn before it
    // -- and the "N selected" line above appears and disappears with the
    // selection, which would renumber every row inside on the frame a selection
    // changed.
    area.id_salt("outliner-tree").show(ui, |ui| {
        ui.set_style(restore);
        ui.add_space(2.0);
        let ids = visible_rows(app);
        for id in ids {
            // What is being dragged is drawn on the pointer, not in the tree:
            // a row that stays where it was, while a copy of it follows the
            // mouse, reads as two of the same node (issue 39).
            if carried.iter().any(|&source| id == source || app.scene.is_ancestor_of(source, id)) {
                continue;
            }
            row(app, ui, id, &carried);
        }
        // Dropping in the empty space below the tree means "at the end of the
        // root", which is otherwise awkward to reach.
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), ui.available_height().max(24.0)),
            egui::Sense::hover(),
        );
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

    if let Some(source) = dragging {
        drag_ghost(app, &ctx, source, carried.len());
    }

    // Finish the drag on release, wherever the pointer ended up.
    if dragging.is_some() && ctx.input(|i| i.pointer.any_released()) {
        finish_drag(app);
    }
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

fn push_visible(app: &App, id: NodeId, out: &mut Vec<NodeId>) {
    out.push(id);
    if app.collapsed.contains(&id) {
        return;
    }
    for &child in &app.scene.node(id).children {
        push_visible(app, child, out);
    }
}

/// What is being dragged, drawn under the pointer: the node's own glyph and
/// name on a translucent slab.
///
/// The tree has already left the row out, so this is the only place the dragged
/// node appears -- which is what makes it obvious that it is *held* rather than
/// merely highlighted, and what makes the drop line the answer to "where would
/// it land".
fn drag_ghost(app: &App, ctx: &egui::Context, source: NodeId, carried: usize) {
    let Some(pointer) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    if !app.scene.contains(source) {
        return;
    }
    let node = app.scene.node(source);
    // A whole selection travels under one slab, named for how much of it there
    // is: eight slabs stacked on the pointer would cover the drop indicator
    // they exist to point at.
    let name = if carried > 1 { format!("{carried} nodes") } else { node.name.clone() };
    let glyph = if node.is_group() {
        Glyph::Bracket
    } else {
        Glyph::for_primitive(node.spec().map(|s| s.type_id).unwrap_or(""))
    };
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("outliner-drag-ghost")));
    let galley = painter.layout_no_wrap(
        name,
        egui::FontId::proportional(theme::font::VALUE),
        token::TEXT_HI.gamma_multiply(0.9),
    );
    // Held a little below and to the right of the pointer, so the drop
    // indicator under the pointer is never covered by what is being dropped.
    let at = pointer + egui::vec2(14.0, 6.0);
    let slab = egui::Rect::from_min_size(at, egui::vec2(galley.size().x + 26.0, metric::ROW)).expand(1.0);
    painter.rect_filled(slab, 3.0, token::SURFACE_2.gamma_multiply(0.72));
    painter.rect_stroke(
        slab,
        3.0,
        egui::Stroke::new(1.0_f32, token::ACCENT.gamma_multiply(0.55)),
        egui::StrokeKind::Inside,
    );
    icon::draw(
        &painter,
        egui::Rect::from_min_size(at + egui::vec2(4.0, 4.0), egui::Vec2::splat(14.0)),
        glyph,
        token::ACCENT.gamma_multiply(0.75),
    );
    painter.galley(
        egui::pos2(at.x + 22.0, slab.center().y - galley.size().y / 2.0),
        galley,
        token::TEXT_HI.gamma_multiply(0.9),
    );
}

/// The inline confirmation for deleting a group.
///
/// It is a strip in the outliner rather than a dialog over the window because
/// the question is about the tree, and the answer is easier to give while
/// still looking at it. The two answers are named for what they do -- neither
/// of them is "OK".
fn confirm_strip(app: &mut App, ui: &mut egui::Ui) {
    if app.pending_delete.is_none() {
        return;
    }
    let names: Vec<String> = app
        .pending_delete
        .as_ref()
        .map(|ids| ids.iter().map(|id| app.scene.node(*id).name.clone()).collect())
        .unwrap_or_default();
    let total = app.pending_delete_count();
    let groups = names.len();
    let children = total - groups;

    egui::Frame::NONE
        .fill(token::DANGER.gamma_multiply(0.16))
        .stroke(egui::Stroke::new(1.0_f32, token::DANGER.gamma_multiply(0.7)))
        .inner_margin(egui::Margin { left: 8, right: 8, top: 6, bottom: 6 })
        .show(ui, |ui| {
            let what = if groups == 1 { format!("\u{201C}{}\u{201D}", names[0]) } else { format!("{groups} groups") };
            ui.add(
                egui::Label::new(theme::value(format!(
                    "Delete {what}? It holds {children} node{}.",
                    if children == 1 { "" } else { "s" }
                )))
                .selectable(false)
                .wrap(),
            );
            ui.horizontal_wrapped(|ui| {
                if ui.button("Delete the children too").on_hover_text("The group and everything inside it").clicked() {
                    app.confirm_delete(false);
                }
                if ui
                    .button("Keep the children")
                    .on_hover_text("The children move up into the group's own place; only the group goes")
                    .clicked()
                {
                    app.confirm_delete(true);
                }
                if ui.button("Cancel").clicked() {
                    app.cancel_delete();
                }
            });
        });
    // Escape is the way out of everything else in this application, so it is
    // the way out of this too.
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.cancel_delete();
    }
}

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

fn row(app: &mut App, ui: &mut egui::Ui, id: NodeId, carried: &[NodeId]) {
    let depth = app.scene.depth(id);
    let node = app.scene.node(id);
    let name = node.name.clone();
    let visible = node.visible;
    let is_group = node.is_group();
    let is_root = id == app.scene.root();
    let selected = app.is_selected(id);
    let primary = app.primary() == Some(id);
    let failed = app.evaluated.error_for(id).is_some();
    let badge = operator_badge(&app.scene, id);
    let type_id = node.spec().map(|s| s.type_id).unwrap_or("");

    let full = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(full, metric::ROW), egui::Sense::hover());
    let response = ui.interact(rect, row_id(id), egui::Sense::click_and_drag());

    // Background: selection is an accent tint with a solid bar at the left edge,
    // so the primary node of a multi-selection is distinguishable from the rest
    // without a second colour.
    let painter = ui.painter().clone();
    if selected {
        painter.rect_filled(rect, 0.0, token::ACCENT.gamma_multiply(if primary { 0.22 } else { 0.13 }));
        painter.rect_filled(
            egui::Rect::from_min_size(rect.left_top(), egui::vec2(2.0, rect.height())),
            0.0,
            if primary { token::ACCENT } else { token::ACCENT.gamma_multiply(0.5) },
        );
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, token::SURFACE_3);
    }

    // Visibility lives at the right edge, always drawn, dimmed rather than
    // hidden when the node is not visible.
    let eye_rect =
        egui::Rect::from_min_size(egui::pos2(rect.right() - 22.0, rect.top() + 3.0), egui::Vec2::splat(16.0));
    let eye = ui.interact(eye_rect, ui.id().with((id, "eye")), egui::Sense::click());
    if !is_root {
        let colour = if eye.hovered() {
            token::TEXT_HI
        } else if visible {
            token::TEXT_LO
        } else {
            token::TEXT_LO.gamma_multiply(0.45)
        };
        icon::draw(&painter, eye_rect.shrink(1.0), if visible { Glyph::Eye } else { Glyph::EyeOff }, colour);
        if eye.clicked() {
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
    let has_children = !app.scene.node(id).children.is_empty();
    let mut collapsed = app.collapsed.contains(&id);
    let mut twisty_hovered = false;
    if has_children {
        let twisty = ui.interact(twisty_rect, ui.id().with((id, "twisty")), egui::Sense::click());
        twisty_hovered = twisty.hovered();
        theme::twisty(
            &painter,
            twisty_rect.center(),
            !collapsed,
            if twisty.hovered() { token::TEXT_HI } else { token::TEXT_LO },
        );
        if twisty.clicked() {
            collapsed = !collapsed;
            app.set_collapsed(id, collapsed);
        }
    }
    x += 14.0;

    // Type glyph: a bracket for a group, a solid mark for a shape.
    let glyph_rect = egui::Rect::from_min_size(egui::pos2(x, rect.top() + 4.0), egui::Vec2::splat(14.0));
    let glyph = if is_group { Glyph::Bracket } else { Glyph::for_primitive(type_id) };
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
    let galley = painter.layout(
        name.clone(),
        egui::FontId::proportional(theme::font::VALUE),
        text_colour,
        (name_right - x).max(1.0),
    );
    painter.galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, text_colour);

    let response = response.on_hover_text(hover_text(
        app,
        id,
        is_group,
        app.scene.node(id).group_op(),
        app.scene.difference_base(id),
    ));

    // The eye and the twisty own their own clicks: pressing either must not
    // also select the row.
    if !eye.hovered() && !twisty_hovered {
        if response.clicked() {
            if ui.input(|i| i.modifiers.command || i.modifiers.shift) {
                app.toggle_selected(id);
            } else {
                app.select_only(id);
            }
        }
        if response.double_clicked() && !is_root {
            app.rename = Some((id, name.clone()));
        }
        if response.drag_started() && !is_root {
            app.outliner_drag = Some(id);
        }
    }

    context_menu(app, &response, id, is_root);

    // Drop indicator: which group would take the load, or which gap it would
    // land in. Both are drawn on the row under the pointer, because that is
    // where the pointer is looking (issue 43).
    if !carried.is_empty() && response.contains_pointer() {
        let pointer = ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center());
        let fraction = ((pointer.y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
        let root = app.scene.root();
        if let Some(target) = drop_position(&app.scene, id, fraction, root) {
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
                        let after = own.is_some_and(|index| target.index > index);
                        let y = if after { rect.bottom() } else { rect.top() };
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

/// Whether everything being dragged may land on `target`: nothing can be
/// dropped into itself or into anything it holds, and only a group can hold
/// children at all.
pub fn drop_is_legal(scene: &Scene, carried: &[NodeId], target: &DropTarget) -> bool {
    scene.node(target.parent).is_group()
        && carried.iter().all(|&source| target.parent != source && !scene.is_ancestor_of(source, target.parent))
}

/// The right-click menu on an outliner row.
///
/// Everything here is also a command with a keyboard binding and a place in the
/// menu bar -- that is deliberate. The menu is not a second way of doing things,
/// it is the same commands where the tree is, so the shortcut is learned by
/// reading the row you are already looking at.
fn context_menu(app: &mut App, response: &egui::Response, id: NodeId, is_root: bool) {
    let is_group = app.scene.node(id).is_group();
    use simple3d_core::keymap::Command;

    /// One command in the menu. What was picked is collected rather than run on
    /// the spot: labelling a button borrows the keymap, and running a command
    /// wants the whole application.
    fn item(ui: &mut egui::Ui, keymap: &Keymap, chosen: &mut Option<Command>, command: Command, enabled: bool) {
        if crate::ui::menu_entry(ui, &crate::ui::menu_label(keymap, command), enabled).clicked() {
            *chosen = Some(command);
            ui.close();
        }
    }

    response.context_menu(|ui| {
        // Right-clicking a row that is not in the selection acts on that row,
        // not on whatever happened to be selected before -- guessing the other
        // way round is how a delete takes the wrong node.
        if !app.is_selected(id) {
            app.select_only(id);
        }
        let hidden = !app.scene.node(id).visible;
        let multiple = app.selection.len() > 1;
        let have_clipboard = app.clipboard.is_some();

        // Named for what it does rather than for the flag it flips, because
        // which of the two "Toggle visibility" means depends on the row.
        let show_hide = format!(
            "{}\t{}",
            if hidden { "Show" } else { "Hide" },
            app.keymap.shortcut_text(Command::ToggleVisibility)
        );

        let mut chosen: Option<Command> = None;
        let mut operation: Option<GroupOp> = None;
        let mut add: Option<(Option<&'static str>, GroupOp)> = None;
        let mut save_as_primitive = false;
        let mut paint: Option<Option<Colour>> = None;
        let keymap = &app.keymap;

        // First, because adding is what the tree is most often opened to do,
        // and because a shape added from a row belongs on that row: inside the
        // group that was clicked, or beside anything else (issue 44). The Add
        // menu in the menu bar has the same contents and puts them at the
        // document's insertion point instead.
        ui.menu_button("Add", |ui| {
            for op in GroupOp::ALL {
                if ui.button(format!("{} group", op.label())).clicked() {
                    add = Some((None, op));
                    ui.close();
                }
            }
            ui.separator();
            for category in simple3d_core::primitive::categories() {
                ui.menu_button(category, |ui| {
                    for spec in simple3d_core::primitive::REGISTRY.iter().filter(|s| s.category == category) {
                        if ui.button(spec.label).clicked() {
                            add = Some((Some(spec.type_id), GroupOp::Union));
                            ui.close();
                        }
                    }
                });
            }
        })
        .response
        .on_hover_text(if is_group { "Into this group" } else { "Beside this node" });
        ui.separator();
        item(ui, keymap, &mut chosen, Command::Rename, !is_root && !multiple);
        item(ui, keymap, &mut chosen, Command::Duplicate, !is_root);
        ui.separator();
        item(ui, keymap, &mut chosen, Command::Copy, !is_root);
        item(ui, keymap, &mut chosen, Command::Cut, !is_root);
        item(ui, keymap, &mut chosen, Command::Paste, have_clipboard);
        ui.separator();
        item(ui, keymap, &mut chosen, Command::Group, !is_root);
        // Disabled where the move has nowhere to go, rather than enabled and
        // silent: a node that is already first among its siblings used to
        // answer a click with a status line that had faded by the time anyone
        // looked for it (issue 41).
        item(ui, keymap, &mut chosen, Command::MoveUp, app.can_reorder(-1));
        item(ui, keymap, &mut chosen, Command::MoveDown, app.can_reorder(1));
        ui.separator();
        // A group's operator, where the group is: the property editor has the
        // same four, but pointing at the group in the tree and saying what it
        // does is one gesture rather than three (issue 37).
        if is_group {
            let current = app.scene.node(id).group_op();
            ui.menu_button("Operation", |ui| {
                for option in GroupOp::ALL {
                    if crate::ui::menu_entry(ui, option.label(), current != Some(option)).clicked() {
                        operation = Some(option);
                        ui.close();
                    }
                }
            });
        }
        ui.separator();
        if crate::ui::menu_entry(ui, &show_hide, !is_root).clicked() {
            chosen = Some(Command::ToggleVisibility);
            ui.close();
        }
        ui.separator();
        // Painting is here as well as in the property editor, because the
        // outliner is where a group is easiest to point at and painting a group
        // is the reason most people open this menu.
        //
        // A short palette of plain buttons, not a colour picker: the picker is
        // a popup, and opening one from inside a menu closes the menu under it
        // before a colour can be chosen. Anything not on the palette is a
        // click away in the property editor, which the hint says.
        ui.label(crate::theme::hint("Colour"));
        ui.horizontal(|ui| {
            for (name, preset) in crate::theme::PAINT_PRESETS {
                let swatch = egui::Button::new("")
                    .fill(preset)
                    .stroke(egui::Stroke::new(1.0_f32, crate::theme::token::SURFACE_3))
                    .min_size(egui::vec2(16.0, 16.0));
                if ui.add(swatch).on_hover_text(name).clicked() {
                    paint = Some(Some(Colour([preset.r(), preset.g(), preset.b()])));
                    ui.close();
                }
            }
        });
        // The colours the user has mixed for themselves, offered where the
        // fixed palette is: the property editor keeps the same two rows. A
        // preset never appears here -- it is already one row up.
        let recent: Vec<[u8; 3]> = app.custom_recent_colours();
        if !recent.is_empty() {
            ui.horizontal(|ui| {
                for colour in recent {
                    let swatch = egui::Button::new("")
                        .fill(egui::Color32::from_rgb(colour[0], colour[1], colour[2]))
                        .stroke(egui::Stroke::new(1.0_f32, crate::theme::token::SURFACE_3))
                        .min_size(egui::vec2(16.0, 16.0));
                    let name = format!("#{:02x}{:02x}{:02x}", colour[0], colour[1], colour[2]);
                    if ui.add(swatch).on_hover_text(name).clicked() {
                        paint = Some(Some(Colour(colour)));
                        ui.close();
                    }
                }
            });
        }
        if ui
            .add_enabled(app.scene.subtree_is_painted(id), egui::Button::new("Clear the colour"))
            .on_hover_text("Back to the theme's colour for an unpainted solid")
            .clicked()
        {
            paint = Some(None);
            ui.close();
        }
        ui.separator();
        if ui
            .button("Save as primitive\u{2026}")
            .on_hover_text("Keep this, and everything under it, on the palette to use in any project")
            .clicked()
        {
            save_as_primitive = true;
            ui.close();
        }
        ui.separator();
        item(ui, &app.keymap, &mut chosen, Command::Delete, !is_root);

        if let Some(op) = operation {
            app.set_group_op(id, op);
        }
        if let Some((type_id, op)) = add {
            app.add_node_at(id, type_id, op);
        }
        if let Some(colour) = paint {
            let targets: Vec<NodeId> = app.selection.to_vec();
            app.paint(&targets, colour, None);
        }
        if save_as_primitive {
            app.save_selection_as_primitive();
        }
        if let Some(command) = chosen {
            app.run(command);
        }
    });
}

fn hover_text(
    app: &App,
    id: NodeId,
    is_group: bool,
    op: Option<simple3d_core::scene::GroupOp>,
    base_child: Option<NodeId>,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(error) = app.evaluated.error_for(id) {
        lines.push(error.message.clone());
    }
    if is_group {
        if let Some(op) = op {
            lines.push(format!("{} of {} children", op.label(), app.scene.node(id).children.len()));
            if op.order_matters() {
                match base_child {
                    Some(base) => {
                        lines.push(format!("Base: {} (everything below it is subtracted)", app.scene.node(base).name))
                    }
                    None => lines.push("No visible child to use as the base".into()),
                }
            }
        }
    } else if let Some(spec) = app.scene.node(id).spec() {
        lines.push(spec.label.to_string());
    }
    if let Some(mesh) = app.evaluated.node_meshes.get(&id) {
        if let Some((lo, hi)) = mesh.bounds() {
            lines.push(crate::ui::describe_size(hi - lo, app.unit()));
        }
    }
    lines.join("\n")
}

pub(crate) fn finish_drag(app: &mut App) {
    let Some(source) = app.outliner_drag.take() else { return };
    let Some(target) = app.drop_target.take() else { return };
    let carried: Vec<NodeId> = app.dragged_nodes(source).into_iter().filter(|id| app.scene.contains(*id)).collect();
    if carried.is_empty() {
        return;
    }
    app.edit("Reparent", None);
    match app.scene.reparent_many(&carried, target.parent, target.index) {
        Ok(()) => {
            // The load stays selected, in the order it landed in: a drag that
            // dropped the rest of the selection on arrival would make moving
            // several nodes twice in a row impossible.
            app.select_only(carried[0]);
            for &id in &carried[1..] {
                app.toggle_selected(id);
            }
            let into = app.scene.node(target.parent).name.clone();
            app.status = Status::Info(if carried.len() > 1 {
                format!("Moved {} nodes into {into}", carried.len())
            } else {
                format!("Moved into {into}")
            });
        }
        Err(why) => {
            app.history.discard_last();
            app.status = Status::Warning(why.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simple3d_core::scene::GroupOp;

    fn tree() -> (Scene, NodeId, NodeId, NodeId, NodeId) {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        let inner = scene.add_primitive("box", group, 0).unwrap();
        let sibling = scene.add_primitive("box", root, 1).unwrap();
        (scene, root, group, inner, sibling)
    }

    #[test]
    fn the_middle_of_a_group_row_drops_into_it() {
        let (scene, root, group, _, _) = tree();
        let target = drop_position(&scene, group, 0.5, root).unwrap();
        assert_eq!(target.into, Some(group));
        assert_eq!(target.parent, group);
    }

    #[test]
    fn the_edges_of_a_row_drop_beside_it() {
        let (scene, root, group, _, sibling) = tree();
        let before = drop_position(&scene, sibling, 0.05, root).unwrap();
        assert_eq!(before.parent, root);
        assert_eq!(before.index, 1);
        assert_eq!(before.into, None);

        let after = drop_position(&scene, sibling, 0.95, root).unwrap();
        assert_eq!(after.index, 2);

        // A group's edges also mean "beside", not "into".
        let beside_group = drop_position(&scene, group, 0.05, root).unwrap();
        assert_eq!(beside_group.parent, root);
        assert_eq!(beside_group.index, 0);
        assert_eq!(beside_group.into, None);
    }

    #[test]
    fn a_leaf_row_never_drops_into_itself() {
        let (scene, root, _, inner, _) = tree();
        for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let target = drop_position(&scene, inner, fraction, root).unwrap();
            assert_eq!(target.into, None, "a leaf accepted a child at {fraction}");
        }
    }

    #[test]
    fn any_drop_on_the_root_goes_inside_it() {
        let (scene, root, _, _, _) = tree();
        for fraction in [0.0, 0.5, 1.0] {
            let target = drop_position(&scene, root, fraction, root).unwrap();
            assert_eq!(target.parent, root);
            assert_eq!(target.into, Some(root));
        }
    }

    #[test]
    fn the_index_a_drop_reports_is_the_one_reparent_expects() {
        // The two have to agree, or a drop lands one row away from the indicator.
        let (mut scene, root, group, inner, sibling) = tree();
        let target = drop_position(&scene, sibling, 0.95, root).unwrap();
        scene.reparent(inner, target.parent, target.index).unwrap();
        assert_eq!(scene.node(root).children, vec![group, sibling, inner]);
    }

    #[test]
    fn a_drop_is_refused_when_any_one_of_the_dragged_nodes_would_swallow_itself() {
        // Issue 43: the whole load is judged, not just the row that was grabbed.
        let (scene, root, group, inner, sibling) = tree();
        let into_group = DropTarget { parent: group, index: 0, into: Some(group) };
        assert!(drop_is_legal(&scene, &[sibling], &into_group));
        // `group` is being dragged too, so nothing may land inside it.
        assert!(!drop_is_legal(&scene, &[sibling, group], &into_group));
        // Nor may a node land inside itself, however it was reached.
        assert!(!drop_is_legal(&scene, &[group], &into_group));
        // Its own child is a fine place for a sibling, and not for the group.
        let beside_inner = DropTarget { parent: group, index: 1, into: None };
        assert!(drop_is_legal(&scene, &[sibling], &beside_inner));
        assert!(!drop_is_legal(&scene, &[inner, group], &beside_inner));
        // Only a group can hold children.
        let into_leaf = DropTarget { parent: sibling, index: 0, into: Some(sibling) };
        assert!(!drop_is_legal(&scene, &[inner], &into_leaf));
        let _ = root;
    }

    #[test]
    fn a_group_wears_its_own_operator_and_a_cut_child_wears_the_cut() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        let base = scene.add_primitive("box", group, 0).unwrap();
        let cut = scene.add_primitive("cylinder", group, 1).unwrap();

        assert_eq!(operator_badge(&scene, group), Some((Glyph::Difference, false)));
        // The base is what is being cut, so it carries no mark ...
        assert_eq!(operator_badge(&scene, base), None);
        // ... and the operand that does the cutting is marked, in the danger
        // colour, which is what `true` here selects.
        assert_eq!(operator_badge(&scene, cut), Some((Glyph::Difference, true)));
    }

    #[test]
    fn the_root_row_carries_no_operator_mark() {
        let (scene, root, _, _, _) = tree();
        assert_eq!(operator_badge(&scene, root), None);
    }

    #[test]
    fn a_child_of_a_union_carries_no_mark_of_its_own() {
        // Union is the default and the common case; badging every child of one
        // would be noise down the whole tree.
        let (scene, _, _, inner, _) = tree();
        assert_eq!(operator_badge(&scene, inner), None);
    }
}
