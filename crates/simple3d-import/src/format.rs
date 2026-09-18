//! The formats a model can be read from, how a file is recognised as one, and
//! the units a file may state its numbers in.

/// What this crate reads. One variant per *format* rather than per encoding:
/// binary and text STL are the same format and the same extension, and which
/// one a file is is answered by looking at it rather than by asking the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    ThreeMf,
    Stl,
    Obj,
    Ply,
}

impl Format {
    /// Every format, in the order the export dialog lists its own -- so the
    /// file dialog's filters read the same way round as the ones beside them.
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

    /// The format a file name claims, `None` for one this crate does not read.
    pub fn from_path(path: &std::path::Path) -> Option<Format> {
        let extension = path.extension()?.to_string_lossy().to_lowercase();
        Format::ALL.into_iter().find(|format| format.extensions().contains(&extension.as_str()))
    }

    /// The format the *content* is, for the formats that say so in their first
    /// bytes. OBJ says nothing -- it is a text file of lines with no header at
    /// all -- so it is never sniffed, only named.
    ///
    /// Content is consulted before the name because a renamed file is common
    /// and harmless: a 3MF saved as `.stl` is still a 3MF, and refusing it on
    /// its extension would be refusing a model this crate can read perfectly
    /// well.
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

/// The unit a file states its numbers in. Everything above this crate is
/// millimetres, so a file that names anything else is converted on the way in
/// and the import says what it converted from.
///
/// Only 3MF carries a unit. The others are assumed to be millimetres, which is
/// what this workspace writes and what a slicer assumes of an STL.
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
    /// The unit a 3MF `<model unit="...">` names, `None` for a value the
    /// specification does not define.
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
