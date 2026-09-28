//! The features of a body a drag can snap onto, cached between frames.

use super::*;
use crate::gizmo::{self};
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

/// Everything a body offers for snapping or measuring: notable points and the principal plane
/// marks across its surface.
#[derive(Default)]
pub struct BodySnaps {
    pub features: Vec<crate::snap::Feature>,
    pub marks: Vec<(Vec3, Vec3)>,
    /// Each flat face's outline, indexed by a face centre's [`crate::snap::Feature::face`].
    pub faces: Vec<Vec<(Vec3, Vec3)>>,
}

/// One body's snap targets, shared out of the cache without copying.
pub(crate) type Snaps = std::rc::Rc<BodySnaps>;

/// Per node: the mesh's `Arc` address and the settings key (axis crossings, issue 78, and plane
/// marks), with the targets.
pub(crate) type CachedSnaps = ((usize, u8), Snaps);

/// Everything `mesh` offers a snap: its features, shown world axis crossings (issue 78), and
/// principal plane marks.
pub(crate) fn find_snaps(mesh: &simple3d_geom::Mesh, axes: [bool; 3], marked: bool) -> BodySnaps {
    let (mut features, faces) = crate::snap::features_and_faces(mesh);
    features.extend(crate::snap::axis_features(mesh, axes));
    let marks = if marked { crate::snap::plane_mark_lines(mesh, axes) } else { Vec::new() };
    BodySnaps { features, marks, faces }
}

/// Snap targets being found off-thread for newly evaluated meshes (`App::warm_snaps`).
pub(crate) struct SnapWarming {
    found: std::sync::mpsc::Receiver<(NodeId, (usize, u8), BodySnaps)>,
    /// Set when its meshes have been replaced, so it stops.
    stale: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for SnapWarming {
    fn drop(&mut self) {
        self.stale.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The carried body's features: its origin and meshes at drag start, and their offsets once the
/// first snapping frame computes them (`App::drag_snap_sources`).
pub(crate) struct SnapSources {
    origin: Vec3,
    meshes: Vec<(NodeId, std::sync::Arc<simple3d_geom::Mesh>)>,
    offsets: Option<Vec<Vec3>>,
}

impl App {
    /// Start finding snap targets for shown bodies missing from the cache on a thread, when an
    /// evaluation lands, so the first snapping frame does not hang.
    pub(crate) fn warm_snaps(&mut self) {
        let (axes, marked, mask) = self.snap_settings();
        let cached = self.snap_features.borrow();
        let wanted: Vec<(NodeId, std::sync::Arc<simple3d_geom::Mesh>)> = self
            .evaluated
            .node_meshes
            .iter()
            .filter(|&(&id, mesh)| {
                let key = (std::sync::Arc::as_ptr(mesh) as usize, mask);
                self.scene.is_shown(id) && cached.get(&id).is_none_or(|(held, _)| *held != key)
            })
            .map(|(&id, mesh)| (id, mesh.clone()))
            .collect();
        drop(cached);
        // Dropping the previous one tells it to stop.
        self.snap_warming = None;
        if wanted.is_empty() {
            return;
        }
        let (send, found) = std::sync::mpsc::channel();
        let stale = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop = stale.clone();
        let spawned = std::thread::Builder::new().name("snap-targets".into()).spawn(move || {
            for (id, mesh) in wanted {
                if stop.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                let key = (std::sync::Arc::as_ptr(&mesh) as usize, mask);
                if send.send((id, key, find_snaps(&mesh, axes, marked))).is_err() {
                    return;
                }
            }
        });
        if spawned.is_ok() {
            self.snap_warming = Some(SnapWarming { found, stale });
        }
    }

    /// Take in what `warm_snaps` has found, keeping only targets whose mesh is still current.
    pub(crate) fn poll_snap_warming(&mut self) {
        let Some(warming) = &self.snap_warming else { return };
        let mut cache = self.snap_features.borrow_mut();
        loop {
            match warming.found.try_recv() {
                Ok((id, key, snaps)) => {
                    let current = self.evaluated.node_meshes.get(&id).map(|mesh| std::sync::Arc::as_ptr(mesh) as usize);
                    if current == Some(key.0) {
                        cache.insert(id, (key, std::rc::Rc::new(snaps)));
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }
        drop(cache);
        self.snap_warming = None;
    }

    /// The dragged node and everything under it, which snapping must ignore.
    pub(super) fn drag_subtree(&self, id: NodeId) -> Vec<NodeId> {
        std::iter::once(id).chain(self.scene.descendants(id)).collect()
    }

    /// The carried body's snap sources, taken when the handle is grabbed.
    ///
    /// Features are kept as offsets from the origin, since `Evaluated` lags the drag while
    /// `Node::position` is live. Only the origin and meshes are taken now; finding features can take
    /// tens of milliseconds, so it waits for the first frame that snaps.
    pub(super) fn drag_snap_sources(&self, id: NodeId) -> Option<SnapSources> {
        let frame = self.evaluated.node_frames.get(&id)?;
        let meshes = self.drag_subtree(id).into_iter();
        Some(SnapSources {
            origin: frame.point(self.scene.node(id).position),
            meshes: meshes.filter_map(|n| Some((n, self.evaluated.node_meshes.get(&n)?.clone()))).collect(),
            offsets: None,
        })
    }

    /// The carried body's feature offsets, computed on the first frame that asks.
    pub(super) fn drag_feature_offsets(&mut self) -> &[Vec3] {
        let Some(mut sources) = self.snap_sources.take() else { return &[] };
        if sources.offsets.is_none() {
            let mut offsets = Vec::new();
            for (n, mesh) in &sources.meshes {
                offsets.extend(self.snaps_of(*n, mesh).features.iter().map(|f| f.point - sources.origin));
            }
            sources.offsets = Some(offsets);
        }
        self.snap_sources.insert(sources).offsets.as_deref().unwrap_or_default()
    }

    /// Snap a resize so the pulled face lands on the nearest feature under the pointer (issue 68).
    /// Returns what was caught, or `None` to keep the grid resize. Face handles only, since a corner
    /// moves three faces.
    pub(super) fn apply_resize_snap(
        &mut self,
        id: NodeId,
        view: &crate::view::View,
        cursor: egui::Pos2,
        mods: gizmo::Mods,
    ) -> Option<(crate::snap::SnapMark, f64)> {
        let exclude = self.drag_subtree(id);
        let (target, _, node) = self.nearest_feature_excluding(view, cursor, &exclude)?;
        // Taken out and put back so the drag can write the scene the app owns.
        let mut drag = self.drag.take()?;
        let applied = drag.resize_face_to(&mut self.scene, target.point, mods.symmetric);
        self.drag = Some(drag);
        let extent = applied?;
        Some((self.snap_mark(node, &target, &[]), extent))
    }
}
