//! The readable formats, how a file is recognised, and the units a file may use.

/// What this crate reads, one variant per format: binary and text STL are one, told apart by content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    ThreeMf,
    Stl,
    Obj,
    Ply,
}

impl Format {
    /// Every format, in the export dialog's order, so the filters read the same way round.
    pub const ALL: [Format; 4] = [Format::ThreeMf, Format::Stl, Format::Obj, Format::Ply];

    pub fn label(self) -> &'static str {
        match self {
            Format::ThreeMf => "3MF",
            Format::Stl => "STL",
            Format::Obj => "OBJ",
            Format::Ply => "PLY",
        }
    }

    /// The extensions a file of this format is named with, lowercase.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Format::ThreeMf => &["3mf"],
            Format::Stl => &["stl"],
            Format::Obj => &["obj"],
            Format::Ply => &["ply"],
        }
    }

    /// The format a file name claims, or `None`.
    pub fn from_path(path: &std::path::Path) -> Option<Format> {
        let extension = path.extension()?.to_string_lossy().to_lowercase();
        Format::ALL.into_iter().find(|format| format.extensions().contains(&extension.as_str()))
    }

    /// The format the content is, for formats with a signature; OBJ has none and is only named.
    /// Content wins over the name, since renamed files are common and harmless.
    pub fn sniff(bytes: &[u8]) -> Option<Format> {
        if bytes.starts_with(b"PK\x03\x04") {
            return Some(Format::ThreeMf);
        }
        if bytes.starts_with(b"ply") {
            return Some(Format::Ply);
        }
        if crate::stl::looks_like_stl(bytes) {
            return Some(Format::Stl);
        }
        None
    }
}

/// The unit a file states. Everything here is millimetres, so others are converted and reported.
/// Only 3MF carries a unit; the rest are assumed millimetres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Micron,
    Millimetre,
    Centimetre,
    Metre,
    Inch,
    Foot,
}

impl Unit {
    /// The unit a 3MF `<model unit="...">` names, or `None` for an undefined value.
    pub fn from_3mf(value: &str) -> Option<Unit> {
        match value.trim().to_lowercase().as_str() {
            "micron" => Some(Unit::Micron),
            "millimeter" | "millimetre" => Some(Unit::Millimetre),
            "centimeter" | "centimetre" => Some(Unit::Centimetre),
            "meter" | "metre" => Some(Unit::Metre),
            "inch" => Some(Unit::Inch),
            "foot" => Some(Unit::Foot),
            _ => None,
        }
    }

    /// How many millimetres one of this unit is.
    pub fn in_millimetres(self) -> f64 {
        match self {
            Unit::Micron => 0.001,
            Unit::Millimetre => 1.0,
            Unit::Centimetre => 10.0,
            Unit::Metre => 1000.0,
            Unit::Inch => 25.4,
            Unit::Foot => 304.8,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Unit::Micron => "microns",
            Unit::Millimetre => "millimetres",
            Unit::Centimetre => "centimetres",
            Unit::Metre => "metres",
            Unit::Inch => "inches",
            Unit::Foot => "feet",
        }
    }
}
