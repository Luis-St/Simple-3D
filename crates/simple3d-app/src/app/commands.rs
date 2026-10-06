//! Running a command, whatever asked for it.

use super::*;
use crate::gizmo::Mode;
use crate::view::ViewPreset;
use simple3d_core::config::DisplayMode;
use simple3d_core::keymap::Command;

impl App {
    // -- commands -----------------------------------------------------------

    /// Whether `command`'s chord is held now. Modifiers count as much as keys, so a hold on Ctrl+V does
    /// not fire on V, and a modifier-only chord (the holds' default, issue 77) is just the modifier
    /// state. `key_down` reports toolkit keys.
    pub fn holding(&self, command: Command, key_down: impl Fn(egui::Key) -> bool, mods: egui::Modifiers) -> bool {
        self.keymap.binding(command).is_some_and(|chord| {
            chord.satisfied_by(
                |name| crate::ui::key_from_name(name).is_some_and(&key_down),
                mods.command,
                mods.shift,
                mods.alt,
            )
        })
    }

    pub fn run(&mut self, command: Command) {
        use Command::*;
        match command {
            New => self.new_project(),
            Open => self.open_dialog(),
            CloseTab => self.close_tab(self.active),
            NextTab => self.cycle_tab(1),
            PreviousTab => self.cycle_tab(-1),
            Save => self.save(),
            SaveAs => self.save_as(),
            Import => self.start_import(),
            Export => self.modal = Modal::Export,
            Quit => self.request_quit(),

            Undo => self.undo(),
            Redo => self.redo(),
            Copy => self.copy_selection(false),
            Cut => self.copy_selection(true),
            Paste => self.paste(),
            Duplicate => self.duplicate(),
            Delete => self.delete_selection(),
            Group => self.group_selection(),
            Pattern => self.make_pattern(),
            MakeComponent => self.make_component(),
            NewComponent => self.new_component(),
            ConvertToMesh => self.convert_selection_to_mesh(),
            SimplifyMesh => self.open_simplify_tool(),
            RoundEdges => self.open_round_tool(),
            Reassemble => self.open_reassemble_tool(),
            SplitIntoPieces => self.open_split_tool(),
            Rejoin => self.rejoin_selection(),
            Rename => {
                if let Some(id) = self.primary() {
                    self.rename = Some((id, self.scene.node(id).name.clone()));
                }
            }
            ToggleVisibility => self.toggle_visibility(),
            MoveUp => self.reorder(-1),
            MoveDown => self.reorder(1),

            FrameSelection => self.frame_selection(),
            FrameAll => self.frame_all(),
            ViewTop => self.set_view(ViewPreset::Top),
            ViewBottom => self.set_view(ViewPreset::Bottom),
            ViewFront => self.set_view(ViewPreset::Front),
            ViewBack => self.set_view(ViewPreset::Back),
            ViewLeft => self.set_view(ViewPreset::Left),
            ViewRight => self.set_view(ViewPreset::Right),
            ViewIsometric => self.set_view(ViewPreset::Isometric),
            ToggleGrid => self.scene.settings.grid_visible = !self.scene.settings.grid_visible,
            ToggleSection => self.toggle_section(),
            ToggleAxisX => self.toggle_axis(0),
            ToggleAxisY => self.toggle_axis(1),
            ToggleAxisZ => self.toggle_axis(2),
            DisplayShaded => self.settings.display_mode = DisplayMode::Shaded,
            DisplayShadedEdges => self.settings.display_mode = DisplayMode::ShadedWithEdges,
            DisplayWireframe => self.settings.display_mode = DisplayMode::Wireframe,
            ToggleBoundingBox => self.settings.show_bounding_box = !self.settings.show_bounding_box,
            ToggleDocks => {
                self.settings.layout.docks_hidden = !self.settings.layout.docks_hidden;
                self.status = Status::Info(
                    if self.settings.layout.docks_hidden {
                        "Docks hidden; press it again to bring them back exactly as they were"
                    } else {
                        "Docks restored"
                    }
                    .into(),
                );
            }
            ResetLayout => {
                crate::dock::reset(self);
                self.status = Status::Info("Panel layout reset".into());
            }

            // A transform tool puts the measure tool away: only one can own a click.
            ModeMove => self.pick_transform(Mode::Move),
            ModeRotate => self.pick_transform(Mode::Rotate),
            ModeResize => self.pick_transform(Mode::Resize),
            ModeScale => self.pick_transform(Mode::Scale),
            ModePushPull => self.pick_transform(Mode::PushPull),
            MeasureTool => self.toggle_measure(),
            AlignDistribute => self.toggle_arrange_tool(),
            // A hold key read live during a drag, so pressing it alone does nothing (issue 68).
            SnapToGeometry => {}
            // The other hold key, read live by the wheel (issue 97).
            ZoomToPointer => {}
            NudgeLeft | NudgeRight | NudgeUp | NudgeDown | NudgeAway | NudgeToward => self.nudge(command),
        }
    }

