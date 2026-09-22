//! 3MF: the package, the objects inside it, and what the build places.
//!
//! A 3MF is the one format here with a structure worth keeping. Its objects are
//! meshes or assemblies of other objects, each placed by the build with a
//! transform of its own, and it records the unit its numbers are in and the
//! colours its faces are painted. All four are read: an object becomes a part,
//! an assembly is flattened into the part that places it, the numbers are
//! converted to millimetres, and a painted face comes back painted.
//!
//! An assembly may reach into another model part of the package: the Production
//! extension's `p:path` on a component names the part its object is declared
//! in. Bambu Studio and OrcaSlicer write every mesh that way, into
//! `3D/Objects/object_N.model`, and leave the main part holding nothing but the
//! assemblies that place them -- so an object is known by the part it is in as
//! well as by its id, since ids are only unique within one part.
//!
//! What is deliberately not read: print tickets, slicer settings, thumbnails
//! and the other parts a slicer adds to the package. They are not geometry, and
//! this application has nowhere to put them.

use super::*;
use simple3d_geom::{colour_tag, Vec3};
use std::collections::HashMap;

/// A 3MF transform: three basis vectors and a translation, written as twelve
/// numbers in row-major order. 3MF multiplies a *row* vector by the matrix, so
/// the translation is the last row rather than the last column.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Transform([f64; 12]);

