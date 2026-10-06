//! Rounding and bevelling edges and corners (issue 88), in an in-place popup.
//!
//! While the window is open the viewport's clicks pick edges and corners of the model, and a drag
//! away from an edge sizes the treatment. The treatment is kept on the object the faces belong to,
//! as push/pull's edits are ([`simple3d_core::scene::RoundEdit`]), so the object stays one shape whose
//! rounding can be resized, dropped or taken out later from its properties.
//!
//! The picks stand in the document as a draft while the window is open, so the model shows the
//! result itself, with the material that goes or comes drawn red or green over it. The draft is no
//! edit: it is lifted out of every undo snapshot and save (`App::lift_preview`), Apply makes it one
//! step, and Cancel takes it away.

mod apply;
mod draft;
pub(crate) use draft::Lifted;
mod drag;
mod draw;
pub(crate) use draw::draw;
mod ghost;
mod join;
pub(crate) use ghost::templates;
pub(crate) mod pick;
pub(crate) use pick::interact;
mod window;
pub(crate) use window::show;

use crate::app::{App, Status};
use simple3d_core::scene::{NodeId, ObjectEdit};
use simple3d_geom::rounding::{Corner, FeatureEdge, Treatment};
use simple3d_geom::Mesh;
use std::rc::Rc;
use std::sync::Arc;

pub use simple3d_core::scene::RoundKind as Kind;

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "round-tool";

/// The window width: a label and its field.
const WIDTH: f32 = 300.0;

/// The tool while its window is open.
pub struct RoundTool {
    /// The picked edges and corners, in world space, in the order picked.
    pub edges: Vec<FeatureEdge>,
    pub corners: Vec<Corner>,
    /// The inside corners picked, where two picked edges meeting are mitred into one seam.
    pub joints: Vec<simple3d_geom::Vec3>,
    pub kind: Kind,
    pub radius: f64,
    pub segments: u32,
    pub distance: f64,
    /// Whether a corner whose every edge is picked is blended too, without being picked itself.
    pub blend_corners: bool,
    /// Whether an edge running straight on from one object into the next is picked as one; otherwise
    /// each object's stretch of it is an edge of its own.
    pub merge_edges: bool,
    /// Whether each picked edge is treated on along its line to where the body ends, so one an earlier
    /// treatment shortened at a corner is treated as if whole.
    pub extend_edges: bool,
    /// The model without the draft, which the edges and corners are found and picked on.
    base: Arc<Mesh>,
    /// The model's sharp edges and corners, for the mesh (by address) they were found on and whether
    /// edges were merged across objects.
    features: std::cell::RefCell<Option<(FeaturesKey, Rc<Features>)>>,
    /// How much room each picked edge has (`simple3d_geom::rounding::edge_room`), for the mesh (by
    /// address) it was measured on.
    rooms: std::cell::RefCell<(usize, Vec<(FeatureEdge, f64)>)>,
    /// The edits standing in the document as the draft, on the nodes holding them.
    pub(crate) draft: Vec<(NodeId, ObjectEdit)>,
    /// The roundings the draft took off their nodes to join at a picked corner, with where they were.
    pub(crate) taken: Vec<(NodeId, usize, ObjectEdit)>,
    /// What the draft and the ghost were made for, so they are remade only when that changes.
    made_for: Option<u64>,
    /// An edge being dragged to size the treatment.
    pub(crate) drag: Option<drag::SizeDrag>,
    /// The material the draft takes away and adds, drawn in the picture.
    pub(crate) ghost: ghost::Ghost,
}

/// The mesh (by address) features were found on, and whether edges were merged across objects.
type FeaturesKey = (usize, bool);

/// What can be picked on the model.
pub(crate) struct Features {
    pub(crate) edges: Vec<FeatureEdge>,
    pub(crate) corners: Vec<Corner>,
    /// Inside corners, where two outside edges meet and the solid turns inward.
    pub(crate) joints: Vec<simple3d_geom::Vec3>,
    /// The edges again, with each object's stretch apart, to tell which objects a merged edge runs over.
    pub(crate) pieces: Vec<FeatureEdge>,
}

