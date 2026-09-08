//! Which manipulator is in use, and the handles it offers.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Move,
    Rotate,
    /// Rewrite the dimension the shape is defined by. Exact, and only offered on
    /// an axis some parameter actually governs.
    Resize,
    /// Multiply what is there by a factor. Works on a group, and on an axis no
    /// parameter governs, because it does not have to know what anything means.
    Scale,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Move, Mode::Rotate, Mode::Resize, Mode::Scale];

    pub fn label(self) -> &'static str {
        match self {
            Mode::Move => "Move",
            Mode::Rotate => "Rotate",
            Mode::Resize => "Resize",
            Mode::Scale => "Scale",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// Drag along one axis.
    MoveAxis(usize),
    /// Drag in the plane whose normal is this axis, for two-axis movement.
    MovePlane(usize),
    /// Rotate about one axis.
    RotateRing(usize),
    /// A face of the selection's bounding box: which axis, and which side.
    ResizeFace(usize, bool),
    /// A corner: which side on each axis.
    ResizeCorner([bool; 3]),
}

impl Handle {
    /// The axes this handle affects, for colouring and for the readout.
    pub fn axes(self) -> Vec<usize> {
        match self {
            Handle::MoveAxis(a) | Handle::RotateRing(a) | Handle::ResizeFace(a, _) => vec![a],
            Handle::MovePlane(a) => (0..3).filter(|&x| x != a).collect(),
            Handle::ResizeCorner(_) => vec![0, 1, 2],
        }
    }
}
