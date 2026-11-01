//! What the tools are set to: the sizes and counts shown under the tiles, whether new curves are
//! helper geometry, and which variant each family's tile last picked.

use super::Draw;

/// The most sides the polygon tools draw, and the most copies a pattern makes.
pub(crate) const MAX_SIDES: u32 = 64;
pub(crate) const MAX_COPIES: u32 = 64;

pub(crate) struct Settings {
    /// The radius the fillet tool and the rounded rectangle round corners with, in millimetres.
    pub fillet_radius: f64,
    /// How far along both lines the chamfer cuts a corner off.
    pub chamfer_distance: f64,
    /// How far the offset tool copies curves to the side.
    pub offset_distance: f64,
    /// How many sides the polygon tools draw.
    pub polygon_sides: u32,
    /// How many there are in a pattern, the original counted.
    pub pattern_count: u32,
    /// How far round a circular pattern spreads, in degrees; a full turn spaces the copies evenly.
    pub pattern_angle: f64,
    /// Whether the drawing tools put helper geometry down instead of the profile.
    pub construction: bool,
    /// Whether copied face edges come in as helper geometry rather than as part of the profile.
    pub copy_as_helper: bool,
    /// The variant each family's tile picks, the last one used.
    pub remembered: Vec<Draw>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            fillet_radius: 2.0,
            chamfer_distance: 2.0,
            offset_distance: 2.0,
            polygon_sides: 6,
            pattern_count: 3,
            pattern_angle: 360.0,
            construction: false,
            copy_as_helper: true,
            remembered: Vec::new(),
        }
    }
}

impl Settings {
    /// The variant `family`'s tile picks.
    pub(crate) fn variant(&self, family: &[Draw]) -> Draw {
        self.remembered.iter().copied().find(|d| family.contains(d)).unwrap_or(family[0])
    }

    /// Remember `draw` as its family's variant.
    pub(crate) fn remember(&mut self, draw: Draw) {
        let family = draw.family();
        self.remembered.retain(|d| !family.contains(d));
        self.remembered.push(draw);
    }

    pub(crate) fn sides(&self) -> u32 {
        self.polygon_sides.clamp(3, MAX_SIDES)
    }

    pub(crate) fn copies(&self) -> u32 {
        self.pattern_count.clamp(2, MAX_COPIES)
    }
}
