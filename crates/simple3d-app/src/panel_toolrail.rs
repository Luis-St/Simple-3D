//! The tool rail: the far-left icon column, freeing the old toolbar row for the viewport and
//! showing the held tool at a glance.

use crate::app::{App, Status};
use crate::gizmo::Mode;
use crate::icon::{self, Glyph};
use crate::theme::{metric, token};
use simple3d_core::config::DisplayMode;
use simple3d_core::keymap::Command;
use simple3d_core::scene::GroupOp;

/// A transform tool's rail entry: glyph and command. Driven from `Mode::ALL`, so every tool needs one.
pub fn tool(mode: Mode) -> (Glyph, Command) {
    match mode {
        Mode::Move => (Glyph::Move, Command::ModeMove),
        Mode::Rotate => (Glyph::Rotate, Command::ModeRotate),
        Mode::Resize => (Glyph::Resize, Command::ModeResize),
        Mode::Scale => (Glyph::Scale, Command::ModeScale),
    }
}

/// Why a boolean button is disabled now, or `None`; shown verbatim in the tooltip.
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

        // Transform tools; exactly one is in force.
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

        // The section plane, in the old handle-frame slot (issue 100): used like a tool, placed and
        // dragged in the viewport. It only cuts the picture (issue 71).
        let sectioned = app.scene.settings.section.enabled;
        let shortcut = app.keymap.shortcut_text(Command::ToggleSection);
        if icon::button(ui, Glyph::Section, size, sectioned, true)
            .on_hover_text(format!("Section view  {shortcut}\nDrag the grip in the plane to slide it"))
            .clicked()
        {
            app.run(Command::ToggleSection);
        }

        // The measure tool is a mode, so it takes the active fill while it holds the pointer.
        let measuring = app.measure.active;
        let shortcut = app.keymap.shortcut_text(Command::MeasureTool);
        if icon::button(ui, Glyph::Measure, size, measuring, true)
            .on_hover_text(format!("Measure between two features  {shortcut}"))
            .clicked()
        {
            app.run(Command::MeasureTool);
        }

        separator(ui);

        // Booleans are actions, not modes: dimmed with a reason when the selection cannot be combined.
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
        // Patterns also work with nothing selected (an empty one), so the button is always live.
        if icon::button(ui, Glyph::Pattern, size, false, true)
            .on_hover_text(format!(
                "{}  {}",
                if has_selection { Command::Pattern.label() } else { crate::app_chrome::EMPTY_PATTERN },
                app.keymap.shortcut_text(Command::Pattern)
            ))
            .clicked()
        {
            app.run(Command::Pattern);
        }
        if icon::button(ui, Glyph::Delete, size, false, has_selection)
            .on_hover_text(format!("Delete the selection  {}", app.keymap.shortcut_text(Command::Delete)))
            .clicked()
        {
            app.run(Command::Delete);
        }

        // View state sits at the foot, away from the tools: it changes the drawing, not the document.
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

/// Group the selection with the operation the button names, in one gesture.
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
        // Both blocked cases must carry an explanation.
        assert!(combine_blocked(0).is_some());
        assert!(combine_blocked(1).is_some());
        assert_ne!(combine_blocked(0), combine_blocked(1), "both cases give the same unhelpful reason");
        assert_eq!(combine_blocked(2), None);
        assert_eq!(combine_blocked(9), None);
    }
}
