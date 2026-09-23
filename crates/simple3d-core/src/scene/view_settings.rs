//! How the scene is looked at: the origin axes, the preview viewport, and
//! the section plane.

use serde::{Deserialize, Serialize};
use simple3d_geom::Vec3;

/// How the origin axes are drawn. Two readings of the same three lines, kept
/// as a setting rather than a decision, because which one helps depends on
/// whether the axes are being used to place something or to read the ground.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AxisStyle {
    /// The way most 3D software draws them: X and Y *are* the coloured grid
    /// lines through zero. They run the width of the grid and travel with it as
    /// the view pans, so the ground always says which way is which.
    #[default]
    Grid,
    /// A fixed cross pinned at the origin, fading out at its own length. It
    /// says where the origin is rather than which way the ground runs, and it
    /// leaves the view once the origin is panned off screen.
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

/// What the viewport does while a tool draws a preview over it (issue 82).
///
/// An in-place popup exists so that one rectangle can be the modelling area and
/// the preview area at once. That only works if the preview can be seen, and
/// what is in the way depends on what is being previewed: a tiling drawn flat
/// on a plate competes with the grid it lies parallel to, a cut through a tall
/// shape competes with the axes running through it, and a preview of one object
/// in a crowded scene competes with the scene. Which of those is the nuisance
/// is not something the application can know, so it is a document setting.
///
/// It applies only while a preview is actually being drawn, and puts everything
/// back the moment the tool closes: this is not a way to turn the grid off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewViewport {
    /// The viewport carries on as it is, and the preview is drawn over it.
    #[default]
    NoChange,
    /// Drop the origin axes while the preview is up.
    HideAxes,
    /// Drop the ground grid while the preview is up.
    HideGrid,
    /// Drop both: the ground the tiling lies parallel to and the axes running
    /// through the shape are the same nuisance twice, and a tiling drawn flat
    /// on a plate meets both at once.
    HideGridAndAxes,
    /// Nothing but the object being previewed: every other body goes, and so do
    /// the grid and the axes. The strongest answer, for reading a fine pattern
    /// against one shape.
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

    /// Whether the grid is drawn under a preview in this mode.
    pub fn keeps_grid(self) -> bool {
        matches!(self, PreviewViewport::NoChange | PreviewViewport::HideAxes)
    }

    /// Whether the origin axes are drawn under a preview in this mode.
    pub fn keeps_axes(self) -> bool {
        matches!(self, PreviewViewport::NoChange | PreviewViewport::HideGrid)
    }

    /// Whether everything but the previewed object is drawn.
    pub fn keeps_other_bodies(self) -> bool {
        self != PreviewViewport::PreviewOnly
    }
}

/// A plane that cuts the model on screen so its inside can be seen and a wall
/// can be measured by eye (issue 71).
///
/// It belongs to the document rather than to the application: the offset is a
/// place in the model, and "40mm along X" means nothing in the next project.
/// Nothing about it reaches the geometry -- the model, what is exported and
/// what is picked are what they were, and only the picture changes -- so
/// moving the plane is not an edit and there is nothing to undo.
///
/// Which side of it goes is [`SectionKeep`]: by default the side the camera is
/// on (issue 109), or a fixed one when the cut should stay put while the view
/// goes round it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SectionView {
    pub enabled: bool,
    /// Which axis the plane stands perpendicular to before it is turned: 0 for
    /// X, 1 for Y, 2 for Z.
    #[serde(default)]
    pub axis: usize,
    /// Where along that axis it sits, in millimetres. Once it is turned, it is
    /// how far it has been slid along its own normal, counted so that an
    /// untilted plane reads as the coordinate it stands at.
    #[serde(default)]
    pub offset: f64,
    /// How far the plane is turned about X, then Y, then Z, in degrees, about
    /// the middle of the model (issue 109) -- the order a node's rotation uses.
    #[serde(default, skip_serializing_if = "is_untilted")]
    pub tilt: [f64; 3],
    /// Which side of the plane stays in the picture.
    #[serde(default, skip_serializing_if = "SectionKeep::is_auto")]
    pub keep: SectionKeep,
    /// Whether the plane was last slid along its normal rather than against
    /// it, which is what [`SectionKeep::Motion`] decides the side by.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub swept_up: bool,
    /// Whether the plane is cut down to [`SectionView::size`] rather than
    /// running through the whole model. Only the material straight behind the
    /// rectangle goes then, so a pocket can be opened in one place and the
    /// rest of the part stays whole round it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub custom_size: bool,
    /// The rectangle's width and height, in millimetres, along the two
    /// directions the plane spans (see [`SectionView::basis`]). Kept while the
    /// size is automatic, so switching back to custom finds it where it was.
    #[serde(default, skip_serializing_if = "is_unsized")]
    pub size: [f64; 2],
}

