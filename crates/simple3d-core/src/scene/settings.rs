//! The settings a scene carries, so a reopened project looks as it was left.

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
    /// The move and resize step, separate from the grid spacing, since 1 mm is the usual step but a
    /// 1 mm grid is unreadable.
    #[serde(default = "default_snap_step")]
    pub snap_step: f64,
    /// The three origin axes, each switchable.
    #[serde(default = "all_axes")]
    pub axes_visible: [bool; 3],
    #[serde(default)]
    pub axis_style: AxisStyle,
    /// Mark where the principal planes cut solids, showing e.g. how much is below the build plate.
    #[serde(default = "default_true")]
    pub plane_marks: bool,
    /// What the viewport hides under a tool preview (issue 82); omitted from the file at its default.
    #[serde(default, skip_serializing_if = "is_no_change")]
    pub preview_viewport: PreviewViewport,
    /// The on-screen section plane (issue 71), off and omitted until used. Also the first section,
    /// whose `enabled` switches them all; kept separate so older single-section files still open.
    #[serde(default, skip_serializing_if = "is_off")]
    pub section: SectionView,
    /// The sections after the first, all cutting at once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub more_sections: Vec<SectionView>,
}

impl Default for SceneSettings {
    fn default() -> Self {
        SceneSettings {
            unit: Unit::Millimetre,
            // 32 segments keeps a 3 mm pin smooth and a 2 m cylinder acceptable (spec section 5.1).
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
    /// How many sections there are.
    pub fn section_count(&self) -> usize {
        1 + self.more_sections.len()
    }

    /// Section `index`, clamped to the last.
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

    /// Add a section after the last and return its index.
    pub fn add_section(&mut self, section: SectionView) -> usize {
        self.more_sections.push(section);
        self.more_sections.len()
    }

    /// Remove section `index`; the only one cannot be removed. When the first goes, the next takes its
    /// place and keeps the on switch.
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
