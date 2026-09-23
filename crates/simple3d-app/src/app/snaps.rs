//! The features of a body a drag can snap onto, kept between frames.

use super::*;
use crate::gizmo::{self};
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

/// Everything one body offers a snap or a measurement: the notable points on it,
/// and the lines the principal planes leave across its surface -- the marks the
/// renderer draws on the solid, which are as catchable as any other line in the
/// picture.
#[derive(Default)]
pub struct BodySnaps {
    pub features: Vec<crate::snap::Feature>,
    pub marks: Vec<(Vec3, Vec3)>,
}

/// One body's snap targets, shared out of the cache without copying them.
pub(crate) type Snaps = std::rc::Rc<BodySnaps>;

/// What the cache holds per node: which mesh the targets were found on --
/// identified by the address of its `Arc`, which changes on re-evaluation and
/// nowhere else -- together with what the settings were showing, since the axis
/// crossings (issue 78) and the plane marks are part of the answer, and the
/// targets themselves.
pub(crate) type CachedSnaps = ((usize, u8), Snaps);

/// Everything `mesh` offers a snap: its own features, where the shown world
/// axes run through it (issue 78), and the lines the principal planes leave
/// across it, which are drawn on the surface and so can be caught along
/// their length.
pub(crate) fn find_snaps(mesh: &simple3d_geom::Mesh, axes: [bool; 3], marked: bool) -> BodySnaps {
    let mut features = crate::snap::features_of(mesh);
    features.extend(crate::snap::axis_features(mesh, axes));
    let marks = if marked { crate::snap::plane_mark_lines(mesh, axes) } else { Vec::new() };
    BodySnaps { features, marks }
}

/// Snap targets being found off the interface thread for the meshes an
/// evaluation has just brought, so the first frame that snaps finds them
/// ready -- see `App::warm_snaps`.
pub(crate) struct SnapWarming {
    found: std::sync::mpsc::Receiver<(NodeId, (usize, u8), BodySnaps)>,
    /// Set when the meshes it is working on have been replaced, so it stops.
    stale: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for SnapWarming {
    fn drop(&mut self) {
        self.stale.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The features of the body a drag is carrying, as far as they have been
/// found: where its origin stood and the meshes it had when the drag began,
/// and the offsets from the one to the features of the other once a frame of
/// the drag has snapped. See `App::drag_snap_sources`.
pub(crate) struct SnapSources {
    origin: Vec3,
    meshes: Vec<(NodeId, std::sync::Arc<simple3d_geom::Mesh>)>,
    offsets: Option<Vec<Vec3>>,
}

impl App {
    /// Start finding the snap targets of every shown body whose mesh the
    /// cache does not hold, on a thread of their own. Called when an
    /// evaluation lands.
    ///
    /// Snapping asks for the targets of every body on screen on its first
    /// frame, and finding them is a weld and a hash map over every edge of
    /// each: on a large scene that first frame hung for as long as all of
    /// them took. An evaluation replaces only the meshes that changed, and
    /// those are what is found here, while nobody is waiting on them.
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
        // Dropping the one before tells it to stop.
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

    /// Take in whatever the thread `warm_snaps` started has found so far.
    /// A body's targets are only kept while the mesh they were found on is
    /// still the one it has, which the key's address says.
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

    /// The dragged node and everything under it: the bodies a drag is carrying,
    /// which geometry snapping must never snap to.
    pub(super) fn drag_subtree(&self, id: NodeId) -> Vec<NodeId> {
        std::iter::once(id).chain(self.scene.descendants(id)).collect()
    }

    /// Every feature of the body a drag is carrying, as an offset from that
    /// node's origin. Taken once, when the handle is grabbed.
    ///
    /// Which feature should meet the target used to be decided here too, by
    /// looking for one within the catch radius of the cursor. A drag always
    /// starts on a manipulator handle, and those sit a fixed 78 screen pixels out
    /// along an axis -- never on the body's own geometry except by coincidence --
    /// so the answer was almost always "none", and it was the node's *origin*
    /// that landed on the target. Two boxes snapped together interpenetrated by
    /// half, which is not what "snap this corner to that corner" means.
    ///
    /// They are kept as *offsets*, and gathered before the body has moved,
    /// because `Evaluated` lags a drag: the meshes still describe where the body
    /// was at the last evaluation while `Node::position` is already live. An
    /// offset from the origin is the same either way, being a fact about the
    /// shape rather than about where it currently sits.
    ///
    /// What is taken on `Begin` is only the origin and the meshes, though:
    /// finding the features of a large curved body is a weld and a hash map
    /// over every edge -- 64 ms for a 160k-triangle sphere in a release build
    /// -- and a drag that never snaps would pay it as a hitch the moment it
    /// started. They are found from those, on the first frame that snaps.
    pub(super) fn drag_snap_sources(&self, id: NodeId) -> Option<SnapSources> {
        let frame = self.evaluated.node_frames.get(&id)?;
        let meshes = self.drag_subtree(id).into_iter();
        Some(SnapSources {
            origin: frame.point(self.scene.node(id).position),
            meshes: meshes.filter_map(|n| Some((n, self.evaluated.node_meshes.get(&n)?.clone()))).collect(),
            offsets: None,
        })
    }

    /// The carried body's features as offsets from its origin, found now if
    /// this is the first frame of the drag that asks.
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

    /// Snap a resize so the face being pulled lands on the nearest feature of
    /// another body under the pointer (issue 68). Returns the world point it
    /// caught, or `None` when nothing was in reach, in which case the grid
    /// resize stands.
    ///
    /// A resize is a drag, and it snapped only to the grid step. Pulling a plate
    /// out until it meets the block beside it is the same gesture as sliding it
    /// there, and it wants the same answer. Only a *face* handle: a corner moves
    /// three faces at once, and there is no one face to put on a point.
    pub(super) fn apply_resize_snap(
        &mut self,
        id: NodeId,
        view: &crate::view::View,
        cursor: egui::Pos2,
        mods: gizmo::Mods,
    ) -> Option<(Vec3, f64)> {
        let exclude = self.drag_subtree(id);
        let (target, _) = self.nearest_feature_excluding(view, cursor, &exclude)?;
        // Taken out and put back so the drag can write the scene the app owns.
        let mut drag = self.drag.take()?;
        let applied = drag.resize_face_to(&mut self.scene, target.point, mods.symmetric);
        self.drag = Some(drag);
        applied.map(|extent| (target.point, extent))
    }
}
