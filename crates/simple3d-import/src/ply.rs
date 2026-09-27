//! PLY, text and binary, in either byte order.
//!
//! The reader follows the header: vertex and face elements are found by name, and every other
//! property is skipped by its declared width, so files with extra per-vertex data still read.

use super::*;
use simple3d_geom::Vec3;

/// The scalar types PLY defines, under both spellings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scalar {
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

    fn width(self) -> usize {
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
enum Property {
    Scalar { name: String, kind: Scalar },
    List { name: String, count: Scalar, item: Scalar },
}

#[derive(Clone, Debug)]
struct Element {
    name: String,
    count: usize,
    properties: Vec<Property>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Encoding {
    Ascii,
    Little,
    Big,
}

pub(crate) fn read(bytes: &[u8], mut progress: Progress<'_>) -> Result<Model, ImportError> {
    let (encoding, elements, body_at) = header(bytes)?;
    step(&mut progress, 0.1)?;
    let (positions, faces) = match encoding {
        Encoding::Ascii => ascii_body(&text(&bytes[body_at..]), &elements, &mut progress)?,
        _ => binary_body(&bytes[body_at..], &elements, encoding == Encoding::Big, &mut progress)?,
    };
    let mut mesh = Mesh::new();
    mesh.positions = positions;
    for face in &faces {
        // Polygons are fanned, which suffices for the convex faces of a solid.
        for i in 1..face.len() - 1 {
            let tri = [face[0], face[i], face[i + 1]];
            if tri.iter().any(|&index| index as usize >= mesh.positions.len()) {
                return Err(malformed(format!(
                    "a face names vertex {} of {}",
                    tri.iter().max().copied().unwrap_or(0),
                    mesh.positions.len()
                )));
            }
            if tri[0] != tri[1] && tri[1] != tri[2] && tri[0] != tri[2] {
                mesh.indices.push(tri);
                mesh.tags.push(0);
            }
        }
    }
    Ok(Model { format: Format::Ply, unit: None, parts: vec![Part { name: String::new(), mesh }] })
}

/// Read the header: encoding, declared elements, and where the body starts.
fn header(bytes: &[u8]) -> Result<(Encoding, Vec<Element>, usize), ImportError> {
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

/// Which of an element's properties hold the coordinates.
fn coordinate_slots(element: &Element) -> [Option<usize>; 3] {
    let mut slots = [None, None, None];
    for (at, property) in element.properties.iter().enumerate() {
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
fn face_slot(element: &Element) -> Option<usize> {
    let named = element.properties.iter().position(|property| {
        matches!(property, Property::List { name, .. } if name == "vertex_indices" || name == "vertex_index")
    });
    named.or_else(|| element.properties.iter().position(|property| matches!(property, Property::List { .. })))
}

fn binary_body(
    bytes: &[u8],
    elements: &[Element],
    big_endian: bool,
    progress: &mut Progress<'_>,
) -> Result<(Vec<Vec3>, Vec<Vec<u32>>), ImportError> {
    let mut at = 0usize;
    let number = |at: &mut usize, kind: Scalar| -> Result<f64, ImportError> {
        let width = kind.width();
        let slice = bytes.get(*at..*at + width).ok_or_else(|| malformed("the file ends mid-element"))?;
        *at += width;
        let mut raw = [0u8; 8];
        raw[..width].copy_from_slice(slice);
        if big_endian {
            raw[..width].reverse();
        }
        let value = match kind {
            Scalar::I8 => raw[0] as i8 as f64,
            Scalar::U8 => raw[0] as f64,
            Scalar::I16 => i16::from_le_bytes([raw[0], raw[1]]) as f64,
            Scalar::U16 => u16::from_le_bytes([raw[0], raw[1]]) as f64,
            Scalar::I32 => i32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as f64,
            Scalar::U32 => u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as f64,
            Scalar::F32 => f32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as f64,
            Scalar::F64 => f64::from_le_bytes(raw),
        };
        Ok(value)
    };
    walk_body(elements, progress, true, |kind, _| number(&mut at, kind))
}

fn ascii_body(
    body: &str,
    elements: &[Element],
    progress: &mut Progress<'_>,
) -> Result<(Vec<Vec3>, Vec<Vec<u32>>), ImportError> {
    // One whitespace-separated stream, since an element's properties may span several lines.
    let mut words = body.split_whitespace();
    walk_body(elements, progress, false, |_, element| next_number(&mut words, element))
}

/// The positions and polygons of a body, read element by element through `next`, which is given
/// each number's declared type and its element's name. `reserve` sizes the output by the declared counts.
fn walk_body(
    elements: &[Element],
    progress: &mut Progress<'_>,
    reserve: bool,
    mut next: impl FnMut(Scalar, &str) -> Result<f64, ImportError>,
) -> Result<(Vec<Vec3>, Vec<Vec<u32>>), ImportError> {
    let mut positions = Vec::new();
    let mut faces = Vec::new();
    let total: usize = elements.iter().map(|element| element.count).sum::<usize>().max(1);
    let mut done = 0usize;

    for element in elements {
        let coordinates = coordinate_slots(element);
        let vertices = element.name == "vertex" && coordinates.iter().all(Option::is_some);
        let face_at = (element.name == "face").then(|| face_slot(element)).flatten();
        if reserve && vertices {
            positions.reserve(element.count);
        }
        if reserve && face_at.is_some() {
            faces.reserve(element.count);
        }
        for index in 0..element.count {
            let mut read = [0.0f64; 3];
            let mut list: Vec<u32> = Vec::new();
            for (slot, property) in element.properties.iter().enumerate() {
                match property {
                    Property::Scalar { kind, .. } => {
                        let value = next(*kind, &element.name)?;
                        for axis in 0..3 {
                            if coordinates[axis] == Some(slot) {
                                read[axis] = value;
                            }
                        }
                    }
                    Property::List { count, item, .. } => {
                        let length = next(*count, &element.name)?;
                        if !(0.0..=1_000_000.0).contains(&length) {
                            return Err(malformed(format!("a list of {length} items in element {}", element.name)));
                        }
                        let wanted = face_at == Some(slot);
                        for _ in 0..length as usize {
                            let value = next(*item, &element.name)?;
                            if wanted {
                                if !(0.0..=u32::MAX as f64).contains(&value) {
                                    return Err(malformed(format!("{value} is not a vertex index")));
                                }
                                list.push(value as u32);
                            }
                        }
                    }
                }
            }
            if vertices {
                if !read.iter().all(|v| v.is_finite()) {
                    return Err(malformed(format!("vertex {} has a coordinate that is not a number", index + 1)));
                }
                positions.push(Vec3::new(read[0], read[1], read[2]));
            }
            if face_at.is_some() && list.len() >= 3 {
                faces.push(list);
            }
            done += 1;
            if done.is_multiple_of(8192) {
                step(progress, 0.1 + 0.8 * (done as f32 / total as f32))?;
            }
        }
    }
    Ok((positions, faces))
}

fn next_number<'a>(words: &mut impl Iterator<Item = &'a str>, element: &str) -> Result<f64, ImportError> {
    let word = words.next().ok_or_else(|| malformed(format!("the file ends part-way through element {element}")))?;
    word.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| malformed(format!("{word:?} in element {element} is not a number")))
}