    /// Switch the section on or off (issue 71). Unplaced, it starts through the model's middle, since
    /// a plane at zero may cut nothing and look broken.
    pub(super) fn toggle_section(&mut self) {
        let on = !self.scene.settings.section.enabled;
        self.scene.settings.section.enabled = on;
        if on && self.scene.settings.section.offset == 0.0 {
            let axis = self.scene.settings.section.axis();
            self.scene.settings.section.offset = crate::section_tool::middle_of(self.evaluated.bounds, axis);
            self.scene.settings.section.centre = self.model_middle();
        }
        self.status = Status::Info(match on {
            true => crate::section_tool::readout(self),
            false => "Section off".to_string(),
        });
    }

    /// Slide the section to `offset` along its axis. Not an edit, so no undo step
    /// (see [`crate::section_tool`]). The direction is kept for the motion-based side.
    pub fn set_section_offset(&mut self, offset: f64) {
        let section = self.section_mut();
        if offset != section.offset {
            section.swept_up = offset > section.offset;
        }
        section.offset = offset;
        self.status = Status::Info(crate::section_tool::readout(self));
    }

    /// Recentre the shown section's pivot and rectangle on the model's current middle.
    pub fn recentre_section(&mut self) {
        let middle = self.model_middle();
        self.section_mut().centre = middle;
    }

    /// The middle of the model, or nothing without one.
    fn model_middle(&self) -> Option<simple3d_geom::Vec3> {
        self.evaluated.bounds.map(|bounds| simple3d_core::scene::SectionView::pivot(Some(bounds)))
    }

    /// The section the window shows, which its fields and the setters act on.
    pub fn section(&self) -> &simple3d_core::scene::SectionView {
        self.scene.settings.section_at(self.section_tab)
    }

    pub fn section_mut(&mut self) -> &mut simple3d_core::scene::SectionView {
        self.scene.settings.section_at_mut(self.section_tab)
    }

    pub(super) fn toggle_axis(&mut self, axis: usize) {
        let on = !self.scene.settings.axes_visible[axis];
        self.scene.settings.axes_visible[axis] = on;
        let name = ["X", "Y", "Z"][axis];
        self.status = Status::Info(format!("{name} axis {}", if on { "shown" } else { "hidden" }));
    }

    /// Switch to a transform tool, putting the measure tool away and clearing its span.
    pub(super) fn pick_transform(&mut self, mode: Mode) {
        self.mode = mode;
        self.push_pull.drag = None;
        if mode == Mode::PushPull {
            self.status =
                Status::Info("Push / pull: drag a flat face out to add material, or in to cut it away".into());
        }
        if self.measure.active {
            self.measure.active = false;
            self.measure.clear();
        }
    }

    /// Toggle the measure tool; leaving it clears the span so it starts clean next time.
    pub fn toggle_measure(&mut self) {
        self.measure.active = !self.measure.active;
        if self.measure.active {
            self.status = Status::Info("Measure: click two features to read the span between them".into());
        } else {
            self.measure.clear();
            self.status = Status::Info("Measure tool off".into());
        }
    }
}