impl RoundTool {
    fn new(base: Arc<Mesh>) -> RoundTool {
        RoundTool {
            edges: Vec::new(),
            corners: Vec::new(),
            joints: Vec::new(),
            kind: Kind::Round,
            radius: 2.0,
            segments: 8,
            distance: 1.0,
            // Corners are treated when picked, not of themselves.
            blend_corners: false,
            merge_edges: true,
            extend_edges: false,
            base,
            features: std::cell::RefCell::new(None),
            rooms: std::cell::RefCell::new((0, Vec::new())),
            draft: Vec::new(),
            taken: Vec::new(),
            made_for: None,
            drag: None,
            ghost: ghost::Ghost::default(),
        }
    }

    /// The treatment the window's numbers describe.
    pub fn treatment(&self) -> Treatment {
        match self.kind {
            Kind::Round => Treatment::Round { radius: self.radius.max(0.01), segments: self.segments.max(1) },
            Kind::Chamfer => Treatment::Chamfer { distance: self.distance.max(0.01) },
        }
    }

    /// The treatment's one number, whichever it is.
    pub fn size(&self) -> f64 {
        match self.kind {
            Kind::Round => self.radius,
            Kind::Chamfer => self.distance,
        }
    }

    pub(crate) fn set_size(&mut self, size: f64) {
        match self.kind {
            Kind::Round => self.radius = size.max(0.01),
            Kind::Chamfer => self.distance = size.max(0.01),
        }
    }

    /// Whether `edge` has room for the treatment: one reaching past the end of a face is left out.
    pub(crate) fn fits(&self, edge: &FeatureEdge) -> bool {
        let key = Arc::as_ptr(&self.base) as usize;
        let mut rooms = self.rooms.borrow_mut();
        if rooms.0 != key {
            *rooms = (key, Vec::new());
        }
        let room = match rooms.1.iter().find(|(e, _)| pick::same_edge(e, edge)) {
            Some(&(_, room)) => room,
            None => {
                let room = simple3d_geom::rounding::edge_room(&self.base, edge);
                rooms.1.push((*edge, room));
                room
            }
        };
        simple3d_geom::rounding::edge_fits(edge, self.treatment(), room)
    }

    /// How many picked edges and corners are too small for the treatment.
    pub(crate) fn too_small(&self) -> usize {
        let edges = self.edges.iter().filter(|edge| !self.fits(edge)).count();
        let treatment = self.treatment();
        edges + self.corners.iter().filter(|c| !simple3d_geom::rounding::corner_fits(c, treatment)).count()
    }

    pub fn is_empty(&self) -> bool {
        self.edges.is_empty() && self.corners.is_empty() && self.joints.is_empty()
    }
}

impl App {
    /// Open the tool, or put it away when it is already out, taking its draft back.
    pub fn open_round_tool(&mut self) {
        if self.round_tool.is_some() {
            self.cancel_round_tool();
            self.status = Status::Info("Round and bevel closed".into());
            return;
        }
        // One tool holds the viewport's clicks at a time.
        if self.measure.active {
            self.toggle_measure();
        }
        self.round_tool = Some(RoundTool::new(self.evaluated.mesh.clone()));
        self.status = Status::Info(
            "Round and bevel: click the edges and corners to treat in the viewport, drag one to size it".into(),
        );
    }

    /// The model's sharp edges and corners, found once per model without the draft.
    pub(crate) fn round_features(&self) -> Option<Rc<Features>> {
        let tool = self.round_tool.as_ref()?;
        let key = (Arc::as_ptr(&tool.base) as usize, tool.merge_edges);
        if let Some((at, features)) = tool.features.borrow().as_ref() {
            if *at == key {
                return Some(features.clone());
            }
        }
        let pieces = simple3d_geom::rounding::feature_edges_by(&tool.base, false);
        let edges = if tool.merge_edges { simple3d_geom::rounding::feature_edges(&tool.base) } else { pieces.clone() };
        let corners = Corner::find(&edges);
        let mut joints = simple3d_geom::rounding::inside_corners(&edges);
        for at in self.round_join_corners(&tool.base, tool.treatment()) {
            if !joints.iter().any(|&j| pick::near(j, at)) {
                joints.push(at);
            }
        }
        let features = Rc::new(Features { edges, corners, joints, pieces });
        *tool.features.borrow_mut() = Some((key, features.clone()));
        Some(features)
    }
}
