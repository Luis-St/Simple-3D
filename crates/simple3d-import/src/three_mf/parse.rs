//! One 3MF model part read into its objects, colours and build items.

use super::*;

/// Read one model part. `part` is its [`Key`] path, the default for anything naming no other
/// part, and `range` is its share of the progress bar.
pub(super) fn parse(
    document: &str,
    part: &str,
    range: (f32, f32),
    progress: &mut Progress<'_>,
) -> Result<Parsed, ImportError> {
    let key = |tag: &crate::xml::Tag<'_>, id: usize| -> Key {
        (tag.attr("path").map_or_else(|| part.to_string(), |path| part_key(&path)), id)
    };
    let mut unit = None;
    // Colour groups by id; `<colorgroup>` and `<basematerials>` are both index-addressed colour lists.
    let mut palettes: HashMap<usize, Vec<[u8; 3]>> = HashMap::new();
    let mut palette_at: Option<usize> = None;
    let mut objects: HashMap<Key, Object> = HashMap::new();
    let mut order: Vec<Key> = Vec::new();
    let mut build: Vec<(Key, Transform)> = Vec::new();
    // The object being read, its declared colour group, and the vertices its triangles index into.
    let mut open: Option<(Key, Object)> = None;
    let mut object_palette: Option<usize> = None;
    let mut vertices: Vec<Vec3> = Vec::new();
    let mut in_build = false;

    let tags: Vec<_> = crate::xml::tags(document).collect();
    if !tags.iter().any(|tag| tag.opens("model")) {
        return Err(malformed("the model part is not a 3MF model document"));
    }
    for (index, tag) in tags.iter().enumerate() {
        if index % 4096 == 0 {
            step(progress, range.0 + range.1 * (index as f32 / tags.len().max(1) as f32))?;
        }
        if tag.opens("model") {
            // An unreadable unit falls back to the specification's default, millimetres.
            unit = tag.attr("unit").and_then(|value| Unit::from_3mf(&value));
        } else if tag.opens("colorgroup") || tag.opens("basematerials") {
            palette_at = tag.index("id");
            if let Some(id) = palette_at {
                palettes.entry(id).or_default();
            }
        } else if tag.closes("colorgroup") || tag.closes("basematerials") {
            palette_at = None;
        } else if tag.opens("color") || tag.opens("base") {
            // `<m:color color="#RRGGBB">` or `<base displaycolor="#RRGGBBAA">`: the same list either way.
            if let Some(id) = palette_at {
                let written = tag.attr("color").or_else(|| tag.attr("displaycolor"));
                if let Some(rgb) = written.as_deref().and_then(colour) {
                    palettes.entry(id).or_default().push(rgb);
                }
            }
        } else if tag.opens("object") {
            // An unclosed object (`<object id="1"/>`, which some generators emit) is kept, not dropped.
            if let Some((id, object)) = open.take() {
                order.push(id.clone());
                objects.insert(id, object);
            }
            let id = tag.index("id").ok_or_else(|| malformed("an object has no id"))?;
            let name = tag.attr("name").unwrap_or_default();
            object_palette = tag.index("pid");
            open = Some(((part.to_string(), id), Object { name, ..Object::default() }));
            vertices.clear();
        } else if tag.closes("object") {
            if let Some((id, object)) = open.take() {
                order.push(id.clone());
                objects.insert(id, object);
            }
            object_palette = None;
        } else if tag.opens("vertex") {
            let (x, y, z) = (tag.number("x"), tag.number("y"), tag.number("z"));
            let (Some(x), Some(y), Some(z)) = (x, y, z) else {
                return Err(malformed("a vertex does not carry three coordinates"));
            };
            vertices.push(Vec3::new(x, y, z));
        } else if tag.opens("triangle") {
            let Some((_, object)) = open.as_mut() else { continue };
            let (v1, v2, v3) = (tag.index("v1"), tag.index("v2"), tag.index("v3"));
            let (Some(v1), Some(v2), Some(v3)) = (v1, v2, v3) else {
                return Err(malformed("a triangle does not name three vertices"));
            };
            if v1.max(v2).max(v3) >= vertices.len() {
                return Err(malformed(format!("a triangle names vertex {} of {}", v1.max(v2).max(v3), vertices.len())));
            }
            // The colour the triangle names in its own group, or in its object's.
            let group = tag.index("pid").or(object_palette);
            let tag_value = group
                .and_then(|id| palettes.get(&id))
                .and_then(|palette| palette.get(tag.index("p1").unwrap_or(0)).copied())
                .filter(|rgb| *rgb != UNPAINTED)
                .map(colour_tag)
                .unwrap_or(0);
            object.mesh.push_tagged_triangle(vertices[v1], vertices[v2], vertices[v3], tag_value);
        } else if tag.opens("component") {
            let Some((_, object)) = open.as_mut() else { continue };
            let id = tag.index("objectid").ok_or_else(|| malformed("a component names no object"))?;
            let transform = match tag.attr("transform") {
                Some(value) => Transform::parse(&value)?,
                None => Transform::IDENTITY,
            };
            object.components.push((key(tag, id), transform));
        } else if tag.opens("build") {
            in_build = true;
        } else if tag.closes("build") {
            in_build = false;
        } else if tag.opens("item") && in_build {
            let id = tag.index("objectid").ok_or_else(|| malformed("a build item names no object"))?;
            let transform = match tag.attr("transform") {
                Some(value) => Transform::parse(&value)?,
                None => Transform::IDENTITY,
            };
            build.push((key(tag, id), transform));
        }
    }

    if let Some((id, object)) = open.take() {
        order.push(id.clone());
        objects.insert(id, object);
    }
    Ok(Parsed { unit, objects, order, build })
}

/// The neutral an export writes for unpainted faces, since 3MF has no "no colour" inside a
/// coloured object; read back as unpainted rather than as a grey the user never chose.
const UNPAINTED: [u8; 3] = [0x9A, 0xA4, 0xB2];

/// A colour as `#RRGGBB` or `#RRGGBBAA`; alpha is ignored.
fn colour(written: &str) -> Option<[u8; 3]> {
    let hex = written.trim().trim_start_matches('#');
    if hex.len() < 6 {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}
