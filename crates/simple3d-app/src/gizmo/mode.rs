//! Which manipulator is in use, and the handles it offers.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Move,
    Rotate,
    /// Rewrite the defining dimension; exact, and only on axes a parameter governs.
    Resize,
    /// Multiply by a factor; works on groups and ungoverned axes.
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
    MoveAxis(usize),
    /// Drag in the plane normal to this axis.
    MovePlane(usize),
    RotateRing(usize),
    /// A bounding-box face: axis and side.
    ResizeFace(usize, bool),
    /// A corner: side on each axis.
    ResizeCorner([bool; 3]),
}

impl Handle {
    /// Axes this handle affects, for colouring and the readout.
    pub fn axes(self) -> Vec<usize> {
        match self {
            Handle::MoveAxis(a) | Handle::RotateRing(a) | Handle::ResizeFace(a, _) => vec![a],
            Handle::MovePlane(a) => (0..3).filter(|&x| x != a).collect(),
            Handle::ResizeCorner(_) => vec![0, 1, 2],
        }
    }
}
