//! The PLY header: encoding, and the elements and properties it declares.

use super::*;

/// The scalar types PLY defines, under both spellings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scalar {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl Scalar {
    fn parse(word: &str) -> Option<Scalar> {
        match word {
            "char" | "int8" => Some(Scalar::I8),
            "uchar" | "uint8" => Some(Scalar::U8),
            "short" | "int16" => Some(Scalar::I16),
            "ushort" | "uint16" => Some(Scalar::U16),
            "int" | "int32" => Some(Scalar::I32),
            "uint" | "uint32" => Some(Scalar::U32),
            "float" | "float32" => Some(Scalar::F32),
            "double" | "float64" => Some(Scalar::F64),
            _ => None,
        }
    }

    pub(super) fn width(self) -> usize {
        match self {
            Scalar::I8 | Scalar::U8 => 1,
            Scalar::I16 | Scalar::U16 => 2,
            Scalar::I32 | Scalar::U32 | Scalar::F32 => 4,
            Scalar::F64 => 8,
        }
    }
}

/// One declared property: a number, or a count followed by that many numbers.
#[derive(Clone, Debug)]
pub(super) enum Property {
    Scalar { name: String, kind: Scalar },
    List { name: String, count: Scalar, item: Scalar },
}

#[derive(Clone, Debug)]
pub(super) struct Element {
    pub(super) name: String,
    pub(super) count: usize,
    pub(super) properties: Vec<Property>,
}

impl Element {
    /// Which of an element's properties hold the coordinates.
    pub(super) fn coordinate_slots(&self) -> [Option<usize>; 3] {
        let mut slots = [None, None, None];
        for (at, property) in self.properties.iter().enumerate() {
            if let Property::Scalar { name, .. } = property {
                match name.as_str() {
                    "x" => slots[0] = Some(at),
                    "y" => slots[1] = Some(at),
                    "z" => slots[2] = Some(at),
                    _ => {}
                }
            }
        }
        slots
    }

    /// The list property holding a face's vertices: `vertex_indices` per the specification,
    /// `vertex_index` as several programs write, or the only list property.
    pub(super) fn face_slot(&self) -> Option<usize> {
        let named = self.properties.iter().position(|property| {
            matches!(property, Property::List { name, .. } if name == "vertex_indices" || name == "vertex_index")
        });
        named.or_else(|| self.properties.iter().position(|property| matches!(property, Property::List { .. })))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Encoding {
    Ascii,
    Little,
    Big,
}

/// Read the header: encoding, declared elements, and where the body starts.
pub(super) fn header(bytes: &[u8]) -> Result<(Encoding, Vec<Element>, usize), ImportError> {
    const END: &[u8] = b"end_header";
    let end = (0..bytes.len().saturating_sub(END.len()))
        .find(|&at| &bytes[at..at + END.len()] == END)
        .ok_or_else(|| malformed("the header has no end_header line"))?;
    // Past the keyword and whatever line ending the file uses.
    let mut body_at = end + END.len();
    while body_at < bytes.len() && (bytes[body_at] == b'\r' || bytes[body_at] == b' ') {
        body_at += 1;
    }
    if body_at < bytes.len() && bytes[body_at] == b'\n' {
        body_at += 1;
    }

    let mut encoding = None;
    let mut elements: Vec<Element> = Vec::new();
    for line in text(&bytes[..end]).lines() {
        let mut words = line.split_whitespace();
        let Some(keyword) = words.next() else { continue };
        match keyword {
            "ply" | "comment" | "obj_info" => {}
            "format" => {
                encoding = match words.next() {
                    Some("ascii") => Some(Encoding::Ascii),
                    Some("binary_little_endian") => Some(Encoding::Little),
                    Some("binary_big_endian") => Some(Encoding::Big),
                    other => {
                        return Err(malformed(format!("{:?} is not a PLY encoding", other.unwrap_or(""))));
                    }
                };
            }
            "element" => {
                let name = words.next().unwrap_or("").to_string();
                let count = words
                    .next()
                    .and_then(|word| word.parse::<usize>().ok())
                    .ok_or_else(|| malformed(format!("element {name} does not say how many it holds")))?;
                elements.push(Element { name, count, properties: Vec::new() });
            }
            "property" => {
                let element =
                    elements.last_mut().ok_or_else(|| malformed("a property is declared before any element"))?;
                let kind = words.next().unwrap_or("");
                if kind == "list" {
                    let count = Scalar::parse(words.next().unwrap_or(""))
                        .ok_or_else(|| malformed("a list property's count is not a PLY type"))?;
                    let item = Scalar::parse(words.next().unwrap_or(""))
                        .ok_or_else(|| malformed("a list property's items are not a PLY type"))?;
                    let name = words.next().unwrap_or("").to_string();
                    element.properties.push(Property::List { name, count, item });
                } else {
                    let kind = Scalar::parse(kind).ok_or_else(|| malformed(format!("{kind:?} is not a PLY type")))?;
                    let name = words.next().unwrap_or("").to_string();
                    element.properties.push(Property::Scalar { name, kind });
                }
            }
            _ => {}
        }
    }
    let encoding = encoding.ok_or_else(|| malformed("the header does not say how the file is encoded"))?;
    Ok((encoding, elements, body_at))
}
