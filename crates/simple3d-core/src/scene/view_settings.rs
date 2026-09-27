//! How the scene is viewed: origin axes, preview viewport, and section plane.

use serde::{Deserialize, Serialize};
use simple3d_geom::Vec3;

/// How the origin axes are drawn; a setting because either reading can be the useful one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AxisStyle {
    /// X and Y are the coloured grid lines through zero, running the grid's width.
    #[default]
    Grid,
    /// A fixed cross at the origin that fades at its own length.
    Origin,
}

impl AxisStyle {
    pub const ALL: [AxisStyle; 2] = [AxisStyle::Grid, AxisStyle::Origin];

    pub fn label(self) -> &'static str {
        match self {
            AxisStyle::Grid => "Along the grid",
            AxisStyle::Origin => "Pinned at the origin",
        }
    }
}

/// What the viewport hides while a tool draws a preview over it (issue 82). A document setting,
/// since which element gets in the way depends on the preview; everything returns when the tool
/// closes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewViewport {
    /// The viewport is unchanged and the preview drawn over it.
    #[default]
    NoChange,
    /// Hide the origin axes during the preview.
    HideAxes,
    /// Hide the ground grid during the preview.
    HideGrid,
    /// Hide both grid and axes.
    HideGridAndAxes,
    /// Show only the previewed object: other bodies, grid and axes are hidden.
    PreviewOnly,
}

impl PreviewViewport {
    pub const ALL: [PreviewViewport; 5] = [
        PreviewViewport::NoChange,
        PreviewViewport::HideAxes,
        PreviewViewport::HideGrid,
        PreviewViewport::HideGridAndAxes,
        PreviewViewport::PreviewOnly,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PreviewViewport::NoChange => "No change",
            PreviewViewport::HideAxes => "Hide the axes",
            PreviewViewport::HideGrid => "Hide the grid",
            PreviewViewport::HideGridAndAxes => "Hide the grid and axes",
            PreviewViewport::PreviewOnly => "Only what is previewed",
        }
    }

    pub fn keeps_grid(self) -> bool {
        matches!(self, PreviewViewport::NoChange | PreviewViewport::HideAxes)
    }

    pub fn keeps_axes(self) -> bool {
        matches!(self, PreviewViewport::NoChange | PreviewViewport::HideGrid)
    }

    pub fn keeps_other_bodies(self) -> bool {
        self != PreviewViewport::PreviewOnly
    }
}

/// A plane cutting the model on screen to see inside (issue 71).
///
/// A document setting, since its offset is a place in the model. It affects only the picture,
/// never geometry, export or picking, so moving it is not an undoable edit. Which side is kept
/// is [`SectionKeep`] (issue 109).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SectionView {
    pub enabled: bool,
    /// The axis the untilted plane is perpendicular to: 0 X, 1 Y, 2 Z.
    #[serde(default)]
    pub axis: usize,
    /// Position along that axis in millimetres; when tilted, the slide along its own normal.
    #[serde(default)]
    pub offset: f64,
    /// Rotation about X, then Y, then Z in degrees, about the model's middle (issue 109).
    #[serde(default, skip_serializing_if = "is_untilted")]
    pub tilt: [f64; 3],
    /// Which side of the plane stays in the picture.
    #[serde(default, skip_serializing_if = "SectionKeep::is_auto")]
    pub keep: SectionKeep,
    /// Whether the plane was last slid along its normal, for [`SectionKeep::Motion`].
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub swept_up: bool,
    /// Whether the plane is limited to [`SectionView::size`], cutting only behind the rectangle.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub custom_size: bool,
    /// The rectangle's size in millimetres along [`SectionView::basis`]; kept while automatic.
    #[serde(default, skip_serializing_if = "is_unsized")]
    pub size: [f64; 2],
    /// The pivot and rectangle centre, fixed when the plane is placed, so moving bodies does not
    /// swing or slide the cut. `None` in older files or with no model; the model's middle is used then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centre: Option<Vec3>,
}

fn is_unsized(size: &[f64; 2]) -> bool {
    *size == [0.0; 2]
}

/// Which side of the section plane is kept.
///
/// `Auto` keeps the side away from the camera, so the cut always faces the viewer (issue 109).
/// The fixed sides are named by coordinate and stay put while orbiting. `Motion` removes what
/// the plane was last slid over.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SectionKeep {
    #[default]
    Auto,
    Below,
    Above,
    Motion,
}

impl SectionKeep {
    fn is_auto(&self) -> bool {
        *self == SectionKeep::Auto
    }
}

