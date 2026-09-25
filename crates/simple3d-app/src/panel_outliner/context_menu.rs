//! The menu a right click opens on a row.

use crate::app::App;
use simple3d_core::keymap::Keymap;
use simple3d_core::scene::{Colour, GroupOp, NodeId};

/// The right-click menu on an outliner row.
///
/// Everything here is also a command with a keyboard binding and a place in the
/// menu bar -- that is deliberate. The menu is not a second way of doing things,
/// it is the same commands where the tree is, so the shortcut is learned by
/// reading the row you are already looking at.
pub(crate) fn context_menu(app: &mut App, response: &egui::Response, id: NodeId, is_root: bool) {
    let is_group = app.scene.node(id).is_group();
    use simple3d_core::keymap::Command;

    /// The dividers between the menu's blocks. Only what the row can do is
    /// shown, so a block can come out empty, and a rule is drawn only once
    /// something follows it -- never two rules across a gap, and never one at
    /// the bottom of the menu.
    struct Blocks {
        started: bool,
        ruled: bool,
    }

    impl Blocks {
        /// A block ends here, and the next entry drawn starts a new one.
        fn rule(&mut self) {
            self.ruled = true;
        }

        /// An entry is about to be drawn.
        fn entry(&mut self, ui: &mut egui::Ui) {
            if self.ruled && self.started {
                ui.separator();
            }
            self.ruled = false;
            self.started = true;
        }
    }

    /// One command in the menu, left out where it would not do anything. What
    /// was picked is collected rather than run on the spot: labelling a button
    /// borrows the keymap, and running a command wants the whole application.
    fn item(
        ui: &mut egui::Ui,
        blocks: &mut Blocks,
        keymap: &Keymap,
        chosen: &mut Option<Command>,
        command: Command,
        available: bool,
    ) {
        if !available {
            return;
        }
        blocks.entry(ui);
        if crate::ui::menu_entry(ui, &crate::ui::menu_label(keymap, command), true).clicked() {
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
        let mut add_pattern = false;
        let mut add_custom_pattern = false;
        let mut save_as_primitive = false;
        let mut open_component = false;
        let is_component = app.scene.node(id).is_component();
        let uses_components = app.project.uses_components();
        let mut paint: Option<Option<Colour>> = None;
        let keymap = &app.keymap;
        let mut blocks = Blocks { started: false, ruled: false };

        // First, because adding is what the tree is most often opened to do,
        // and because a shape added from a row belongs on that row: inside the
        // group that was clicked, or beside anything else (issue 44). The Add
        // menu in the menu bar has the same contents and puts them at the
        // document's insertion point instead.
        blocks.entry(ui);
        ui.menu_button("Add", |ui| {
            for op in GroupOp::ALL {
                if ui.button(format!("{} group", op.label())).clicked() {
                    add = Some((None, op));
                    ui.close();
                }
            }
            // Ruled off from the group operators above: both halves add a
            // container, but a group is one that combines what is in it and a
            // pattern is one that repeats it, and six buttons in an unbroken
            // column read as six of the same thing.
            ui.separator();
            // The other container a node can be (issue 67). Empty, for shapes
            // to be put into afterwards -- "Make a pattern of the selection",
            // further down this same menu, is the one that wraps what is
            // already there.
            if ui
                .button("Pattern")
                .on_hover_text("An empty pattern, for shapes to be put into it and repeated")
                .clicked()
            {
                add_pattern = true;
                ui.close();
            }
            // The tool that builds a repetition rule out of stages (issue 67).
            // Beside "Pattern" here as well as in the menu bar: the tree's own
            // Add menu is where a container gets added from, and a rule nobody
            // can find is a rule nobody has.
            if ui
                .button("Custom pattern")
                .on_hover_text("Build a repetition rule out of stages, and keep it for other projects")
                .clicked()
            {
                add_custom_pattern = true;
                ui.close();
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
        .on_hover_text(if app.scene.node(id).is_pattern() {
            "Into this pattern, to be repeated"
        } else if is_group {
            "Into this group"
        } else {
            "Beside this node"
        });
        blocks.rule();
        item(ui, &mut blocks, keymap, &mut chosen, Command::Rename, (!is_root || uses_components) && !multiple);
        item(ui, &mut blocks, keymap, &mut chosen, Command::Duplicate, !is_root);
        blocks.rule();
        item(ui, &mut blocks, keymap, &mut chosen, Command::Copy, !is_root);
        item(ui, &mut blocks, keymap, &mut chosen, Command::Cut, !is_root);
        item(ui, &mut blocks, keymap, &mut chosen, Command::Paste, have_clipboard);
        blocks.rule();
        // What a node can be put into: the two containers, and nothing else
        // (issue 94 -- each block of this menu is one kind of action).
        item(ui, &mut blocks, keymap, &mut chosen, Command::Group, !is_root);
        item(ui, &mut blocks, keymap, &mut chosen, Command::Pattern, !is_root);
        // A group taken out into a component of its own (issue 113), and the
        // way into one that already is.
        item(ui, &mut blocks, keymap, &mut chosen, Command::MakeComponent, is_group && !is_root && !multiple);
        // An empty one as well, on every group and the root among them, so a
        // project that has none yet has a way in from the tree.
        item(ui, &mut blocks, keymap, &mut chosen, Command::NewComponent, is_group && !multiple);
        if is_component && !multiple {
            blocks.entry(ui);
            if crate::ui::menu_entry(ui, "Open the component", true).clicked() {
                open_component = true;
                ui.close();
            }
        }
        blocks.rule();
        // What a node can be turned into. Baking a shape into its triangles
        // (issue 80); dropping detail from one already baked, which is the same
        // row the other way round and so sits next to it (issue 106); cutting
        // one into a pattern of pieces, which on a split re-cuts rather than
        // splits a split, and is why it is offered there too (issue 82); and
        // the way back, on the split itself, which is the only row it can act
        // on. All of them act on the selection, which the click above has
        // already made this row.
        item(ui, &mut blocks, keymap, &mut chosen, Command::ConvertToMesh, !is_root && !app.scene.node(id).is_mesh());
        item(ui, &mut blocks, keymap, &mut chosen, Command::SimplifyMesh, !multiple && app.scene.node(id).is_mesh());
        item(ui, &mut blocks, keymap, &mut chosen, Command::Reassemble, !multiple && app.scene.node(id).is_mesh());
        item(ui, &mut blocks, keymap, &mut chosen, Command::SplitIntoPieces, !is_root && !multiple);
        item(ui, &mut blocks, keymap, &mut chosen, Command::Rejoin, !multiple && app.scene.node(id).is_split());
        blocks.rule();
        // Where a node stands among its siblings. Left out where the move has
        // nowhere to go, rather than offered and silent: a node that is already
        // first among its siblings used to answer a click with a status line
        // that had faded by the time anyone looked for it (issue 41).
        item(ui, &mut blocks, keymap, &mut chosen, Command::MoveUp, app.can_reorder(-1));
        item(ui, &mut blocks, keymap, &mut chosen, Command::MoveDown, app.can_reorder(1));
        // A group's operator, where the group is: the property editor has the
        // same four, but pointing at the group in the tree and saying what it
        // does is one gesture rather than three (issue 37).
        //
        // Like everything else here, it is left out rather than greyed out
        // where it does not apply: an operator on something that is not a group
        // is not a choice, it is not a question.
        blocks.rule();
        if is_group {
            blocks.entry(ui);
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
        blocks.rule();
        if !is_root {
            blocks.entry(ui);
            if crate::ui::menu_entry(ui, &show_hide, true).clicked() {
                chosen = Some(Command::ToggleVisibility);
                ui.close();
            }
        }
        blocks.rule();
        // Painting is here as well as in the property editor, because the
        // outliner is where a group is easiest to point at and painting a group
        // is the reason most people open this menu.
        //
        // A short palette of plain buttons, not a colour picker: the picker is
        // a popup, and opening one from inside a menu closes the menu under it
        // before a colour can be chosen. Anything not on the palette is a
        // click away in the property editor, which the hint says.
        blocks.entry(ui);
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
        if app.scene.subtree_is_painted(id)
            && ui
                .button("Clear the colour")
                .on_hover_text("Back to the theme's colour for an unpainted solid")
                .clicked()
        {
            paint = Some(None);
            ui.close();
        }
        blocks.rule();
        blocks.entry(ui);
        if ui
            .button("Save as primitive\u{2026}")
            .on_hover_text("Keep this, and everything under it, on the palette to use in any project")
            .clicked()
        {
            save_as_primitive = true;
            ui.close();
        }
        blocks.rule();
        item(ui, &mut blocks, &app.keymap, &mut chosen, Command::Delete, !is_root);

        if let Some(op) = operation {
            app.set_group_op(id, op);
        }
        if let Some((type_id, op)) = add {
            app.add_node_at(id, type_id, op);
        }
        if add_pattern {
            app.add_pattern_at(id);
        }
        // An empty pattern on this row, and the tool opened on it: adding leaves
        // the new pattern selected, which is what the tool works on -- and which
        // one it just made, so the tool knows it is a custom kind from the start
        // rather than a linear one waiting to be told otherwise.
        if add_custom_pattern {
            app.add_pattern_at(id);
            if let Some(made) = app.primary() {
                app.open_pattern_tool_on_new(made);
            }
        }
        if let Some(colour) = paint {
            let targets: Vec<NodeId> = app.selection.to_vec();
            app.paint(&targets, colour, None);
        }
        if save_as_primitive {
            app.save_selection_as_primitive();
        }
        if open_component {
            app.open_component_of(id);
        }
        if let Some(command) = chosen {
            app.run(command);
        }
    });
}
