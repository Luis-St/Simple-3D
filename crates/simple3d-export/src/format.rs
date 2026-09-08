//! The formats a model can be written in, what each can hold, and the unit
//! 3MF states its numbers in.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// The default, because it records units.
    ThreeMf,
    StlBinary,
    StlAscii,
    Obj,
    PlyBinary,
    PlyAscii,
}

impl Format {
    pub const ALL: [Format; 6] =
        [Format::ThreeMf, Format::StlBinary, Format::StlAscii, Format::Obj, Format::PlyBinary, Format::PlyAscii];

    pub fn label(self) -> &'static str {
        match self {
            Format::ThreeMf => "3MF",
            Format::StlBinary => "STL (binary)",
            Format::StlAscii => "STL (ASCII)",
            Format::Obj => "OBJ",
            Format::PlyBinary => "PLY (binary)",
            Format::PlyAscii => "PLY (ASCII)",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::ThreeMf => "3mf",
            Format::StlBinary | Format::StlAscii => "stl",
            Format::Obj => "obj",
            Format::PlyBinary | Format::PlyAscii => "ply",
        }
    }

    /// A stable identifier for remembering the user's last choice.
    pub fn id(self) -> &'static str {
        match self {
            Format::ThreeMf => "3mf",
            Format::StlBinary => "stl_binary",
            Format::StlAscii => "stl_ascii",
            Format::Obj => "obj",
            Format::PlyBinary => "ply_binary",
            Format::PlyAscii => "ply_ascii",
        }
    }

    pub fn from_id(id: &str) -> Option<Format> {
        Format::ALL.iter().copied().find(|f| f.id() == id)
    }

    /// Whether the format records the unit its numbers are in. For the ones that
    /// do not, the export dialog states the assumed unit instead.
    pub fn carries_units(self) -> bool {
        self == Format::ThreeMf
    }

    /// Whether the format can hold several named objects rather than one body.
    /// 3MF has `<object>` and `<build><item>`; STL is a bag of triangles, and
    /// OBJ and PLY have no notion a slicer would read back as separate parts,
    /// so for those an export is one mesh whatever the option says.
    pub fn keeps_objects_separate(self) -> bool {
        self == Format::ThreeMf
    }
}

/// How an export decides what its objects are (issue 58). Only a format that
/// [`Format::keeps_objects_separate`] can act on anything but [`BodyMode::One`];
/// for the rest an export is one mesh whatever this says.
///
/// The writer only ever sees the difference between "one body" and "the parts I
/// was handed". Which parts those are is the caller's decision, and what the
/// other two modes name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BodyMode {
    /// Everything merged into a single solid, as an export always was.
    #[default]
    One,
    /// One object per top-level shape or group.
    TopLevel,
    /// One object per body the user has grouped the scene into.
    Selected,
}

impl BodyMode {
    pub const ALL: [BodyMode; 3] = [BodyMode::One, BodyMode::TopLevel, BodyMode::Selected];

    pub fn label(self) -> &'static str {
        match self {
            BodyMode::One => "One body",
            BodyMode::TopLevel => "Top level bodies",
            BodyMode::Selected => "User selected bodies",
        }
    }

    /// A stable identifier for remembering the user's last choice.
    pub fn id(self) -> &'static str {
        match self {
            BodyMode::One => "one",
            BodyMode::TopLevel => "top_level",
            BodyMode::Selected => "selected",
        }
    }

    pub fn from_id(id: &str) -> Option<BodyMode> {
        BodyMode::ALL.iter().copied().find(|mode| mode.id() == id)
    }

    /// Whether this mode keeps the parts it was handed apart.
    pub fn separates(self) -> bool {
        self != BodyMode::One
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Unit3mf {
    #[default]
    Millimeter,
    Centimeter,
    Meter,
    Inch,
}

impl Unit3mf {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Unit3mf::Millimeter => "millimeter",
            Unit3mf::Centimeter => "centimeter",
            Unit3mf::Meter => "meter",
            Unit3mf::Inch => "inch",
        }
    }
}
