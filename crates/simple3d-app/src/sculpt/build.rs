//! Making the working copy, and writing it back.

use super::*;
use simple3d_geom::topology::vertex_normals;
use simple3d_geom::Mesh;
use std::collections::BTreeMap;

impl Sculpt {
    /// The copy of `base`, placed in the world by `placement` and drawn in `colour` when the node has
    /// one, with its topology, normals and picking tree.
    pub fn new(target: NodeId, base: Arc<MeshData>, placement: Xform, colour: Option<u32>) -> Sculpt {
        let local = &base.mesh;
        let count = local.indices.len();
        let tags: Vec<u32> = (0..count).map(|index| local.tag(index)).collect();
        let mut world = Mesh {
            positions: local.positions.iter().map(|&p| placement.point(p)).collect(),
            indices: local.indices.clone(),
            tags: colour.map_or_else(|| tags.clone(), |tag| vec![tag; count]),
            sources: Vec::new(),
        };
        // As the evaluation places it: a mirroring placement would turn the faces inside out.
        if placement.mirrors() {
            world.flip_winding();
        }
        let smooth = vec![true; count];
        let (topology, normals, tree) = std::thread::scope(|scope| {
            let topology = scope.spawn(|| Topology::of(&world));
            let normals = scope.spawn(|| vertex_normals(&world));
            let tree = Bvh::build(&world);
            (topology.join().expect("the topology panicked"), normals.join().expect("the normals panicked"), tree)
        });
        let refit = tree.refit_map();
        let renderable = Renderable::prepare_welded(world, true, &smooth, BTreeMap::new());
        Sculpt {
            target,
            base,
            placement,
            colour,
            tags,
            renderable,
            topology,
            normals,
            smooth,
            tree,
            refit,
            visited: Vec::new(),
            visit: 0,
            edits: Edits::default(),
            stroke: None,
            lagging: false,
            settling: false,
        }
    }

    /// The copy back in the mesh's own coordinates, as the node will store it. Vertices the session
    /// never moved keep their stored positions exactly, rather than going through the placement and
    /// back.
    pub fn result(&self) -> MeshData {
        let inverse = self.placement.inverse();
        let world = &self.renderable.mesh;
        let stored = &self.base.mesh.positions;
        let positions = match stored.len() == world.positions.len() {
            true => stored
                .iter()
                .zip(&world.positions)
                .map(|(&local, &at)| if self.placement.point(local) == at { local } else { inverse.point(at) })
                .collect(),
            false => world.positions.iter().map(|&at| inverse.point(at)).collect(),
        };
        let mut mesh = Mesh { positions, indices: world.indices.clone(), tags: self.tags.clone(), sources: Vec::new() };
        if self.placement.mirrors() {
            mesh.flip_winding();
        }
        MeshData::new(mesh)
    }
}
