//! Running a command, whatever asked for it.

use super::*;
use crate::gizmo::Mode;
use crate::view::ViewPreset;
use simple3d_core::config::DisplayMode;
use simple3d_core::keymap::Command;

impl App {
    // -- commands -----------------------------------------------------------

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
            Export => self.modal = Modal::Export,
            Quit => self.request_quit(),

            Undo => match {
                let label = self.history.undo(&mut self.scene);
                label
            } {
                Some(label) => {
                    self.after_history(&format!("Undid {label}"));
                }
                None => self.status = Status::Info("Nothing to undo".into()),
            },
            Redo => match {
                let label = self.history.redo(&mut self.scene);
                label
            } {
                Some(label) => self.after_history(&format!("Redid {label}")),
                None => self.status = Status::Info("Nothing to redo".into()),
            },
            Copy => self.copy_selection(false),
            Cut => self.copy_selection(true),
            Paste => self.paste(),
            Duplicate => self.duplicate(),
            Delete => self.delete_selection(),
            Group => self.group_selection(),
            Pattern => self.make_pattern(),
            ConvertToMesh => self.convert_selection_to_mesh(),
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

            // Reaching for a transform tool puts the measure tool away: only one
            // of them can own a click.
            ModeMove => self.pick_transform(Mode::Move),
            ModeRotate => self.pick_transform(Mode::Rotate),
            ModeResize => self.pick_transform(Mode::Resize),
            ModeScale => self.pick_transform(Mode::Scale),
            ToggleHandleFrame => {
                self.settings.handle_frame = self.settings.handle_frame.toggled();
                self.status = Status::Info(format!("Handles: {} frame", self.settings.handle_frame.label()));
            }
            MeasureTool => self.toggle_measure(),
            // A hold key, read live while a drag runs rather than acted on when
            // pressed, so pressing it on its own does nothing (issue 68).
            SnapToGeometry => {}
            NudgeLeft | NudgeRight | NudgeUp | NudgeDown | NudgeAway | NudgeToward => self.nudge(command),
        }
    }

    /// Switch the section plane on or off (issue 71).
    ///
    /// Switching it on puts it in the middle of the model along its axis unless
    /// it has already been placed somewhere. A plane left at zero cuts nothing
    /// at all for a part that stands beside the origin, and a section that
    /// appears to do nothing reads as a broken one rather than as a plane that
    /// needs sliding.
    pub(super) fn toggle_section(&mut self) {
        let on = !self.scene.settings.section.enabled;
        self.scene.settings.section.enabled = on;
        if on && self.scene.settings.section.offset == 0.0 {
            let axis = self.scene.settings.section.axis();
            self.scene.settings.section.offset = crate::section_tool::middle_of(self.evaluated.mesh.bounds(), axis);
        }
        self.status = Status::Info(match on {
            true => crate::section_tool::readout(self),
            false => "Section off".to_string(),
        });
    }

    /// Slide the section plane to `offset`, in millimetres along its own axis.
    /// Nothing about the model changes, so this is not an edit and there is no
    /// undo step for it -- see [`crate::section_tool`].
    pub fn set_section_offset(&mut self, offset: f64) {
        self.scene.settings.section.offset = offset;
        self.status = Status::Info(crate::section_tool::readout(self));
    }

    pub(super) fn toggle_axis(&mut self, axis: usize) {
        let on = !self.scene.settings.axes_visible[axis];
        self.scene.settings.axes_visible[axis] = on;
        let name = ["X", "Y", "Z"][axis];
        self.status = Status::Info(format!("{name} axis {}", if on { "shown" } else { "hidden" }));
    }

    /// Switch to a transform tool, which also takes the measure tool out of the
    /// pointer's way and clears its span.
    pub(super) fn pick_transform(&mut self, mode: Mode) {
        self.mode = mode;
        if self.measure.active {
            self.measure.active = false;
            self.measure.clear();
        }
    }

    /// Turn the measure tool on or off. Leaving it clears the span it was showing
    /// -- that is what "dismiss" means -- so the next time it is picked up it
    /// starts clean rather than with a stale line hanging in the scene.
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