impl Transform {
    const IDENTITY: Transform = Transform([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);

    /// Read the `transform` attribute's twelve numbers, `None` when it is
    /// absent and an error when it is there and is not twelve numbers -- a
    /// transform nobody can read is a part in the wrong place, which is worse
    /// than a refusal.
    fn parse(value: &str) -> Result<Transform, ImportError> {
        let numbers: Vec<f64> = value.split_whitespace().filter_map(|word| word.parse::<f64>().ok()).collect();
        if numbers.len() != 12 || !numbers.iter().all(|v| v.is_finite()) {
            return Err(malformed(format!("a transform of {:?} is not twelve numbers", value)));
        }
        let mut matrix = [0.0; 12];
        matrix.copy_from_slice(&numbers);
        Ok(Transform(matrix))
    }

    fn apply(&self, p: Vec3) -> Vec3 {
        let m = &self.0;
        Vec3::new(
            p.x * m[0] + p.y * m[3] + p.z * m[6] + m[9],
            p.x * m[1] + p.y * m[4] + p.z * m[7] + m[10],
            p.x * m[2] + p.y * m[5] + p.z * m[8] + m[11],
        )
    }

    /// `self` applied first, then `outer` -- which is how a component inside an
    /// object placed by the build reaches the plate.
    fn then(&self, outer: &Transform) -> Transform {
        let mut out = [0.0; 12];
        for row in 0..3 {
            for column in 0..3 {
                out[row * 3 + column] = (0..3).map(|k| self.0[row * 3 + k] * outer.0[k * 3 + column]).sum();
            }
        }
        let translated = outer.apply(Vec3::new(self.0[9], self.0[10], self.0[11]));
        out[9] = translated.x;
        out[10] = translated.y;
        out[11] = translated.z;
        Transform(out)
    }

    /// Whether the transform turns the model inside out. A mirrored object's
    /// triangles come out wound the other way round, so the winding is flipped
    /// back -- otherwise the surface is inward-facing and every check
    /// downstream reports a solid that is inside out.
    fn mirrors(&self) -> bool {
        let m = &self.0;
        let determinant = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
            + m[2] * (m[3] * m[7] - m[4] * m[6]);
        determinant < 0.0
    }
}

/// Where an object is declared: the model part holding it, as a package path
/// without its leading slash and in lower case, and its id within that part.
type Key = (String, usize);

/// A package path as a [`Key`] spells it. Paths in a package are compared
/// without case, and a `p:path` is written from the package root with a
/// leading slash that the archive's own names do not carry.
fn part_key(path: &str) -> String {
    path.trim().trim_start_matches('/').to_lowercase()
}

/// An object as the resources declare it: either its own mesh, or a list of
/// other objects placed inside it.
#[derive(Clone, Debug, Default)]
struct Object {
    name: String,
    mesh: Mesh,
    components: Vec<(Key, Transform)>,
}

/// What one model part declares.
#[derive(Default)]
struct Parsed {
    unit: Option<Unit>,
    objects: HashMap<Key, Object>,
    order: Vec<Key>,
    build: Vec<(Key, Transform)>,
}

pub(crate) fn read(bytes: &[u8], mut progress: Progress<'_>) -> Result<Model, ImportError> {
    let archive = crate::unzip::Archive::open(bytes).map_err(malformed)?;
    // The model part is at a fixed place in every 3MF this workspace writes and
    // in every one written by a slicer; a package that puts it elsewhere is
    // still read, by taking the first part named like a model.
    let root_name = archive
        .names()
        .find(|name| name.eq_ignore_ascii_case("3D/3dmodel.model"))
        .or_else(|| archive.names().find(|name| name.to_lowercase().ends_with(".model")))
        .map(str::to_string)
        .ok_or_else(|| {
            let held: Vec<&str> = archive.names().take(8).collect();
            malformed(format!("the package holds no model part -- only {}", held.join(", ")))
        })?;
    let read_part = |key: &str| -> Result<Vec<u8>, ImportError> {
        archive
            .read_by(|name| part_key(name) == key)
            .ok_or_else(|| malformed(format!("a component is declared in /{key}, which the package does not hold")))?
            .map_err(malformed)
    };
    let root = part_key(&root_name);
    let part = read_part(&root)?;
    step(&mut progress, 0.2)?;
    let mut parsed = parse(&text(&part), &root, (0.2, 0.3), &mut progress)?;

    // The parts the assemblies reach into, read once each. A part read this
    // way may itself name another, so this goes on until nothing new is named;
    // its own build, if it has one, is not the package's and is not placed.
    let mut loaded = vec![root];
    loop {
        let wanted: Vec<String> = parsed
            .objects
            .values()
            .flat_map(|object| object.components.iter().map(|((part, _), _)| part.clone()))
            .chain(parsed.build.iter().map(|((part, _), _)| part.clone()))
            .filter(|part| !loaded.contains(part))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if wanted.is_empty() {
            break;
        }
        let span = 0.6 / wanted.len() as f32;
        for (at, key) in wanted.into_iter().enumerate() {
            let bytes = read_part(&key)?;
            let from = 0.3 + span * at as f32;
            let inner = parse(&text(&bytes), &key, (from, span), &mut progress)?;
            parsed.objects.extend(inner.objects);
            loaded.push(key);
        }
    }
    assemble(parsed)
}

/// Read one model part. `part` is its own [`Key`] path, which every object,
/// component and build item in it that names no other part belongs to, and
/// `range` is the stretch of the progress bar its tags are reported across.
fn parse(document: &str, part: &str, range: (f32, f32), progress: &mut Progress<'_>) -> Result<Parsed, ImportError> {
    let key = |tag: &crate::xml::Tag<'_>, id: usize| -> Key {
        (tag.attr("path").map_or_else(|| part.to_string(), |path| part_key(&path)), id)
    };
    let mut unit = None;
    // Every colour the resources declare, by the id of the group holding them.
    // A `<colorgroup>` and a `<basematerials>` are read into the same shape:
    // both are a list of colours a triangle names by index.
    let mut palettes: HashMap<usize, Vec<[u8; 3]>> = HashMap::new();
    let mut palette_at: Option<usize> = None;
    let mut objects: HashMap<Key, Object> = HashMap::new();
    let mut order: Vec<Key> = Vec::new();
    let mut build: Vec<(Key, Transform)> = Vec::new();
    // The object being read, its declared colour group, and the vertices its
    // triangles index into.
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
            // An unreadable unit is not a reason to refuse the file: the
            // specification's default is millimetres, which is what everything
            // here is in anyway.
            unit = tag.attr("unit").and_then(|value| Unit::from_3mf(&value));
        } else if tag.opens("colorgroup") || tag.opens("basematerials") {
            palette_at = tag.index("id");
            if let Some(id) = palette_at {
                palettes.entry(id).or_default();
            }
        } else if tag.closes("colorgroup") || tag.closes("basematerials") {
            palette_at = None;
        } else if tag.opens("color") || tag.opens("base") {
            // `<m:color color="#RRGGBB">` in a colour group, `<base
            // displaycolor="#RRGGBBAA">` in base materials: the same list, read
            // out of whichever attribute the file uses.
            if let Some(id) = palette_at {
                let written = tag.attr("color").or_else(|| tag.attr("displaycolor"));
                if let Some(rgb) = written.as_deref().and_then(colour) {
                    palettes.entry(id).or_default().push(rgb);
                }
            }
        } else if tag.opens("object") {
            // An object left unclosed -- `<object id="1"/>`, which declares
            // nothing and which some generators emit -- is taken as it stands
            // rather than dropped when the next one opens.
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
            // The colour the triangle names in its own group, or in the one its
            // object declared.
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

/// Place what the build places, now every part it reaches into has been read.
fn assemble(parsed: Parsed) -> Result<Model, ImportError> {
    let Parsed { unit, objects, order, build } = parsed;
    // A package with no build section still holds its objects, and every
    // program that writes one writes the build too -- but a file that does not
    // is read as "everything, where it stands" rather than as empty.
    let placed: Vec<(Key, Transform)> =
        if build.is_empty() { order.iter().map(|id| (id.clone(), Transform::IDENTITY)).collect() } else { build };

    let scale = unit.map_or(1.0, Unit::in_millimetres);
    let mut parts = Vec::with_capacity(placed.len());
    for (id, transform) in &placed {
        let mut mesh = Mesh::new();
        // A component that places itself, directly or round a ring of other
        // objects, would recurse forever; the chain of objects already being
        // placed is what stops it.
        flatten(&objects, id, transform, &mut Vec::new(), &mut mesh)?;
        if scale != 1.0 {
            for position in mesh.positions.iter_mut() {
                *position = *position * scale;
            }
        }
        if mesh.triangle_count() == 0 {
            continue;
        }
        let name = objects.get(id).map(|object| object.name.clone()).unwrap_or_default();
        parts.push(Part { name, mesh });
    }
    Ok(Model { format: Format::ThreeMf, unit, parts })
}

/// The neutral an export writes for a face nobody painted. 3MF has no "no
/// colour" for a triangle inside a coloured object, so the exporter writes the
/// colour the viewport would have drawn -- and reading it back as a painted
/// face would repaint a model in a grey the user never chose.
const UNPAINTED: [u8; 3] = [0x9A, 0xA4, 0xB2];

/// Append the object `id` places, and everything it is assembled from, under
/// `transform`.
fn flatten(
    objects: &HashMap<Key, Object>,
    id: &Key,
    transform: &Transform,
    chain: &mut Vec<Key>,
    out: &mut Mesh,
) -> Result<(), ImportError> {
    let number = id.1;
    if chain.contains(id) {
        return Err(malformed(format!("object {number} is assembled out of itself")));
    }
    let Some(object) = objects.get(id) else {
        return Err(malformed(format!("the build places object {number}, which the file does not declare")));
    };
    chain.push(id.clone());
    if object.mesh.triangle_count() > 0 {
        let base = out.positions.len() as u32;
        out.positions.extend(object.mesh.positions.iter().map(|p| transform.apply(*p)));
        let flip = transform.mirrors();
        for (triangle, index) in object.mesh.indices.iter().enumerate() {
            let placed = if flip {
                [base + index[0], base + index[2], base + index[1]]
            } else {
                [base + index[0], base + index[1], base + index[2]]
            };
            out.indices.push(placed);
            out.tags.push(object.mesh.tag(triangle));
        }
    }
    for (component, inner) in &object.components {
        flatten(objects, component, &inner.then(transform), chain, out)?;
    }
    chain.pop();
    Ok(())
}

/// A colour written as `#RRGGBB` or `#RRGGBBAA`. The alpha is read past: a
/// node here is opaque, and there is nowhere to keep it.
fn colour(written: &str) -> Option<[u8; 3]> {
    let hex = written.trim().trim_start_matches('#');
    if hex.len() < 6 {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}
