//! The tool rail: the far-left icon column.
//!
//! Everything here was previously a word in the menu bar's toolbar row. Moving
//! it into a rail buys back the whole width of that row for the viewport, and
//! makes the one question the row really answers -- "which tool am I holding?" --
//! answerable without reading anything.

use crate::app::{App, Status};
use crate::gizmo::Mode;
use crate::icon::{self, Glyph};
use crate::theme::{metric, token};
use simple3d_core::config::{DisplayMode, HandleFrame};
use simple3d_core::keymap::Command;
use simple3d_core::scene::GroupOp;

/// The rail's entry for a transform tool: its glyph and the command that
/// selects it. Driven from `Mode::ALL`, so adding a tool to the manipulator
/// without giving it a button here does not compile.
pub fn tool(mode: Mode) -> (Glyph, Command) {
    match mode {
        Mode::Move => (Glyph::Move, Command::ModeMove),
        Mode::Rotate => (Glyph::Rotate, Command::ModeRotate),
        Mode::Resize => (Glyph::Resize, Command::ModeResize),
        Mode::Scale => (Glyph::Scale, Command::ModeScale),
    }
}

/// Why a boolean button cannot be pressed right now, or `None` when it can.
/// Split out so the rule can be stated once and shown in the tooltip verbatim.
pub fn combine_blocked(selection_len: usize) -> Option<&'static str> {
    match selection_len {
        0 => Some("Select two or more shapes to combine them"),
        1 => Some("One shape on its own has nothing to combine with"),
        _ => None,
    }
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let frame =
        egui::Frame::NONE.fill(token::SURFACE_2).inner_margin(egui::Margin { left: 4, right: 4, top: 6, bottom: 6 });
    egui::SidePanel::left("tool-rail").frame(frame).resizable(false).exact_width(metric::RAIL).show(ctx, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 3.0);
        let size = metric::RAIL - 8.0;

        // Transform tools. Exactly one is in force at any moment.
        for mode in Mode::ALL {
            let (glyph, command) = tool(mode);
            let active = app.mode == mode;
            let shortcut = app.keymap.shortcut_text(command);
            if icon::button(ui, glyph, size, active, true)
                .on_hover_text(format!("{}  {shortcut}", mode.label()))
                .clicked()
            {
                app.run(command);
            }
        }

        separator(ui);

        // Which frame the handles work in: a mode, so it is marked the same
        // way the tools are.
        let world = app.settings.handle_frame == HandleFrame::World;
        let shortcut = app.keymap.shortcut_text(Command::ToggleHandleFrame);
        if icon::button(ui, Glyph::Frame, size, world, true)
            .on_hover_text(format!(
                "Handles work in the {} frame  {shortcut}",
                app.settings.handle_frame.label().to_lowercase()
            ))
            .clicked()
        {
            app.run(Command::ToggleHandleFrame);
        }

        separator(ui);

        // Booleans are momentary actions, not modes, so they never take the
        // active fill -- they dim instead when the selection cannot be
        // combined, and say why.
        let blocked = combine_blocked(app.selection.len());
        for (op, glyph) in [
            (GroupOp::Union, Glyph::Union),
            (GroupOp::Difference, Glyph::Difference),
            (GroupOp::Intersection, Glyph::Intersection),
        ] {
            let response = icon::button(ui, glyph, size, false, blocked.is_none());
            let response = match blocked {
                Some(why) => response.on_hover_text(why),
                None => response.on_hover_text(format!("{} of the selection", op.label())),
            };
            if response.clicked() {
                combine(app, op);
            }
        }

        separator(ui);

        let has_selection = !app.selection.is_empty();
        if icon::button(ui, Glyph::Group, size, false, has_selection)
            .on_hover_text(format!("Group the selection  {}", app.keymap.shortcut_text(Command::Group)))
            .clicked()
        {
            app.run(Command::Group);
        }
        if icon::button(ui, Glyph::Delete, size, false, has_selection)
            .on_hover_text(format!("Delete the selection  {}", app.keymap.shortcut_text(Command::Delete)))
            .clicked()
        {
            app.run(Command::Delete);
        }

        // View state sits at the foot of the rail, away from the tools: it
        // changes what is drawn, never what is in the document.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 3.0);
            let grid = app.scene.settings.grid_visible;
            if icon::button(ui, Glyph::Grid, size, grid, true)
                .on_hover_text(format!("Ground grid  {}", app.keymap.shortcut_text(Command::ToggleGrid)))
                .clicked()
            {
                app.run(Command::ToggleGrid);
            }
            separator(ui);
            for (mode, glyph, command) in [
                (DisplayMode::Wireframe, Glyph::Wireframe, Command::DisplayWireframe),
                (DisplayMode::ShadedWithEdges, Glyph::ShadedEdges, Command::DisplayShadedEdges),
                (DisplayMode::Shaded, Glyph::Shaded, Command::DisplayShaded),
            ] {
                let active = app.settings.display_mode == mode;
                if icon::button(ui, glyph, size, active, true)
                    .on_hover_text(format!("{}  {}", mode.label(), app.keymap.shortcut_text(command)))
                    .clicked()
                {
                    app.run(command);
                }
            }
        });
    });
}

/// Group what is selected and give the group the operation the button names --
/// one gesture for what was previously "group, then find the radio row".
fn combine(app: &mut App, op: GroupOp) {
    if let Some(why) = combine_blocked(app.selection.len()) {
        app.status = Status::Info(why.into());
        return;
    }
    app.edit(op.label(), None);
    let selection = app.selection.clone();
    match app.scene.group_selection(&selection) {
        Some(group) => {
            if let Some(node) = app.scene.get_mut(group) {
                node.body = simple3d_core::scene::Body::Group { op };
            }
            app.select_only(group);
            app.status = Status::Info(format!("{} of {} shapes", op.label(), selection.len()));
        }
        None => app.status = Status::Warning("That selection cannot be combined".into()),
    }
}

fn separator(ui: &mut egui::Ui) {
    ui.add_space(3.0);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), egui::Sense::hover());
    ui.painter().hline(rect.x_range().shrink(3.0_f32), rect.center().y, egui::Stroke::new(1.0_f32, token::SURFACE_3));
    ui.add_space(3.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_its_own_button_and_its_own_glyph() {
        let glyphs: Vec<Glyph> = Mode::ALL.iter().map(|m| tool(*m).0).collect();
        for (i, a) in glyphs.iter().enumerate() {
            for b in &glyphs[i + 1..] {
                assert_ne!(a, b, "two tools share a glyph, so the rail cannot say which is active");
            }
        }
    }

    #[test]
    fn a_boolean_button_says_why_it_is_dimmed() {
        // "Disabled with no explanation" is the failure this rule exists to
        // prevent, so both blocked cases must carry a sentence.
        assert!(combine_blocked(0).is_some());
        assert!(combine_blocked(1).is_some());
        assert_ne!(combine_blocked(0), combine_blocked(1), "both cases give the same unhelpful reason");
        assert_eq!(combine_blocked(2), None);
        assert_eq!(combine_blocked(9), None);
    }
}
