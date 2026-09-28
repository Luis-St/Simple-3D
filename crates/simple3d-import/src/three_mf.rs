//! 3MF: the package, its objects, and what the build places.
//!
//! Objects become parts, assemblies are flattened into the part that places them, units are
//! converted to millimetres, and face colours are kept. Components may reach into other model
//! parts via the Production extension's `p:path` (Bambu Studio and OrcaSlicer put every mesh in
//! `3D/Objects/object_N.model`), so objects are keyed by part and id.
//!
//! Print tickets, slicer settings and thumbnails are deliberately ignored.

use super::*;
use simple3d_geom::{colour_tag, Vec3};
use std::collections::HashMap;

mod parse;
use parse::parse;

/// A 3MF transform: twelve numbers, row-major. 3MF multiplies a row vector by the matrix, so the
/// translation is the last row.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Transform([f64; 12]);

impl Transform {
    const IDENTITY: Transform = Transform([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);

    /// Parse a `transform` attribute; an unreadable one is an error rather than a misplaced part.
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

    /// `self` applied first, then `outer`.
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

    /// Whether the transform mirrors, in which case winding is flipped back to keep the surface
    /// outward-facing.
    fn mirrors(&self) -> bool {
        let m = &self.0;
        let determinant = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
            + m[2] * (m[3] * m[7] - m[4] * m[6]);
        determinant < 0.0
    }
}

/// Where an object is declared: its model part (lower case, no leading slash) and its id.
type Key = (String, usize);

/// A package path as a [`Key`] spells it: case-insensitive, without `p:path`'s leading slash.
fn part_key(path: &str) -> String {
    path.trim().trim_start_matches('/').to_lowercase()
}

/// An object as declared: its own mesh, or other objects placed inside it.
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
    // A package with the model part elsewhere is still read, via the first part named like a model.
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

    // Read referenced parts until nothing new is named; their own builds are not placed.
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

/// Place what the build places, once every referenced part has been read.
fn assemble(parsed: Parsed) -> Result<Model, ImportError> {
    let Parsed { unit, objects, order, build } = parsed;
    // Without a build section, every object is placed where it stands rather than reading as empty.
    let placed: Vec<(Key, Transform)> =
        if build.is_empty() { order.iter().map(|id| (id.clone(), Transform::IDENTITY)).collect() } else { build };

    let scale = unit.map_or(1.0, Unit::in_millimetres);
    let mut parts = Vec::with_capacity(placed.len());
    for (id, transform) in &placed {
        let mut mesh = Mesh::new();
        // The chain of objects being placed stops self-referencing components from recursing forever.
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

/// Append the object `id` places, and everything it is assembled from, under `transform`.
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
