use crate::vec3::Vec3;

/// An indexed triangle mesh; `indices` holds triangles into `positions`. Generated and boolean
/// results are expected to be watertight, manifold and wound CCW from outside.
///
/// `tags` holds one number per triangle naming the body it came from. This crate only carries it
/// through booleans, welding and healing, so callers can still colour faces by their operand.
///
/// `sources` is a second such channel naming the scene node a triangle came from (issue 73), so a
/// picked face of a boolean's result can be traced to its object. Empty or short means 0, unknown.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    pub indices: Vec<[u32; 3]>,
    pub tags: Vec<u32>,
    pub sources: Vec<u32>,
}

/// The tag for a surface painted `rgb`. The top byte separates painted tags from 0, which means
/// unpainted; defined here so the scene and the exporter agree.
pub fn colour_tag(rgb: [u8; 3]) -> u32 {
    0x0100_0000 | (u32::from(rgb[0]) << 16) | (u32::from(rgb[1]) << 8) | u32::from(rgb[2])
}

/// The colour a tag stands for, or `None` for an unpainted surface.
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
        self.push_face(a, b, c, tag, 0);
    }

    /// A triangle with both its tag and its source.
    pub fn push_face(&mut self, a: Vec3, b: Vec3, c: Vec3, tag: u32, source: u32) {
        self.pad_sources();
        self.sources.push(source);
        let base = self.positions.len() as u32;
        self.positions.push(a);
        self.positions.push(b);
        self.positions.push(c);
        self.indices.push([base, base + 1, base + 2]);
        self.tags.push(tag);
    }

    /// One triangle's tag; zero unless a caller set it.
    pub fn tag(&self, triangle: usize) -> u32 {
        self.tags.get(triangle).copied().unwrap_or(0)
    }

    /// One triangle's source node; zero unless a caller set it.
    pub fn source(&self, triangle: usize) -> u32 {
        self.sources.get(triangle).copied().unwrap_or(0)
    }

    /// Mark every triangle as coming from one node.
    pub fn set_source(&mut self, source: u32) {
        self.sources.clear();
        self.sources.resize(self.indices.len(), source);
    }

    /// Fill missing sources with 0, so a push or an append lines up with its triangle.
    fn pad_sources(&mut self) {
        let n = self.indices.len();
        if self.sources.len() != n {
            self.sources.resize(n, 0);
        }
    }

    /// Mark every triangle as one body, once per primitive before combining.
    pub fn set_tag(&mut self, tag: u32) {
        self.tags.clear();
        self.tags.resize(self.indices.len(), tag);
    }

    /// Append another mesh's geometry, offsetting its indices.
    pub fn append(&mut self, other: &Mesh) {
        if !(self.sources.is_empty() && other.sources.is_empty()) {
            self.pad_sources();
            self.sources.extend((0..other.indices.len()).map(|i| other.source(i)));
        }
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.indices.extend(other.indices.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
        self.tags.extend((0..other.indices.len()).map(|i| other.tag(i)));
    }

    pub fn transformed(&self, translate: Vec3, rotate_deg: Vec3) -> Mesh {
        let positions = self.positions.iter().map(|p| p.rotate_xyz_deg(rotate_deg) + translate).collect();
        Mesh { positions, indices: self.indices.clone(), tags: self.tags.clone(), sources: self.sources.clone() }
    }

    /// Componentwise scale about the mesh's origin. Callers clamp non-positive factors first.
    pub fn scaled(&self, factor: Vec3) -> Mesh {
        let positions = self.positions.iter().map(|p| p.scaled_by(factor)).collect();
        Mesh { positions, indices: self.indices.clone(), tags: self.tags.clone(), sources: self.sources.clone() }
    }

    pub fn translated(&self, offset: Vec3) -> Mesh {
        let positions = self.positions.iter().map(|p| *p + offset).collect();
        Mesh { positions, indices: self.indices.clone(), tags: self.tags.clone(), sources: self.sources.clone() }
    }

    /// The three corner positions of triangle `tri`.
    pub fn corners(&self, tri: [u32; 3]) -> [Vec3; 3] {
        [self.positions[tri[0] as usize], self.positions[tri[1] as usize], self.positions[tri[2] as usize]]
    }

    /// The enclosed volume by the divergence theorem, positive for outward winding on a closed mesh.
    pub fn signed_volume(&self) -> f64 {
        let mut total = 0.0;
        for tri in &self.indices {
            let [a, b, c] = self.corners(*tri);
            total += a.dot(b.cross(c));
        }
        total / 6.0
    }

    pub fn triangle_normal(&self, tri: [u32; 3]) -> Vec3 {
        let [a, b, c] = self.corners(tri);
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

    /// Move a `Base` anchor to the origin: minimum Z to Z=0, X and Y centred. Shapes are generated
    /// centred, so `Centre` is a no-op.
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

    /// Merge vertices within ~1e-6 mm. Generators emit unshared vertices per triangle; welding
    /// recovers a connected mesh for the manifold check and smaller output.
    pub fn weld(&self) -> Mesh {
        self.weld_with_remap().0
    }

    /// The same, with each original vertex's welded index; welded vertices are numbered by first
    /// appearance.
    pub fn weld_with_remap(&self) -> (Mesh, Vec<u32>) {
        let key = |p: Vec3| -> (i64, i64, i64) {
            let s = 1_000_000.0; // 1e-6 mm buckets
            ((p.x * s).round() as i64, (p.y * s).round() as i64, (p.z * s).round() as i64)
        };
        // A cheap multiplicative hash rather than SipHash: keys are not adversarial, and hashing
        // dominated welding on large meshes.
        let mut map: FastMap<(i64, i64, i64), u32> = FastMap::default();
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
        let mut sources = Vec::new();
        for (i, t) in self.indices.iter().enumerate() {
            let t = [remap[t[0] as usize], remap[t[1] as usize], remap[t[2] as usize]];
            if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
                indices.push(t);
                tags.push(self.tag(i));
                if !self.sources.is_empty() {
                    sources.push(self.source(i));
                }
            }
        }
        (Mesh { positions, indices, tags, sources }, remap)
    }

    /// Closedness check on the welded mesh: every directed edge must be balanced by as many reverse
    /// edges. Returns the first problem, if any. Balanced rather than exactly one each way, since
    /// touching bodies (split cells) legitimately share edges.
    pub fn manifold_issue(&self) -> Option<String> {
        let welded = self.weld();
        let mut directed: FastMap<(u32, u32), u32> = FastMap::default();
        for tri in &welded.indices {
            for i in 0..3 {
                let a = tri[i];
                let b = tri[(i + 1) % 3];
                *directed.entry((a, b)).or_insert(0) += 1;
            }
        }
        for (&(a, b), &count) in directed.iter() {
            let back = directed.get(&(b, a)).copied().unwrap_or(0);
            if back != count {
                return Some(format!("edge ({a},{b}) used {count} times and ({b},{a}) {back}"));
            }
        }
        None
    }
}

/// A map with integer keys using [`CoordHasher`] instead of SipHash, which was a third of the
/// repair passes' time.
pub(crate) type FastMap<K, V> = std::collections::HashMap<K, V, std::hash::BuildHasherDefault<CoordHasher>>;

/// An FxHash-style hasher for integer keys (see [`Mesh::weld`]).
#[derive(Default)]
pub(crate) struct CoordHasher(u64);

impl std::hash::Hasher for CoordHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(byte as u64);
        }
    }

    fn write_u64(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }

    fn write_i64(&mut self, word: i64) {
        self.write_u64(word as u64);
    }

    fn write_u32(&mut self, word: u32) {
        self.write_u64(word as u64);
    }

    fn write_usize(&mut self, word: usize) {
        self.write_u64(word as u64);
    }
}
