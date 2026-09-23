//! The settings a scene carries with it, so a reopened project looks the
//! way it was left.

use super::*;
use crate::unit::Unit;
use serde::{Deserialize, Serialize};
use simple3d_geom::Vec3;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneSettings {
    pub unit: Unit,
    pub default_segments: u32,
    #[serde(default)]
    pub notes: String,
    pub grid_spacing: f64,
    pub grid_visible: bool,
    /// How far one step of a move or resize goes: the increment a drag snaps to
    /// and one press of a nudge key covers. Its own setting rather than the grid
    /// spacing, which is about what the ground looks like -- 1 mm is the step
    /// most people want and a 1 mm grid is unreadable.
    #[serde(default = "default_snap_step")]
    pub snap_step: f64,
    /// The three origin axes, each on its own. An axis running through the model
    /// is a distraction when it is not the one being worked to.
    #[serde(default = "all_axes")]
    pub axes_visible: [bool; 3],
    #[serde(default)]
    pub axis_style: AxisStyle,
    /// Draw, on the surface of a solid, the line where a principal plane cuts
    /// through it. Where the ground plane crosses a shape is a real dimension
    /// -- how much of it is below the build plate -- and it is invisible until
    /// something marks it.
    #[serde(default = "default_true")]
    pub plane_marks: bool,
    /// What the viewport does while a tool draws a preview over it (issue 82).
    /// Absent from the file while it is the default, so a project written by
    /// this version still diffs cleanly against one written before in-place
    /// previews existed.
    #[serde(default, skip_serializing_if = "is_no_change")]
    pub preview_viewport: PreviewViewport,
    /// The plane the model is cut with on screen (issue 71). Off, and absent
    /// from the file, until it is asked for.
    ///
    /// It is also the first of the sections, and the one whose `enabled` says
    /// whether any of them cut: the tool is on or off as a whole. Kept as its
    /// own field so a file written before there could be several still opens
    /// with the one it had.
    #[serde(default, skip_serializing_if = "is_off")]
    pub section: SectionView,
    /// The sections after the first, each cutting on its own at the same time
    /// as the rest: what the model shows is what all of them leave.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub more_sections: Vec<SectionView>,
}

impl Default for SceneSettings {
    fn default() -> Self {
        SceneSettings {
            unit: Unit::Millimetre,
            // 32 segments keeps a 3mm pin smooth and a 2m cylinder acceptable
            // without the user touching the setting (spec section 5.1).
            default_segments: 32,
            notes: String::new(),
            grid_spacing: 10.0,
            grid_visible: true,
            snap_step: default_snap_step(),
            axes_visible: all_axes(),
            axis_style: AxisStyle::default(),
            plane_marks: true,
            preview_viewport: PreviewViewport::NoChange,
            section: SectionView::default(),
            more_sections: Vec::new(),
        }
    }
}

impl SceneSettings {
    /// How many sections there are: the first, and those after it.
    pub fn section_count(&self) -> usize {
        1 + self.more_sections.len()
    }

    /// Section `index`, clamped to the last there is.
    pub fn section_at(&self, index: usize) -> &SectionView {
        match index {
            0 => &self.section,
            _ => {
                self.more_sections.get(index - 1).unwrap_or_else(|| self.more_sections.last().unwrap_or(&self.section))
            }
        }
    }

    pub fn section_at_mut(&mut self, index: usize) -> &mut SectionView {
        let last = self.more_sections.len();
        match index.min(last) {
            0 => &mut self.section,
            index => &mut self.more_sections[index - 1],
        }
    }

    /// Every section, first to last.
    pub fn sections(&self) -> impl Iterator<Item = &SectionView> {
        std::iter::once(&self.section).chain(self.more_sections.iter())
    }

    /// Add a section after the last, and say which it is.
    pub fn add_section(&mut self, section: SectionView) -> usize {
        self.more_sections.push(section);
        self.more_sections.len()
    }

    /// Take section `index` away. The first cannot go while it is the only
    /// one: with none left there would be nothing for the tool to show. When
    /// it goes and others are left, the next one takes its place, keeping the
    /// switch that says the tool is on.
    pub fn remove_section(&mut self, index: usize) {
        if index == 0 {
            if self.more_sections.is_empty() {
                return;
            }
            let enabled = self.section.enabled;
            self.section = self.more_sections.remove(0);
            self.section.enabled = enabled;
        } else if index <= self.more_sections.len() {
            self.more_sections.remove(index - 1);
        }
    }

    /// The planes every section cuts with, or none while the tool is off.
    pub fn section_planes(&self, bounds: Option<(Vec3, Vec3)>, forward: Vec3) -> Vec<simple3d_geom::section::Plane> {
        if !self.section.enabled {
            return Vec::new();
        }
        self.sections().map(|section| section.cut(bounds, forward)).collect()
    }
}