fn is_untilted(tilt: &[f64; 3]) -> bool {
    *tilt == [0.0; 3]
}

impl SectionView {
    /// The axis, clamped so an invalid file reads as Z rather than panicking.
    pub fn axis(&self) -> usize {
        self.axis.min(2)
    }

    /// The plane's normal: its axis turned by the tilt.
    pub fn normal(&self) -> Vec3 {
        Vec3::axis(self.axis()).rotate_xyz_deg(Vec3::new(self.tilt[0], self.tilt[1], self.tilt[2])).normalized()
    }

    /// The middle of the model, or the origin for an empty scene.
    pub fn pivot(bounds: Option<(Vec3, Vec3)>) -> Vec3 {
        bounds.map_or(Vec3::ZERO, |(lo, hi)| (lo + hi) * 0.5)
    }

    /// The point the plane turns about: [`SectionView::centre`], or the model's middle.
    pub fn turned_about(&self, bounds: Option<(Vec3, Vec3)>) -> Vec3 {
        self.centre.unwrap_or_else(|| Self::pivot(bounds))
    }

    /// Fix the pivot at the model's middle, if unset and there is a model.
    pub fn pin_centre(&mut self, bounds: Option<(Vec3, Vec3)>) {
        if self.centre.is_none() && bounds.is_some() {
            self.centre = Some(Self::pivot(bounds));
        }
    }

    /// A point on the plane: the untilted plane at `offset` through the centre, turned about the
    /// centre rather than the origin, which would swing it off a distant model.
    pub fn anchor(&self, bounds: Option<(Vec3, Vec3)>) -> Vec3 {
        let pivot = self.turned_about(bounds);
        let along = [pivot.x, pivot.y, pivot.z][self.axis()];
        pivot + self.normal() * (self.offset - along)
    }

    /// Where a full plane's frame is drawn: the model's middle projected onto the plane.
    pub fn frame_middle(&self, bounds: Option<(Vec3, Vec3)>) -> Vec3 {
        let normal = self.normal();
        let middle = Self::pivot(bounds);
        middle - normal * normal.dot(middle - self.anchor(bounds))
    }

    /// The two directions the plane spans, turned with it; the second stays upright for an upright plane.
    pub fn basis(&self) -> (Vec3, Vec3) {
        let axis = self.axis();
        let (u, v) = match axis {
            1 => (0, 2),
            _ => ((axis + 1) % 3, (axis + 2) % 3),
        };
        let unit =
            |a: usize| Vec3::axis(a).rotate_xyz_deg(Vec3::new(self.tilt[0], self.tilt[1], self.tilt[2])).normalized();
        (unit(u), unit(v))
    }

    /// The half-space the renderer cuts with, or `None` while off. `forward` decides the side for
    /// [`SectionKeep::Auto`]; exactly edge-on takes the normal's side.
    pub fn plane(&self, bounds: Option<(Vec3, Vec3)>, forward: Vec3) -> Option<simple3d_geom::section::Plane> {
        self.enabled.then(|| self.cut(bounds, forward))
    }

    /// The half-space this section cuts with, on or not, for
    /// [`SceneSettings::section_planes`](super::SceneSettings::section_planes).
    pub fn cut(&self, bounds: Option<(Vec3, Vec3)>, forward: Vec3) -> simple3d_geom::section::Plane {
        let normal = self.normal();
        let at = normal.dot(self.anchor(bounds));
        let flipped = match self.keep {
            SectionKeep::Auto => normal.dot(forward) > 0.0,
            SectionKeep::Below => false,
            SectionKeep::Above => true,
            // Slid along the normal, it has passed over what lies against it.
            SectionKeep::Motion => self.swept_up,
        };
        let plane = match flipped {
            true => simple3d_geom::section::Plane::new(-normal, -at),
            false => simple3d_geom::section::Plane::new(normal, at),
        };
        // Cut down to its rectangle, which is the frame drawn round it.
        match self.custom_size {
            true => {
                let (u, v) = self.basis();
                let half = self.size.map(|side| side.max(0.0) * 0.5);
                plane.within(simple3d_geom::section::Window { centre: self.anchor(bounds), u, v, half })
            }
            false => plane,
        }
    }

    /// Whether the plane is tilted at all.
    pub fn tilted(&self) -> bool {
        !is_untilted(&self.tilt)
    }

    pub fn axis_label(&self) -> &'static str {
        ["X", "Y", "Z"][self.axis()]
    }
}
