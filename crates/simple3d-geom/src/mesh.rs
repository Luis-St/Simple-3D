use crate::vec3::Vec3;

/// An indexed triangle mesh. `indices` holds flat triangle triples into `positions`.
/// A `Mesh` returned by a primitive generator or a boolean op is expected to be
/// watertight, manifold, correctly wound (CCW seen from outside) with outward normals.
///
/// `tags` runs parallel to `indices`: one number per triangle, saying which
/// body the surface came from. Nothing in this crate interprets it -- it is
/// carried through booleans, welding and healing so that a caller which paints
/// bodies (see the scene's per-node colour) can still tell, on the far side of
/// a difference, which of the operands each surviving face belonged to.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    pub indices: Vec<[u32; 3]>,
    pub tags: Vec<u32>,
}

/// The tag that stands for "this surface is painted this colour", and the
/// reading of one. The encoding lives here, next to the field it goes in, so
/// the scene that writes tags and the exporter that reads them cannot drift:
/// the top byte separates a painted surface from tag 0, which every generator
/// produces and which means "unpainted".
pub fn colour_tag(rgb: [u8; 3]) -> u32 {
    0x0100_0000 | (u32::from(rgb[0]) << 16) | (u32::from(rgb[1]) << 8) | u32::from(rgb[2])
}

/// The colour a tag stands for, or `None` for a surface nobody painted.
pub fn tag_colour(tag: u32) -> Option<[u8; 3]> {
    (tag & 0xFF00_0000 != 0).then_some([(tag >> 16) as u8, (tag >> 8) as u8, tag as u8])
}

impl Mesh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len()
    }

    pub fn push_triangle(&mut self, a: Vec3, b: Vec3, c: Vec3) {
        self.push_tagged_triangle(a, b, c, 0);
    }

    pub fn push_tagged_triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, tag: u32) {
        let base = self.positions.len() as u32;
        self.positions.push(a);
        self.positions.push(b);
        self.positions.push(c);
        self.indices.push([base, base + 1, base + 2]);
        self.tags.push(tag);
    }

    /// The tag of one triangle. Zero for a mesh built before tags mattered,
    /// which is what every generator produces until a caller says otherwise.
    pub fn tag(&self, triangle: usize) -> u32 {
        self.tags.get(triangle).copied().unwrap_or(0)
    }

    /// Mark every triangle as belonging to one body. Called once per primitive
    /// as evaluation collects it, before anything is combined.
    pub fn set_tag(&mut self, tag: u32) {
        self.tags.clear();
        self.tags.resize(self.indices.len(), tag);
    }

    /// Append another mesh's geometry, offsetting its indices.
    pub fn append(&mut self, other: &Mesh) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.indices.extend(other.indices.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
        self.tags.extend((0..other.indices.len()).map(|i| other.tag(i)));
    }

    pub fn transformed(&self, translate: Vec3, rotate_deg: Vec3) -> Mesh {
        let positions = self.positions.iter().map(|p| p.rotate_xyz_deg(rotate_deg) + translate).collect();
        Mesh { positions, indices: self.indices.clone(), tags: self.tags.clone() }
    }

    /// Componentwise scale about the mesh's own origin. A factor of zero or
    /// less on any axis would collapse or invert the solid, so callers clamp
    /// before they get here; this does the arithmetic and nothing else.
    pub fn scaled(&self, factor: Vec3) -> Mesh {
        let positions = self.positions.iter().map(|p| p.scaled_by(factor)).collect();
        Mesh { positions, indices: self.indices.clone(), tags: self.tags.clone() }
    }

    pub fn translated(&self, offset: Vec3) -> Mesh {
        let positions = self.positions.iter().map(|p| *p + offset).collect();
        Mesh { positions, indices: self.indices.clone(), tags: self.tags.clone() }
    }

    pub fn triangle_normal(&self, tri: [u32; 3]) -> Vec3 {
        let a = self.positions[tri[0] as usize];
        let b = self.positions[tri[1] as usize];
        let c = self.positions[tri[2] as usize];
        (b - a).cross(c - a).normalized()
    }

    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut it = self.positions.iter();
        let first = *it.next()?;
        let mut lo = first;
        let mut hi = first;
        for p in it {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        Some((lo, hi))
    }

    /// Translate so the anchor sits at the origin: for `Base`, the mesh's
    /// minimum Z moves to Z=0 while X/Y stay centred on the shape's own centre.
    /// Shapes are generated centred on the origin already, so `Centre` is a no-op.
    pub fn apply_base_anchor(&mut self) {
        if let Some((lo, _hi)) = self.bounds() {
            let offset = Vec3::new(0.0, 0.0, -lo.z);
            for p in self.positions.iter_mut() {
                *p = *p + offset;
            }
        }
    }

    pub fn flip_winding(&mut self) {
        for t in self.indices.iter_mut() {
            t.swap(1, 2);
        }
    }

    /// Merge vertices that sit within ~1e-6mm of each other. Primitive
    /// generators push each triangle's own fresh vertices rather than
    /// sharing indices between adjacent triangles (simpler to write, and
    /// harmless for boolean evaluation and STL/OBJ export, which only care
    /// about positions) -- welding recovers a topologically connected mesh
    /// for the manifold check and for smaller PLY/3MF/OBJ output.
    pub fn weld(&self) -> Mesh {
        use std::collections::HashMap;
        let key = |p: Vec3| -> (i64, i64, i64) {
            let s = 1_000_000.0; // 1e-6 mm buckets
            ((p.x * s).round() as i64, (p.y * s).round() as i64, (p.z * s).round() as i64)
        };
        let mut map: HashMap<(i64, i64, i64), u32> = HashMap::new();
        let mut positions = Vec::new();
        let mut remap = vec![0u32; self.positions.len()];
        for (i, p) in self.positions.iter().enumerate() {
            let k = key(*p);
            let id = *map.entry(k).or_insert_with(|| {
                positions.push(*p);
                (positions.len() - 1) as u32
            });
            remap[i] = id;
        }
        let mut indices = Vec::with_capacity(self.indices.len());
        let mut tags = Vec::with_capacity(self.indices.len());
        for (i, t) in self.indices.iter().enumerate() {
            let t = [remap[t[0] as usize], remap[t[1] as usize], remap[t[2] as usize]];
            if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
                indices.push(t);
                tags.push(self.tag(i));
            }
        }
        Mesh { positions, indices, tags }
    }

    /// Manifoldness check on the welded mesh: every undirected edge must be
    /// shared by exactly two triangles, used with opposite winding by those
    /// two (each directed edge appears exactly once). Returns a description
    /// of the first problem found, if any.
    pub fn manifold_issue(&self) -> Option<String> {
        use std::collections::HashMap;
        let welded = self.weld();
        let mut directed: HashMap<(u32, u32), u32> = HashMap::new();
        for tri in &welded.indices {
            for i in 0..3 {
                let a = tri[i];
                let b = tri[(i + 1) % 3];
                *directed.entry((a, b)).or_insert(0) += 1;
            }
        }
        for (&(a, b), &count) in directed.iter() {
            if count != 1 {
                return Some(format!("edge ({a},{b}) used {count} times, expected 1"));
            }
            if !directed.contains_key(&(b, a)) {
                return Some(format!("edge ({a},{b}) has no opposite twin ({b},{a})"));
            }
        }
        None
    }
}