fn is_unsized(size: &[f64; 2]) -> bool {
    *size == [0.0; 2]
}

/// Which side of the section plane is kept.
///
/// `Auto` keeps the side away from the camera, so the cut always faces whoever
/// is looking and orbiting round shows the other half (issue 109). The fixed
/// two are named by the coordinate rather than by the camera -- below is the
/// side the plane's axis points away from, turned with the plane when it is
/// tilted -- and stay put however the model is orbited, which is what a cut
/// being compared from several angles wants.
///
/// `Motion` goes by the way the plane was last slid: what it has passed over
/// goes and what lies ahead of it stays, so pushing it from the front of a part
/// to the back peels the part away in front of it, and pulling it back brings
/// that half back and takes the other.
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
    /// The axis it stands on, clamped: a file naming a fourth axis reads as Z
    /// rather than as a panic.
    pub fn axis(&self) -> usize {
        self.axis.min(2)
    }

    /// The way the plane faces and slides: its axis, turned by the tilt.
    pub fn normal(&self) -> Vec3 {
        let mut axis = Vec3::ZERO;
        match self.axis() {
            0 => axis.x = 1.0,
            1 => axis.y = 1.0,
            _ => axis.z = 1.0,
        }
        axis.rotate_xyz_deg(Vec3::new(self.tilt[0], self.tilt[1], self.tilt[2])).normalized()
    }

    /// The point the plane is turned about: the middle of the model, or the
    /// origin with nothing in the scene.
    pub fn pivot(bounds: Option<(Vec3, Vec3)>) -> Vec3 {
        bounds.map_or(Vec3::ZERO, |(lo, hi)| (lo + hi) * 0.5)
    }

    /// A point the plane passes through.
    ///
    /// The untilted plane through the pivot, slid to `offset` along its axis,
    /// then turned about the pivot: without the tilt that is exactly the plane
    /// at `offset`, whatever the model's bounds, and with it the plane swings
    /// about the middle of the shape rather than about the origin -- which for
    /// a part standing 300 mm out would swing it clean off the model.
    pub fn anchor(&self, bounds: Option<(Vec3, Vec3)>) -> Vec3 {
        let pivot = Self::pivot(bounds);
        let along = [pivot.x, pivot.y, pivot.z][self.axis()];
        pivot + self.normal() * (self.offset - along)
    }

    /// The two directions the plane spans, turned with it: the axes after its
    /// own, with the second one upright wherever the plane is upright, so a
    /// standing plane's height is its height.
    pub fn basis(&self) -> (Vec3, Vec3) {
        let axis = self.axis();
        let (u, v) = match axis {
            1 => (0, 2),
            _ => ((axis + 1) % 3, (axis + 2) % 3),
        };
        let unit = |a: usize| {
            let mut at = Vec3::ZERO;
            match a {
                0 => at.x = 1.0,
                1 => at.y = 1.0,
                _ => at.z = 1.0,
            }
            at.rotate_xyz_deg(Vec3::new(self.tilt[0], self.tilt[1], self.tilt[2])).normalized()
        };
        (unit(u), unit(v))
    }

    /// The half-space the renderer cuts with, or `None` while the section is
    /// off -- which is the one question every drawing path asks.
    ///
    /// `forward` is the way the camera looks, which [`SectionKeep::Auto`] cuts
    /// away the near side by, so the cut is always seen face on. A plane seen
    /// exactly edge-on takes the side its normal points to, which is as good as
    /// either.
    pub fn plane(&self, bounds: Option<(Vec3, Vec3)>, forward: Vec3) -> Option<simple3d_geom::section::Plane> {
        self.enabled.then(|| self.cut(bounds, forward))
    }

    /// The half-space this section cuts with, whether or not it is on: what
    /// [`SceneSettings::section_planes`](super::SceneSettings::section_planes)
    /// asks of every section once the tool as a whole is.
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

    /// Whether the plane is turned off its axis at all.
    pub fn tilted(&self) -> bool {
        !is_untilted(&self.tilt)
    }

    pub fn axis_label(&self) -> &'static str {
        ["X", "Y", "Z"][self.axis()]
    }
}
