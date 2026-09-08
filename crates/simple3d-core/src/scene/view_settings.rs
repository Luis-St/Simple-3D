//! How the scene is looked at: the origin axes, the preview viewport, and
//! the section plane.

use serde::{Deserialize, Serialize};

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
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SectionView {
    pub enabled: bool,
    /// Which axis the plane stands perpendicular to: 0 for X, 1 for Y, 2 for Z.
    #[serde(default)]
    pub axis: usize,
    /// Where along that axis it sits, in millimetres.
    #[serde(default)]
    pub offset: f64,
    /// Which side of it goes: the material past the plane along the axis, or
    /// the material before it.
    #[serde(default)]
    pub flipped: bool,
}

impl SectionView {
    /// The axis it stands on, clamped: a file naming a fourth axis reads as Z
    /// rather than as a panic.
    pub fn axis(&self) -> usize {
        self.axis.min(2)
    }

    /// The half-space the renderer cuts with, or `None` while the section is
    /// off -- which is the one question every drawing path asks.
    pub fn plane(&self) -> Option<simple3d_geom::section::Plane> {
        self.enabled.then(|| simple3d_geom::section::Plane::on_axis(self.axis(), self.offset, self.flipped))
    }

    pub fn axis_label(&self) -> &'static str {
        ["X", "Y", "Z"][self.axis()]
    }
}
