//! The node tree (spec section 3): nodes, groups, the scene and every
//! structural edit the outliner offers.
//!
//! The tree is an arena of nodes keyed by a stable id that survives save/load,
//! reparenting and undo. `BTreeMap` rather than `HashMap` so iteration order is
//! deterministic, which matters because evaluation must be (section 5.2).

use crate::mesh_data::{MeshBlob, MeshData};
use crate::primitive::{self, Params, PrimitiveSpec};
use crate::unit::Unit;
use serde::{Deserialize, Serialize};
use simple3d_geom::tiling::Tiling;
use simple3d_geom::{BooleanOp, Vec3};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

pub type NodeId = u64;

/// What a copy of `name` is called before it is numbered: `Box` -> `Box copy`,
/// and `Box copy` -> `Box copy` again rather than `Box copy copy`.
///
/// Duplicating repeatedly is the ordinary way to lay out a row of something, and
/// each duplicate is of the one just made -- so without the trim the fourth
/// press of Ctrl+D gives "Box copy copy copy copy". Trimmed, the numbering rule
/// below takes over and gives "Box copy 2", "Box copy 3".
pub fn copy_name(name: &str) -> String {
    let stem = name.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end();
    let stem = stem.strip_suffix(" copy").unwrap_or(name);
    format!("{stem} copy")
}

/// `base`, or `base 2`, `base 3`... -- the first that is not in `taken`.
///
/// The one place the numbering rule lives, so a node added, pasted, duplicated
/// or dropped in from the library all read the same way in the outliner.
pub fn free_name(taken: &HashSet<String>, base: &str) -> String {
    if !taken.contains(base) {
        return base.to_string();
    }
    for n in 2.. {
        let candidate = format!("{base} {n}");
        if !taken.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

/// Where a node's origin sits (spec section 3.1). Changing it moves the origin,
/// never the shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    #[default]
    Centre,
    /// Minimum Z at the origin, for standing things on a build plate.
    Base,
}

impl Anchor {
    pub const ALL: [Anchor; 2] = [Anchor::Centre, Anchor::Base];

    pub fn label(self) -> &'static str {
        match self {
            Anchor::Centre => "Centre",
            Anchor::Base => "Base",
        }
    }
}

/// A node's colour: what it is painted, if anything. Stored as the three
/// sRGB bytes a colour picker produces, and written to the project file as a
/// `#rrggbb` string so a scene stays readable and diffable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colour(pub [u8; 3]);

impl Colour {
    /// The tag `simple3d-geom` carries through booleans for a painted body. The
    /// top byte separates "painted this colour" from tag 0, which every
    /// generator produces and which means "whatever the theme paints solids".
    pub const UNPAINTED: u32 = 0;

    pub fn tag(self) -> u32 {
        simple3d_geom::colour_tag(self.0)
    }

    /// The colour a tag stands for, or `None` for a surface nobody painted.
    pub fn from_tag(tag: u32) -> Option<Colour> {
        simple3d_geom::tag_colour(tag).map(Colour)
    }

    pub fn to_hex(self) -> String {
        let [r, g, b] = self.0;
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    /// Parse `#rrggbb` or `rrggbb`. Anything else is not a colour, and the
    /// loader treats it as an unpainted node rather than failing the file.
    pub fn from_hex(text: &str) -> Option<Colour> {
        let digits = text.strip_prefix('#').unwrap_or(text);
        if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
        Some(Colour([byte(0)?, byte(2)?, byte(4)?]))
    }
}

/// The tag for a node's effective colour, for `simple3d-geom` to carry.
pub fn colour_tag(colour: Option<Colour>) -> u32 {
    colour.map_or(Colour::UNPAINTED, Colour::tag)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupOp {
    #[default]
    Union,
    Difference,
    Intersection,
    Hull,
}

impl GroupOp {
    pub const ALL: [GroupOp; 4] = [GroupOp::Union, GroupOp::Difference, GroupOp::Intersection, GroupOp::Hull];

    pub fn label(self) -> &'static str {
        match self {
            GroupOp::Union => "Union",
            GroupOp::Difference => "Difference",
            GroupOp::Intersection => "Intersection",
            GroupOp::Hull => "Hull",
        }
    }

    /// Difference is the only operation where child order carries meaning.
    pub fn order_matters(self) -> bool {
        self == GroupOp::Difference
    }

    /// Whether the result still contains its operands as pieces that can be
    /// taken out of it. A union is its operands standing side by side; a
    /// difference, an intersection and a hull are one new surface, and a child
    /// of one of those is not a solid that exists in the result at all -- so it
    /// can never be a body of its own in an export.
    pub fn separable(self) -> bool {
        self == GroupOp::Union
    }

    pub fn to_geom(self) -> BooleanOp {
        match self {
            GroupOp::Union => BooleanOp::Union,
            GroupOp::Difference => BooleanOp::Difference,
            GroupOp::Intersection => BooleanOp::Intersection,
            GroupOp::Hull => BooleanOp::Hull,
        }
    }
}

/// What a node is in an export that lets the user choose its bodies (issue 58).
///
/// The absence of one -- `Node::export_body` being `None` -- means "a body of
/// its own", which is what every node is until it is told otherwise. An
/// untouched project therefore exports exactly as "top level bodies" does, and
/// a mark is only ever needed where the answer differs from that.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportBody {
    /// Merged, as one solid, with every other node carrying the same number.
    Shared(u32),
    /// Not a body itself: its children are each considered in its place, which
    /// is how an export reaches inside a group. Only a separable group can
    /// carry this -- see [`GroupOp::separable`].
    Split,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Group {
        op: GroupOp,
    },
    Primitive {
        type_id: String,
        params: Params,
    },
    /// A node that repeats its children under a rule (issue 67). It holds
    /// children like a group, and its parameters -- the kind of pattern and its
    /// numbers -- ride in the same `Params` map a primitive uses.
    Pattern {
        params: Params,
    },
    /// Geometry the node owns outright, with no recipe behind it -- what a
    /// shape becomes when it is converted to a mesh (issue 80).
    ///
    /// Behind an `Arc` because undo snapshots the whole scene, and a converted
    /// body of any size is megabytes: sharing the triangles between every
    /// snapshot that did not touch them is what keeps the history affordable.
    Mesh {
        mesh: Arc<MeshData>,
    },
    /// What breaking a shape apart leaves behind (issue 82): a node holding the
    /// separate pieces the shape was actually in, as children, and the object it
    /// was made from, so it can be put back together.
    ///
    /// It combines its children exactly as a union group does -- the pieces
    /// stand side by side, which is what they did inside the shape -- but it is
    /// a body of its own rather than a group, because a group is something the
    /// user assembles and this is something the application made *out of* one
    /// object. That is also why nothing offers to create one: it exists only
    /// where "break into separate objects" put it.
    ///
    /// `original` is the whole subtree the pieces came from, in the portable
    /// form the project file and the clipboard already use, behind an `Arc` for
    /// the same reason a stored mesh is: undo snapshots the scene, and the
    /// recipe must not be copied into every snapshot that never touched it.
    ///
    /// `tiling` is how the pieces were made: the cell shape and its numbers for
    /// a shape cut into a pattern of them, and `None` for one merely separated
    /// into the pieces it was already in. It is a label and an offer, not a
    /// recipe -- the pieces are geometry and nothing re-derives them from it --
    /// but it is what lets the panel say what was done and lets the tool open
    /// again on the numbers that did it.
    Split {
        original: Arc<NodeData>,
        tiling: Option<Tiling>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    /// Millimetres, in the parent's frame.
    pub position: Vec3,
    /// Degrees, applied X then Y then Z.
    pub rotation: Vec3,
    /// A factor per axis, applied in the node's own axes before its rotation.
    ///
    /// Distinct from a *resize*, which rewrites the dimension a shape is defined
    /// by and leaves nothing behind. A scale is a factor the node carries, it
    /// applies to a whole group as readily as to one shape, and it is the only
    /// way to make something a proportion of what it was without touching every
    /// number underneath. `1, 1, 1` is no scale at all.
    pub scale: Vec3,
    pub anchor: Anchor,
    /// Hidden nodes are excluded from evaluation and export entirely.
    pub visible: bool,
    /// What a hidden node looks like: nothing at all, or a translucent ghost.
    ///
    /// Meaningless while `visible` is true, and the pair is read through
    /// `Node::visibility` rather than field by field. It is per node because
    /// the two reasons to hide something are different reasons: a tool body
    /// about to be subtracted has to be seen while it is positioned, and
    /// everything else that is hidden has to be *gone*. One switch over the
    /// whole document could only ever answer one of them.
    pub ghost: bool,
    /// What this node is painted, if anything. A node without one takes its
    /// nearest painted ancestor's colour, which is what makes painting a group
    /// paint everything in it; nothing painted anywhere leaves the theme's own
    /// colour for solids.
    pub colour: Option<Colour>,
    /// Per-object override of the scene's default segment count.
    pub segments: Option<u32>,
    /// What this node is in an export whose bodies the user chooses. `None`,
    /// which is nearly always, means a body of its own.
    pub export_body: Option<ExportBody>,
    /// Whether this node has been lifted out of the collection holding it, so
    /// that it has a row of its own in the tree (issue 82).
    ///
    /// Meaningless everywhere but under a [`Body::Split`], which holds its
    /// pieces *inside* itself: a split is one row however many thousand pieces
    /// it is in, and the ones marked here are the few that have been asked for
    /// by name. Read through [`Scene::has_row`] rather than field by field,
    /// because "is this drawn in the tree" is a question about the node *and*
    /// its parent and answering half of it is how a piece ends up in two places
    /// at once.
    pub extracted: bool,
    pub body: Body,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
}

/// What a node shows in the viewport: the three states the interface offers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    #[default]
    Visible,
    /// Excluded from the model, drawn as a translucent shell -- for a body that
    /// is about to be subtracted and has to be positioned first.
    Ghost,
    /// Excluded from the model and not drawn at all.
    Hidden,
}

impl Visibility {
    pub const ALL: [Visibility; 3] = [Visibility::Visible, Visibility::Ghost, Visibility::Hidden];

    pub fn label(self) -> &'static str {
        match self {
            Visibility::Visible => "Visible",
            Visibility::Ghost => "Ghost",
            Visibility::Hidden => "Hidden",
        }
    }
}

impl Node {
    /// The smallest a scale factor may get. Zero collapses a solid into a plane
    /// and negative turns it inside out, and neither is a thing to export.
    pub const MIN_SCALE: f64 = 1e-4;

    /// The node's own visibility, as the one three-state answer the interface
    /// asks for rather than as the two flags that store it.
    pub fn visibility(&self) -> Visibility {
        match (self.visible, self.ghost) {
            (true, _) => Visibility::Visible,
            (false, true) => Visibility::Ghost,
            (false, false) => Visibility::Hidden,
        }
    }

    pub fn set_visibility(&mut self, visibility: Visibility) {
        self.visible = visibility == Visibility::Visible;
        self.ghost = visibility == Visibility::Ghost;
    }

    /// A scale with every axis clamped into the range that produces a solid.
    pub fn sane_scale(scale: Vec3) -> Vec3 {
        Vec3::new(scale.x.max(Node::MIN_SCALE), scale.y.max(Node::MIN_SCALE), scale.z.max(Node::MIN_SCALE))
    }

    pub fn is_group(&self) -> bool {
        matches!(self.body, Body::Group { .. })
    }

    pub fn is_pattern(&self) -> bool {
        matches!(self.body, Body::Pattern { .. })
    }

    pub fn is_mesh(&self) -> bool {
        matches!(self.body, Body::Mesh { .. })
    }

    /// Whether this node is a shape that was broken into its pieces (issue 82).
    pub fn is_split(&self) -> bool {
        matches!(self.body, Body::Split { .. })
    }

    /// The object a split was made from, for a node that is one.
    pub fn split_original(&self) -> Option<&Arc<NodeData>> {
        match &self.body {
            Body::Split { original, .. } => Some(original),
            _ => None,
        }
    }

    /// How a split's pieces were cut, for one cut into a pattern of cells.
    /// `None` for a node that is not a split, and for a split that was
    /// separated into the pieces it was already in rather than cut.
    pub fn split_tiling(&self) -> Option<Tiling> {
        match &self.body {
            Body::Split { tiling, .. } => *tiling,
            _ => None,
        }
    }

    /// The geometry this node owns, for a body that owns any.
    pub fn mesh(&self) -> Option<&Arc<MeshData>> {
        match &self.body {
            Body::Mesh { mesh } => Some(mesh),
            _ => None,
        }
    }

    /// What kind of thing this node is, in one word, for a status line or a
    /// tooltip. Not the primitive's own label -- "Box" -- but the family.
    pub fn kind_label(&self) -> &'static str {
        match &self.body {
            Body::Group { .. } => "group",
            Body::Primitive { .. } => "shape",
            Body::Pattern { .. } => "pattern",
            Body::Mesh { .. } => "mesh",
            Body::Split { .. } => "split",
        }
    }

    /// Whether this node can hold children: a group, a pattern or a split. A
    /// primitive and a mesh cannot, and a drag or an Add that would put a child
    /// under one is refused.
    pub fn can_hold_children(&self) -> bool {
        self.is_group() || self.is_pattern() || self.is_split()
    }

    /// The node's own boolean operation, which only a group has. A split
    /// combines its children too -- see [`Node::combine_op`] -- but it is not a
    /// group, and nothing that edits an operation may reach it.
    pub fn group_op(&self) -> Option<GroupOp> {
        match self.body {
            Body::Group { op } => Some(op),
            _ => None,
        }
    }

    /// How this node's children are combined, for the bodies that combine
    /// children at all: the group's own operation, and a union for a split,
    /// because pieces of one shape standing side by side is what a union is.
    pub fn combine_op(&self) -> Option<GroupOp> {
        match self.body {
            Body::Group { op } => Some(op),
            Body::Split { .. } => Some(GroupOp::Union),
            _ => None,
        }
    }

    pub fn spec(&self) -> Option<&'static PrimitiveSpec> {
        match &self.body {
            Body::Primitive { type_id, .. } => primitive::lookup(type_id),
            _ => None,
        }
    }

    pub fn params(&self) -> Option<&Params> {
        match &self.body {
            Body::Primitive { params, .. } | Body::Pattern { params } => Some(params),
            _ => None,
        }
    }

    pub fn params_mut(&mut self) -> Option<&mut Params> {
        match &mut self.body {
            Body::Primitive { params, .. } | Body::Pattern { params } => Some(params),
            _ => None,
        }
    }
}

/// How the origin axes are drawn. Two readings of the same three lines, kept
/// as a setting rather than a decision, because which one helps depends on
/// whether the axes are being used to place something or to read the ground.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AxisStyle {
    /// The way most 3D software draws them: X and Y *are* the coloured grid
    /// lines through zero. They run the width of the grid and travel with it as
    /// the view pans, so the ground always says which way is which.
    #[default]
    Grid,
    /// A fixed cross pinned at the origin, fading out at its own length. It
    /// says where the origin is rather than which way the ground runs, and it
    /// leaves the view once the origin is panned off screen.
    Origin,
}

impl AxisStyle {
    pub const ALL: [AxisStyle; 2] = [AxisStyle::Grid, AxisStyle::Origin];

    pub fn label(self) -> &'static str {
        match self {
            AxisStyle::Grid => "Along the grid",
            AxisStyle::Origin => "Pinned at the origin",
        }
    }
}

/// What the viewport does while a tool draws a preview over it (issue 82).
///
/// An in-place popup exists so that one rectangle can be the modelling area and
/// the preview area at once. That only works if the preview can be seen, and
/// what is in the way depends on what is being previewed: a tiling drawn flat
/// on a plate competes with the grid it lies parallel to, a cut through a tall
/// shape competes with the axes running through it, and a preview of one object
/// in a crowded scene competes with the scene. Which of those is the nuisance
/// is not something the application can know, so it is a document setting.
///
/// It applies only while a preview is actually being drawn, and puts everything
/// back the moment the tool closes: this is not a way to turn the grid off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewViewport {
    /// The viewport carries on as it is, and the preview is drawn over it.
    #[default]
    NoChange,
    /// Drop the origin axes while the preview is up.
    HideAxes,
    /// Drop the ground grid while the preview is up.
    HideGrid,
    /// Nothing but the object being previewed: every other body goes, and so do
    /// the grid and the axes. The strongest answer, for reading a fine pattern
    /// against one shape.
    PreviewOnly,
}

impl PreviewViewport {
    pub const ALL: [PreviewViewport; 4] =
        [PreviewViewport::NoChange, PreviewViewport::HideAxes, PreviewViewport::HideGrid, PreviewViewport::PreviewOnly];

    pub fn label(self) -> &'static str {
        match self {
            PreviewViewport::NoChange => "No change",
            PreviewViewport::HideAxes => "Hide the axes",
            PreviewViewport::HideGrid => "Hide the grid",
            PreviewViewport::PreviewOnly => "Only what is previewed",
        }
    }

    /// Whether the grid is drawn under a preview in this mode.
    pub fn keeps_grid(self) -> bool {
        matches!(self, PreviewViewport::NoChange | PreviewViewport::HideAxes)
    }

    /// Whether the origin axes are drawn under a preview in this mode.
    pub fn keeps_axes(self) -> bool {
        matches!(self, PreviewViewport::NoChange | PreviewViewport::HideGrid)
    }

    /// Whether everything but the previewed object is drawn.
    pub fn keeps_other_bodies(self) -> bool {
        self != PreviewViewport::PreviewOnly
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneSettings {
    pub unit: Unit,
    pub default_segments: u32,
    #[serde(default)]
    pub notes: String,
    pub grid_spacing: f64,
    pub grid_visible: bool,
    /// How far one step of a move or resize goes: the increment a drag snaps to
    /// and one press of a nudge key covers. Its own setting rather than the grid
    /// spacing, which is about what the ground looks like -- 1 mm is the step
    /// most people want and a 1 mm grid is unreadable.
    #[serde(default = "default_snap_step")]
    pub snap_step: f64,
    /// The three origin axes, each on its own. An axis running through the model
    /// is a distraction when it is not the one being worked to.
    #[serde(default = "all_axes")]
    pub axes_visible: [bool; 3],
    #[serde(default)]
    pub axis_style: AxisStyle,
    /// Draw, on the surface of a solid, the line where a principal plane cuts
    /// through it. Where the ground plane crosses a shape is a real dimension
    /// -- how much of it is below the build plate -- and it is invisible until
    /// something marks it.
    #[serde(default = "default_true")]
    pub plane_marks: bool,
    /// What the viewport does while a tool draws a preview over it (issue 82).
    /// Absent from the file while it is the default, so a project written by
    /// this version still diffs cleanly against one written before in-place
    /// previews existed.
    #[serde(default, skip_serializing_if = "is_no_change")]
    pub preview_viewport: PreviewViewport,
}

fn is_no_change(mode: &PreviewViewport) -> bool {
    *mode == PreviewViewport::NoChange
}

fn default_snap_step() -> f64 {
    1.0
}

fn all_axes() -> [bool; 3] {
    [true; 3]
}

impl Default for SceneSettings {
    fn default() -> Self {
        SceneSettings {
            unit: Unit::Millimetre,
            // 32 segments keeps a 3mm pin smooth and a 2m cylinder acceptable
            // without the user touching the setting (spec section 5.1).
            default_segments: 32,
            notes: String::new(),
            grid_spacing: 10.0,
            grid_visible: true,
            snap_step: default_snap_step(),
            axes_visible: all_axes(),
            axis_style: AxisStyle::default(),
            plane_marks: true,
            preview_viewport: PreviewViewport::NoChange,
        }
    }
}

/// Saved with the project (spec section 6.1: "The camera position is part of
/// the saved project").
///
/// The projection is always orthographic. A perspective view converges every
/// parallel line, which in a modelling tool means the grid lines, the origin
/// axes and the edges of a box all fan out from one another instead of running
/// together -- a box on the origin was drawn with the axes crossing its top
/// face at a visibly different angle from the grid they lie on. Nothing here is
/// judged by eye; every measurement is typed and read back, so the projection
/// that keeps parallels parallel is the only one worth having. Older files
/// carrying an `orthographic` flag still load: the field is simply ignored.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub target: Vec3,
    pub distance: f64,
    /// Degrees around Z.
    pub yaw: f64,
    /// Degrees above the XY plane.
    pub pitch: f64,
    /// How much of the scene the frame covers, expressed as the field of view a
    /// perspective camera at `distance` would need to cover the same height.
    /// Under orthographic projection it is one half of the zoom: `distance`
    /// times the tangent of half of this is the half-height of the frame in
    /// millimetres.
    pub fov_deg: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { target: Vec3::ZERO, distance: 160.0, yaw: -55.0, pitch: 28.0, fov_deg: 45.0 }
    }
}

#[derive(Clone, Debug)]
pub struct Scene {
    nodes: BTreeMap<NodeId, Node>,
    root: NodeId,
    next_id: NodeId,
    pub settings: SceneSettings,
    pub camera: Camera,
}

impl Default for Scene {
    fn default() -> Self {
        Scene::new()
    }
}

impl Scene {
    pub fn new() -> Scene {
        let root = Node {
            id: 1,
            name: "Scene".to_string(),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            ghost: false,
            colour: None,
            segments: None,
            export_body: None,
            extracted: false,
            body: Body::Group { op: GroupOp::Union },
            children: Vec::new(),
            parent: None,
        };
        let mut nodes = BTreeMap::new();
        nodes.insert(1, root);
        Scene { nodes, root: 1, next_id: 2, settings: SceneSettings::default(), camera: Camera::default() }
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes[&self.root].children.is_empty()
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id)
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    /// The node, or a panic. For the many callers that have already established
    /// the id is live -- one walked out of the tree, one just created -- and
    /// would only have `unwrap` to write instead.
    ///
    /// `#[track_caller]` because the panic is never about this line. An id that
    /// outlived its node is a bug where the id was *kept*, and a report naming
    /// `scene.rs` gives no way to tell which of the forty callers held it.
    #[track_caller]
    pub fn node(&self, id: NodeId) -> &Node {
        match self.nodes.get(&id) {
            Some(node) => node,
            None => panic!("node {id} is not in the scene"),
        }
    }

    pub fn ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes.keys().copied()
    }

    fn fresh_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Depth-first order, root first -- the outliner's order.
    pub fn depth_first(&self) -> Vec<NodeId> {
        let mut out = Vec::with_capacity(self.nodes.len());
        self.push_depth_first(self.root, &mut out);
        out
    }

    fn push_depth_first(&self, id: NodeId, out: &mut Vec<NodeId>) {
        out.push(id);
        for &child in &self.nodes[&id].children {
            self.push_depth_first(child, out);
        }
    }

    pub fn descendants(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        self.push_depth_first(id, &mut out);
        out.remove(0);
        out
    }

    pub fn is_ancestor_of(&self, ancestor: NodeId, mut node: NodeId) -> bool {
        while let Some(parent) = self.nodes.get(&node).and_then(|n| n.parent) {
            if parent == ancestor {
                return true;
            }
            node = parent;
        }
        false
    }

    /// Whether a node is actually drawn: hidden itself, or under anything
    /// hidden, and it is not. Hiding a group hides everything in it, so asking
    /// the node's own `visible` flag is not the same question.
    pub fn is_shown(&self, id: NodeId) -> bool {
        let mut at = Some(id);
        while let Some(node) = at.and_then(|id| self.nodes.get(&id)) {
            if !node.visible {
                return false;
            }
            at = node.parent;
        }
        true
    }

    pub fn depth(&self, mut id: NodeId) -> usize {
        let mut d = 0;
        while let Some(parent) = self.nodes.get(&id).and_then(|n| n.parent) {
            d += 1;
            id = parent;
        }
        d
    }

    /// Where a new node goes given the current selection: into it if it is a
    /// group, otherwise directly after it as a sibling (spec sections 7.2, 8.1).
    pub fn insertion_point(&self, selection: Option<NodeId>) -> (NodeId, usize) {
        match selection.and_then(|id| self.nodes.get(&id)) {
            // A collection is a container the tree does not open, so nothing is
            // put inside one by accident: a shape added while one is selected
            // stands beside it, the way it would beside a shape (issue 82).
            // Dragging something in is still a drop into it, because that is
            // aimed at rather than defaulted to.
            Some(node) if node.can_hold_children() && !node.is_split() => (node.id, node.children.len()),
            Some(node) => {
                let parent = node.parent.unwrap_or(self.root);
                let index = self.nodes[&parent].children.iter().position(|&c| c == node.id).map_or(0, |i| i + 1);
                (parent, index)
            }
            None => (self.root, self.nodes[&self.root].children.len()),
        }
    }

    /// Every name in the document, which is what a new name has to be free of.
    pub fn taken_names(&self) -> HashSet<String> {
        self.nodes.values().map(|n| n.name.clone()).collect()
    }

    /// A name no other node in the document carries, so the outliner stays
    /// readable.
    ///
    /// Scoped to the whole tree rather than to one parent's children: the
    /// outliner shows every depth at once, so two rows reading "Box 2" are two
    /// rows the user cannot tell apart, whether or not they happen to share a
    /// parent.
    fn unique_name(&self, base: &str) -> String {
        free_name(&self.taken_names(), base)
    }

    pub fn add_primitive(&mut self, type_id: &str, parent: NodeId, index: usize) -> Option<NodeId> {
        let spec = primitive::lookup(type_id)?;
        let id = self.fresh_id();
        let node = Node {
            id,
            name: self.unique_name(spec.label),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            ghost: false,
            colour: None,
            segments: None,
            export_body: None,
            extracted: false,
            body: Body::Primitive { type_id: type_id.to_string(), params: spec.default_params() },
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        Some(id)
    }

    pub fn add_group(&mut self, op: GroupOp, parent: NodeId, index: usize) -> NodeId {
        let id = self.fresh_id();
        let node = Node {
            id,
            name: self.unique_name("Group"),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            ghost: false,
            colour: None,
            segments: None,
            export_body: None,
            extracted: false,
            body: Body::Group { op },
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        id
    }

    /// Add a pattern node (issue 67), which repeats whatever children are put
    /// under it. It starts as a linear pattern with the default numbers.
    pub fn add_pattern(&mut self, parent: NodeId, index: usize) -> NodeId {
        let id = self.fresh_id();
        let node = Node {
            id,
            name: self.unique_name("Pattern"),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            ghost: false,
            colour: None,
            segments: None,
            export_body: None,
            extracted: false,
            body: Body::Pattern { params: crate::pattern::default_params() },
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        id
    }

    /// Add a node that owns the geometry it is given (issue 80).
    pub fn add_mesh(&mut self, name: &str, mesh: MeshData, parent: NodeId, index: usize) -> NodeId {
        let id = self.fresh_id();
        let name = self.unique_name(name);
        let node = Node {
            id,
            name,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            ghost: false,
            colour: None,
            segments: None,
            export_body: None,
            extracted: false,
            body: Body::Mesh { mesh: Arc::new(mesh) },
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        id
    }

    /// Add the node that holds what a shape was broken into (issue 82).
    ///
    /// It stands where the shape stood and wears the shape's own properties --
    /// its name, its transform, its anchor, its colour, its export mark -- so
    /// the pieces inside it are exactly where they were and the document still
    /// shows one item called what it was called. `original` is kept whole, and
    /// is what [`Scene::restore_split`] puts back.
    ///
    /// `tiling` says how the pieces were made, for a shape cut into a pattern
    /// of cells (issue 82); a shape merely separated into the pieces it was
    /// already in passes `None`.
    ///
    /// The caller adds the pieces as children afterwards; a split with none is
    /// as empty as a group with none, and evaluates to nothing.
    pub fn add_split(&mut self, original: NodeData, tiling: Option<Tiling>, parent: NodeId, index: usize) -> NodeId {
        let id = self.fresh_id();
        let node = Node {
            id,
            // Verbatim, not put through `unique_name`: the shape this was made
            // from is on its way out of the document, and its name goes to the
            // node standing in its place rather than to a second copy of it.
            name: original.name.clone(),
            position: original.position,
            rotation: original.rotation,
            scale: Node::sane_scale(original.scale),
            anchor: original.anchor,
            visible: original.visible,
            ghost: original.ghost,
            colour: original.colour.as_deref().and_then(Colour::from_hex),
            segments: original.segments,
            export_body: original.export_body,
            extracted: original.extracted,
            body: Body::Split { original: Arc::new(original), tiling },
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        id
    }

    /// Put a split back together: the object it was made from returns to its
    /// place, and the pieces go with the split node (issue 82).
    ///
    /// The restored object takes the *split's* current properties, not the ones
    /// it had when it was broken -- moving the pieces about as one item and then
    /// joining them back must not send the shape back to where it started. What
    /// was done to the pieces themselves is not carried across, because the
    /// object coming back is the recipe, not the triangles: that is the whole of
    /// what makes the break reversible, and undo is there for the other answer.
    ///
    /// Returns the id of the restored object, or `None` for a node that is not
    /// a split, or one whose stored recipe this build cannot rebuild.
    pub fn restore_split(&mut self, id: NodeId) -> Option<NodeId> {
        // The whole node, cloned: a split's body is a pointer to its recipe, so
        // this costs nothing and keeps every property in one place.
        let kept = self.nodes.get(&id)?.clone();
        let original = Arc::clone(kept.split_original()?);
        let parent = kept.parent?;
        let index = self.nodes.get(&parent)?.children.iter().position(|&c| c == id)?;
        // Rebuilt before the split is taken out, so a recipe that cannot be
        // rebuilt leaves the document exactly as it was rather than empty-handed.
        let restored = self.import_subtree(&original, parent, index)?;
        self.remove(id);
        let node = self.nodes.get_mut(&restored)?;
        node.name = kept.name;
        node.position = kept.position;
        node.rotation = kept.rotation;
        node.scale = kept.scale;
        node.anchor = kept.anchor;
        node.visible = kept.visible;
        node.ghost = kept.ghost;
        node.colour = kept.colour;
        node.segments = kept.segments;
        node.export_body = kept.export_body;
        // Including whether it had a row: a collection extracted out of another
        // collection and then joined back together is still the piece that was
        // extracted (issue 82).
        node.extracted = kept.extracted;
        self.rename_subtree_uniquely(restored, true);
        Some(restored)
    }

    // -- collections (issue 82) ---------------------------------------------

    /// Whether `id` holds its children *inside* itself rather than as rows of
    /// the tree: a split, which is one object in the outliner however many
    /// thousand pieces it is in.
    ///
    /// This is the whole of what makes a cut into ten thousand cells usable.
    /// The pieces are real nodes -- they evaluate, export, take a colour and a
    /// transform apiece -- but a tree with ten thousand rows in it is a tree
    /// nobody can find anything in, so they are reached through the split's own
    /// panel and only the ones asked for by name get a row.
    pub fn is_collection(&self, id: NodeId) -> bool {
        self.nodes.get(&id).is_some_and(Node::is_split)
    }

    /// Whether `id` is drawn as a row of the outliner at all.
    ///
    /// Everything is, except a piece still inside a collection. Asked of the
    /// node *and* its parent, because a piece extracted from one collection and
    /// dragged into another is a piece of the second one now.
    pub fn has_row(&self, id: NodeId) -> bool {
        let Some(node) = self.nodes.get(&id) else { return false };
        match node.parent {
            Some(parent) => node.extracted || !self.is_collection(parent),
            None => true,
        }
    }

    /// The nearest ancestor of `id` that the tree actually draws -- `id` itself
    /// for all but a piece inside a collection, and the collection for one of
    /// those.
    ///
    /// What a click in the viewport means: a click on a piece that has no row
    /// is a click on the collection, the way a click anywhere on a pattern's
    /// output means the pattern. A piece that *has* been extracted is a thing in
    /// its own right and answers as itself.
    pub fn row_for(&self, id: NodeId) -> NodeId {
        let mut walk = id;
        while !self.has_row(walk) {
            match self.nodes.get(&walk).and_then(|node| node.parent) {
                Some(parent) => walk = parent,
                None => break,
            }
        }
        walk
    }

    /// The children of `id` that the tree draws under it: all of them for
    /// everything but a collection, and the extracted ones for a collection.
    pub fn row_children(&self, id: NodeId) -> Vec<NodeId> {
        let Some(node) = self.nodes.get(&id) else { return Vec::new() };
        if !self.is_collection(id) {
            return node.children.clone();
        }
        node.children.iter().copied().filter(|c| self.nodes[c].extracted).collect()
    }

    /// Lift pieces out of the collection holding them, so each gets a row of
    /// its own under it (issue 82). Returns how many were newly marked.
    ///
    /// Only a child of `collection` can be extracted from it, and a piece
    /// already extracted is left alone rather than counted twice.
    pub fn extract_pieces(&mut self, collection: NodeId, pieces: &[NodeId]) -> usize {
        if !self.is_collection(collection) {
            return 0;
        }
        let mine: Vec<NodeId> =
            self.nodes[&collection].children.iter().copied().filter(|c| pieces.contains(c)).collect();
        let mut marked = 0;
        for id in mine {
            let node = self.nodes.get_mut(&id).expect("it was just read out of the collection");
            if !node.extracted {
                node.extracted = true;
                marked += 1;
            }
        }
        marked
    }

    /// Put extracted pieces back inside the collection they came from, losing
    /// their rows again. The inverse of [`Scene::extract_pieces`].
    pub fn return_pieces(&mut self, collection: NodeId, pieces: &[NodeId]) -> usize {
        if !self.is_collection(collection) {
            return 0;
        }
        let mine: Vec<NodeId> =
            self.nodes[&collection].children.iter().copied().filter(|c| pieces.contains(c)).collect();
        let mut cleared = 0;
        for id in mine {
            let node = self.nodes.get_mut(&id).expect("it was just read out of the collection");
            if node.extracted {
                node.extracted = false;
                cleared += 1;
            }
        }
        cleared
    }

    /// Turn a collection into an ordinary union group, its pieces becoming
    /// plain children of it (issue 82).
    ///
    /// What extracting the last piece comes to: with nothing left inside it, a
    /// collection is a container holding a list of objects, which is what a
    /// union group is -- and a union group is the thing the rest of the
    /// application already knows how to edit. The recipe the split was holding
    /// goes with it, so this is where the break stops being reversible; the
    /// caller is the one that says so before doing it.
    ///
    /// Returns false for a node that is not a collection.
    pub fn dissolve_collection(&mut self, id: NodeId) -> bool {
        if !self.is_collection(id) {
            return false;
        }
        for child in self.nodes[&id].children.clone() {
            if let Some(node) = self.nodes.get_mut(&child) {
                node.extracted = false;
            }
        }
        let Some(node) = self.nodes.get_mut(&id) else { return false };
        node.body = Body::Group { op: GroupOp::Union };
        true
    }

    /// Replace a node's body with stored geometry, keeping everything about the
    /// node that is not its shape -- its name, its place in the tree, its
    /// transform, its colour, its export mark (issue 80).
    ///
    /// Its children go with the old body, because they *were* the old body: the
    /// operands of a boolean are not parts of the result, and leaving them in
    /// the tree under a mesh that already contains them would double every
    /// solid in the export.
    pub fn convert_to_mesh(&mut self, id: NodeId, mesh: MeshData) -> bool {
        if id == self.root || !self.nodes.contains_key(&id) {
            return false;
        }
        for child in self.descendants(id) {
            self.nodes.remove(&child);
        }
        let Some(node) = self.nodes.get_mut(&id) else { return false };
        node.children.clear();
        node.body = Body::Mesh { mesh: Arc::new(mesh) };
        true
    }

    fn link(&mut self, id: NodeId, parent: NodeId, index: usize) {
        let children = &mut self.nodes.get_mut(&parent).expect("parent exists").children;
        let index = index.min(children.len());
        children.insert(index, id);
        self.nodes.get_mut(&id).unwrap().parent = Some(parent);
    }

    fn unlink(&mut self, id: NodeId) {
        if let Some(parent) = self.nodes[&id].parent {
            if let Some(p) = self.nodes.get_mut(&parent) {
                p.children.retain(|&c| c != id);
            }
        }
    }

    /// Delete a subtree. The root is protected (spec section 7.2).
    pub fn remove(&mut self, id: NodeId) -> bool {
        if id == self.root || !self.nodes.contains_key(&id) {
            return false;
        }
        self.unlink(id);
        for descendant in self.descendants(id) {
            self.nodes.remove(&descendant);
        }
        self.nodes.remove(&id);
        true
    }

    /// Deep copy with fresh identities, inserted directly after the original
    /// (spec section 7.2). Every property is preserved.
    pub fn duplicate(&mut self, id: NodeId) -> Option<NodeId> {
        if id == self.root {
            return None;
        }
        let parent = self.nodes.get(&id)?.parent?;
        let index = self.nodes[&parent].children.iter().position(|&c| c == id)? + 1;
        let mut data = self.export_subtree(id)?;
        // A duplicate is a copy of something already in the document, so it is
        // named the way a pasted copy is -- "Box copy", not a second "Box".
        data.name = free_name(&self.taken_names(), &copy_name(&data.name));
        let new_id = self.import_subtree(&data, parent, index)?;
        self.rename_subtree_uniquely(new_id, true);
        Some(new_id)
    }

    /// Give every node of a freshly imported subtree a name no other node in the
    /// document carries.
    ///
    /// `keep_top` is for a top whose name was already chosen against the
    /// document -- a duplicate's "Box copy", a paste's -- so it is not put
    /// through the rule twice and does not come out "Box copy 2" when nothing
    /// clashed.
    pub fn rename_subtree_uniquely(&mut self, id: NodeId, keep_top: bool) {
        let subtree: HashSet<NodeId> = std::iter::once(id).chain(self.descendants(id)).collect();
        let mut taken: HashSet<String> =
            self.nodes.values().filter(|n| !subtree.contains(&n.id)).map(|n| n.name.clone()).collect();
        if keep_top {
            taken.insert(self.nodes[&id].name.clone());
        }
        let mut stack = if keep_top { self.nodes[&id].children.clone() } else { vec![id] };
        stack.reverse();
        while let Some(node) = stack.pop() {
            let name = free_name(&taken, &self.nodes[&node].name);
            taken.insert(name.clone());
            self.nodes.get_mut(&node).unwrap().name = name;
            for &child in self.nodes[&node].children.iter().rev() {
                stack.push(child);
            }
        }
    }

    /// Move `id` under `new_parent` at `index`. Refuses to create a cycle and
    /// refuses to move the root (spec section 7.2).
    pub fn reparent(&mut self, id: NodeId, new_parent: NodeId, index: usize) -> Result<(), &'static str> {
        self.reparent_many(&[id], new_parent, index)
    }

    /// Move several nodes under `new_parent`, starting at `index` and keeping
    /// the order they are given in.
    ///
    /// Not a loop over `reparent` at the call site, because each single move
    /// would shift the index the next one was measured against -- and because
    /// the whole drag has to be refused as one when any part of it is illegal,
    /// rather than half-applied and then rejected. A node whose own ancestor is
    /// also being moved is left out: it travels inside it, and moving it as
    /// well would tear it out of the parent that carries it.
    pub fn reparent_many(&mut self, ids: &[NodeId], new_parent: NodeId, index: usize) -> Result<(), &'static str> {
        if !self.nodes.contains_key(&new_parent) {
            return Err("no such node");
        }
        if !self.nodes[&new_parent].can_hold_children() {
            return Err("only groups and patterns can hold children");
        }
        for &id in ids {
            if id == self.root {
                return Err("the scene root cannot be moved");
            }
            if !self.nodes.contains_key(&id) {
                return Err("no such node");
            }
            if new_parent == id || self.is_ancestor_of(id, new_parent) {
                return Err("a node cannot be moved inside itself");
            }
        }
        let mut moving: Vec<NodeId> = Vec::new();
        for &id in ids {
            let carried = ids.iter().any(|&other| other != id && self.is_ancestor_of(other, id));
            if !carried && !moving.contains(&id) {
                moving.push(id);
            }
        }
        // Index is interpreted against the target's child list *before* the
        // move, so dragging within one parent lands where the indicator showed.
        let mut index = index;
        for &id in &moving {
            if self.nodes[&new_parent].children.iter().position(|&c| c == id).is_some_and(|old| old < index) {
                index -= 1;
            }
        }
        for &id in &moving {
            self.unlink(id);
        }
        // Something dropped into a collection arrives with a row, and something
        // dragged out of one loses the mark it no longer means anything to: a
        // collection hides its *pieces*, and a node the user carried in by hand
        // is not one of them -- vanishing on release is not a move anybody aimed
        // for (issue 82).
        let into_collection = self.is_collection(new_parent);
        for (offset, &id) in moving.iter().enumerate() {
            self.link(id, new_parent, index + offset);
            if let Some(node) = self.nodes.get_mut(&id) {
                node.extracted = into_collection;
            }
        }
        Ok(())
    }

    /// Move a node up or down among its siblings. Order is semantic inside a
    /// difference group, so this has to be user-controllable.
    pub fn reorder(&mut self, id: NodeId, delta: isize) -> bool {
        let Some(parent) = self.nodes.get(&id).and_then(|n| n.parent) else { return false };
        let children = &mut self.nodes.get_mut(&parent).unwrap().children;
        let Some(from) = children.iter().position(|&c| c == id) else { return false };
        let to = from as isize + delta;
        if to < 0 || to >= children.len() as isize {
            return false;
        }
        let to = to as usize;
        children.remove(from);
        children.insert(to, id);
        true
    }

    /// Wrap the selection in a new group, preserving relative positions and
    /// order (spec section 7.2: "the single most-used structural operation").
    ///
    /// Only the topmost selected nodes are moved -- selecting a group and one of
    /// its children groups the group, not both. The new group's own position
    /// stays at the origin and children keep their coordinates, which is what
    /// keeps relative positions exactly unchanged.
    pub fn group_selection(&mut self, selection: &[NodeId]) -> Option<NodeId> {
        let mut tops: Vec<NodeId> = selection
            .iter()
            .copied()
            .filter(|&id| id != self.root && self.nodes.contains_key(&id))
            .filter(|&id| !selection.iter().any(|&other| other != id && self.is_ancestor_of(other, id)))
            .collect();
        if tops.is_empty() {
            return None;
        }
        // Group into the first selected node's parent, at its position, keeping
        // the tree's own order rather than click order.
        let parent = self.nodes[&tops[0]].parent?;
        let order = self.depth_first();
        tops.sort_by_key(|id| order.iter().position(|o| o == id).unwrap_or(usize::MAX));
        tops.retain(|&id| self.nodes[&id].parent == Some(parent));
        if tops.is_empty() {
            return None;
        }
        let index = self.nodes[&parent].children.iter().position(|c| *c == tops[0])?;
        let group = self.add_group(GroupOp::Union, parent, index);
        for (offset, id) in tops.iter().enumerate() {
            self.reparent(*id, group, offset).ok()?;
        }
        Some(group)
    }

    /// A group's base child in a difference: the first *visible* one (spec
    /// section 3.3). The property editor states this plainly.
    pub fn difference_base(&self, group: NodeId) -> Option<NodeId> {
        self.nodes.get(&group)?.children.iter().copied().find(|c| self.nodes[c].visible)
    }

    // -- portable form (project file and clipboard share one schema) ---------

    pub fn export_subtree(&self, id: NodeId) -> Option<NodeData> {
        let node = self.nodes.get(&id)?;
        let mut blob: Option<MeshBlob> = None;
        let mut original: Option<Box<NodeData>> = None;
        let mut tiling: Option<Tiling> = None;
        let (type_id, op, params) = match &node.body {
            Body::Group { op } => ("group".to_string(), Some(*op), Params::new()),
            Body::Primitive { type_id, params } => (type_id.clone(), None, params.clone()),
            Body::Pattern { params } => ("pattern".to_string(), None, params.clone()),
            Body::Mesh { mesh } => {
                blob = Some(mesh.to_blob());
                ("mesh".to_string(), None, Params::new())
            }
            Body::Split { original: was, tiling: cut } => {
                original = Some(Box::new((**was).clone()));
                tiling = *cut;
                ("split".to_string(), None, Params::new())
            }
        };
        Some(NodeData {
            name: node.name.clone(),
            type_id,
            op,
            position: node.position,
            rotation: node.rotation,
            scale: node.scale,
            anchor: node.anchor,
            visible: node.visible,
            ghost: node.ghost,
            colour: node.colour.map(Colour::to_hex),
            segments: node.segments,
            export_body: node.export_body,
            extracted: node.extracted,
            mesh: blob,
            original,
            tiling,
            params,
            children: node.children.iter().filter_map(|&c| self.export_subtree(c)).collect(),
        })
    }

    /// Insert a portable subtree, giving every node a fresh identity. Unknown
    /// primitive types are rejected so a corrupt or newer file cannot produce a
    /// half-loaded scene.
    pub fn import_subtree(&mut self, data: &NodeData, parent: NodeId, index: usize) -> Option<NodeId> {
        let body = match data.type_id.as_str() {
            "group" => Body::Group { op: data.op.unwrap_or_default() },
            "pattern" => Body::Pattern { params: crate::pattern::migrate_params(&data.params) },
            // A mesh whose blob cannot be read is refused rather than loaded as
            // an empty node: the geometry is the whole of what the node is, and
            // a silently empty one would be a body quietly missing from a print.
            "mesh" => Body::Mesh { mesh: Arc::new(MeshData::from_blob(data.mesh.as_ref()?)?) },
            // A split without the object it was made from is refused for the
            // same reason: what the node *is* is missing. Its pieces would still
            // draw, but it would be a shape broken apart with no way back, which
            // is not the node the file says it is.
            "split" => Body::Split { original: Arc::new((**data.original.as_ref()?).clone()), tiling: data.tiling },
            type_id => {
                let spec = primitive::lookup(type_id)?;
                Body::Primitive { type_id: data.type_id.clone(), params: spec.migrate_params(&data.params) }
            }
        };
        let id = self.fresh_id();
        let node = Node {
            id,
            name: if data.name.is_empty() { "Node".to_string() } else { data.name.clone() },
            position: data.position,
            rotation: data.rotation,
            scale: Node::sane_scale(data.scale),
            anchor: data.anchor,
            visible: data.visible,
            ghost: data.ghost,
            colour: data.colour.as_deref().and_then(Colour::from_hex),
            segments: data.segments,
            export_body: data.export_body,
            extracted: data.extracted,
            body,
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        for (i, child) in data.children.iter().enumerate() {
            if self.import_subtree(child, id, i).is_none() {
                self.remove(id);
                return None;
            }
        }
        Some(id)
    }

    /// Replace the whole tree from a portable root, used when loading a project.
    pub fn replace_root(&mut self, data: &NodeData) -> Option<()> {
        let mut fresh = Scene::new();
        fresh.settings = self.settings.clone();
        fresh.camera = self.camera;
        {
            let root = fresh.nodes.get_mut(&fresh.root).unwrap();
            root.name = data.name.clone();
            root.body = Body::Group { op: data.op.unwrap_or_default() };
            root.position = data.position;
            root.rotation = data.rotation;
            root.scale = Node::sane_scale(data.scale);
            root.anchor = data.anchor;
            root.visible = data.visible;
            root.segments = data.segments;
        }
        let root = fresh.root;
        for (i, child) in data.children.iter().enumerate() {
            fresh.import_subtree(child, root, i)?;
        }
        self.nodes = fresh.nodes;
        self.root = fresh.root;
        self.next_id = fresh.next_id;
        Some(())
    }

    /// What a node is painted, following the tree upward: its own colour, else
    /// the nearest painted ancestor's, else nothing at all. This is what makes
    /// painting a group paint every shape inside it without touching any of
    /// them, and what a shape painted inside a painted group overrides.
    /// Whether `id` is a node whose children an export could consider one by
    /// one. A primitive has no parts, and a boolean that fuses its operands has
    /// none that survive it. A split's children are separate solids by
    /// construction -- that is what breaking a shape apart found -- so it always
    /// can.
    pub fn can_split_for_export(&self, id: NodeId) -> bool {
        self.get(id).and_then(|n| n.combine_op()).is_some_and(GroupOp::separable)
    }

    /// Every export body mark in the scene, by node, in a stable order. Small
    /// -- a scene nobody has grouped has none -- and it is what tells a cached
    /// export summary that the grouping has been edited under it.
    pub fn export_body_marks(&self) -> Vec<(NodeId, ExportBody)> {
        self.nodes.iter().filter_map(|(id, node)| node.export_body.map(|body| (*id, body))).collect()
    }

    /// The largest body number used anywhere, so a picker can offer the next
    /// one. Zero when nothing is grouped, which makes the first offer "Body 1".
    pub fn highest_export_body(&self) -> u32 {
        self.export_body_marks()
            .into_iter()
            .filter_map(|(_, body)| match body {
                ExportBody::Shared(n) => Some(n),
                ExportBody::Split => None,
            })
            .max()
            .unwrap_or(0)
    }

    /// Give `id` a body mark, clearing anything below it that the mark makes
    /// unreachable: marking a group as a body of its own leaves the marks
    /// inside it saying something that is no longer true, and a stale mark that
    /// springs back when the group is split again is worse than none.
    pub fn set_export_body(&mut self, id: NodeId, body: Option<ExportBody>) {
        if let Some(node) = self.get_mut(id) {
            node.export_body = body;
        }
        if body != Some(ExportBody::Split) {
            for descendant in self.descendants(id) {
                if let Some(node) = self.get_mut(descendant) {
                    node.export_body = None;
                }
            }
        }
    }

    pub fn effective_colour(&self, id: NodeId) -> Option<Colour> {
        let mut at = Some(id);
        while let Some(node) = at.and_then(|id| self.nodes.get(&id)) {
            if node.colour.is_some() {
                return node.colour;
            }
            at = node.parent;
        }
        None
    }

    /// Whether this node, or anything under it, carries a colour of its own --
    /// which is exactly when clearing has anything to do. A node that merely
    /// inherits its colour from a group above it has nothing of its own to
    /// clear, and offering to clear it would be a control that does nothing.
    pub fn subtree_is_painted(&self, id: NodeId) -> bool {
        let mut stack = vec![id];
        while let Some(at) = stack.pop() {
            let Some(node) = self.nodes.get(&at) else { continue };
            if node.colour.is_some() {
                return true;
            }
            stack.extend(node.children.iter().copied());
        }
        false
    }

    /// Paint a node and everything under it. Clearing the descendants' own
    /// colours is the point: "paint this group red" means the whole group turns
    /// red, not that the shapes which were painted individually keep their own.
    /// Passing `None` strips the colour from the subtree entirely.
    pub fn paint_subtree(&mut self, id: NodeId, colour: Option<Colour>) {
        let mut stack = vec![id];
        while let Some(at) = stack.pop() {
            let Some(node) = self.nodes.get_mut(&at) else { continue };
            node.colour = if at == id { colour } else { None };
            stack.extend(node.children.iter().copied());
        }
    }

    /// The effective segment count for a node: its own override, else the
    /// scene default.
    pub fn segments_for(&self, id: NodeId) -> u32 {
        self.nodes.get(&id).and_then(|n| n.segments).unwrap_or(self.settings.default_segments).clamp(3, 512)
    }
}

fn default_true() -> bool {
    true
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// The readable, diffable form of a node used by both the project file and the
/// clipboard, so a selection can be pasted into a text editor and back again
/// (spec sections 8.1, 10).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeData {
    pub name: String,
    #[serde(rename = "type")]
    pub type_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<GroupOp>,
    #[serde(default = "Vec3_zero")]
    pub position: Vec3,
    #[serde(default = "Vec3_zero")]
    pub rotation: Vec3,
    /// Left out of the file entirely when it is `1, 1, 1`, which is almost
    /// always, so a project written by this version still diffs cleanly against
    /// one written before scale existed.
    #[serde(default = "Vec3_one", skip_serializing_if = "is_unit_scale")]
    pub scale: Vec3,
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Absent from the file for every node that is not a ghost, which is nearly
    /// all of them.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ghost: bool,
    /// `#rrggbb`, and absent from the file for the usual unpainted node, so a
    /// project written by this version still diffs cleanly against one written
    /// before colours existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<u32>,
    /// Absent from the file for every node that is a body of its own, which is
    /// every node until an export is told to group them differently -- so a
    /// project written by this version still diffs cleanly against one written
    /// before export bodies existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_body: Option<ExportBody>,
    /// Whether a piece has been lifted out of the collection holding it, so
    /// that it has a row of its own in the tree (issue 82). Absent from the
    /// file for every node that is not one -- which is every node in a project
    /// that has never split anything -- so a project written by this version
    /// still diffs cleanly against one written before collections existed.
    #[serde(default, skip_serializing_if = "is_false")]
    pub extracted: bool,
    /// The geometry of a `mesh` node, and nothing else's. Present only on the
    /// one body type that owns its triangles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<MeshBlob>,
    /// The object a `split` node was broken apart from, and nothing else's:
    /// the whole subtree, so joining the pieces back together rebuilds the
    /// shape with its operands and its parameters intact (issue 82).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<Box<NodeData>>,
    /// How a `split` node's pieces were cut, and nothing else's: the cell shape
    /// and its numbers (issue 82). Absent for a split that merely separated a
    /// shape into the pieces it was already in, and for every other node -- so a
    /// project written by this version still diffs cleanly against one written
    /// before a split could be a pattern of cells.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiling: Option<Tiling>,
    #[serde(default, skip_serializing_if = "Params::is_empty")]
    pub params: Params,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NodeData>,
}

#[allow(non_snake_case)]
fn Vec3_zero() -> Vec3 {
    Vec3::ZERO
}

#[allow(non_snake_case)]
fn Vec3_one() -> Vec3 {
    Vec3::ONE
}

fn is_unit_scale(scale: &Vec3) -> bool {
    *scale == Vec3::ONE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_at(scene: &mut Scene, parent: NodeId, x: f64) -> NodeId {
        let index = scene.node(parent).children.len();
        let id = scene.add_primitive("box", parent, index).unwrap();
        scene.get_mut(id).unwrap().position = Vec3::new(x, 0.0, 0.0);
        id
    }

    #[test]
    fn a_split_stands_where_the_shape_stood_and_gives_it_back_on_request() {
        // Issue 82: breaking a shape apart must be reversible, so the node the
        // pieces go under carries the shape itself, and putting it back is one
        // call rather than a rebuild by hand.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        scene.get_mut(group).unwrap().name = "Bracket".into();
        scene.get_mut(group).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
        scene.get_mut(group).unwrap().anchor = Anchor::Base;
        box_at(&mut scene, group, 0.0);
        box_at(&mut scene, group, 60.0);

        let original = scene.export_subtree(group).unwrap();
        scene.remove(group);
        let split = scene.add_split(original, None, root, 0);
        assert!(scene.node(split).is_split());
        // The shape's own properties came across, name included.
        assert_eq!(scene.node(split).name, "Bracket");
        assert_eq!(scene.node(split).position, Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(scene.node(split).anchor, Anchor::Base);
        assert_eq!(scene.node(split).group_op(), None, "a split is not a group");
        assert_eq!(scene.node(split).combine_op(), Some(GroupOp::Union), "its pieces stand side by side");
        assert!(scene.can_split_for_export(split), "its pieces are separate solids and an export can say so");

        // Whatever is done to the node afterwards is what the restored shape
        // wears: the recipe says what it is, the node says where it is.
        scene.get_mut(split).unwrap().position = Vec3::new(5.0, 0.0, 40.0);
        scene.get_mut(split).unwrap().name = "Bracket, in pieces".into();
        let back = scene.restore_split(split).unwrap();
        assert!(!scene.contains(split), "the split outlived the shape it gave back");
        assert_eq!(scene.node(back).group_op(), Some(GroupOp::Union));
        assert_eq!(scene.node(back).children.len(), 2, "the operands did not come back");
        assert_eq!(scene.node(back).name, "Bracket, in pieces");
        assert_eq!(scene.node(back).position, Vec3::new(5.0, 0.0, 40.0));
        assert_eq!(scene.node(back).anchor, Anchor::Base);
        assert_eq!(scene.node(root).children, vec![back], "it came back somewhere else in the tree");
    }

    #[test]
    fn a_split_survives_the_portable_form_with_its_shape_and_its_pieces() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        box_at(&mut scene, group, 0.0);
        box_at(&mut scene, group, 5.0);
        let original = scene.export_subtree(group).unwrap();
        scene.remove(group);
        let split = scene.add_split(original, None, root, 0);
        let piece = crate::mesh_data::MeshData::new(simple3d_geom::primitives::box_mesh(10.0, 10.0, 10.0));
        scene.add_mesh("Piece", piece, split, 0);

        let data = scene.export_subtree(split).unwrap();
        assert_eq!(data.type_id, "split");
        let mut other = Scene::new();
        let other_root = other.root();
        let copy = other.import_subtree(&data, other_root, 0).expect("a split imports");
        assert!(other.node(copy).is_split());
        assert_eq!(other.node(copy).children.len(), 1, "the piece was lost");
        let back = other.restore_split(copy).expect("the shape rebuilds");
        assert_eq!(other.node(back).group_op(), Some(GroupOp::Difference));
        assert_eq!(other.node(back).children.len(), 2);

        // A split with no shape behind it is not a split, and is refused the way
        // a mesh with no geometry is rather than loaded as something else.
        let mut hollow = data.clone();
        hollow.original = None;
        assert!(other.import_subtree(&hollow, other_root, 0).is_none());
    }

    /// A collection holding `pieces` nameless mesh pieces, standing where a
    /// two-box difference stood: the shape both halves of issue 82 leave behind.
    fn collection_of(scene: &mut Scene, pieces: usize) -> (NodeId, Vec<NodeId>) {
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        box_at(scene, group, 0.0);
        box_at(scene, group, 5.0);
        let original = scene.export_subtree(group).unwrap();
        scene.remove(group);
        let split = scene.add_split(original, None, root, 0);
        let made = (0..pieces)
            .map(|i| {
                let mesh = crate::mesh_data::MeshData::new(simple3d_geom::primitives::box_mesh(1.0, 1.0, 1.0));
                scene.add_mesh(&format!("Piece {i}"), mesh, split, i)
            })
            .collect();
        (split, made)
    }

    #[test]
    fn a_collection_keeps_its_pieces_out_of_the_tree() {
        // The point of issue 82's rework: the pieces are real nodes -- they
        // evaluate, export and take a transform apiece -- but the tree shows the
        // collection as one object however many thousand it is in.
        let mut scene = Scene::new();
        let (split, pieces) = collection_of(&mut scene, 4);
        assert!(scene.is_collection(split));
        assert!(scene.has_row(split), "the collection itself must be a row");
        assert!(scene.row_children(split).is_empty(), "the pieces were drawn in the tree");
        for &piece in &pieces {
            assert!(!scene.has_row(piece), "a piece inside the collection has a row");
            assert_eq!(scene.row_for(piece), split, "a click on a piece must mean the collection");
        }
        // And the pieces are still there, whatever the tree draws.
        assert_eq!(scene.node(split).children.len(), 4);
    }

    #[test]
    fn extracting_a_piece_gives_it_a_row_and_putting_it_back_takes_it_away() {
        let mut scene = Scene::new();
        let (split, pieces) = collection_of(&mut scene, 4);

        assert_eq!(scene.extract_pieces(split, &pieces[..2]), 2);
        assert_eq!(scene.row_children(split), pieces[..2].to_vec(), "the extracted pieces are the rows");
        assert!(scene.has_row(pieces[0]));
        assert_eq!(scene.row_for(pieces[0]), pieces[0], "an extracted piece answers as itself");
        assert_eq!(scene.row_for(pieces[2]), split, "an untouched piece still answers as the collection");
        // Extracting the same piece twice is not two extractions.
        assert_eq!(scene.extract_pieces(split, &pieces[..2]), 0);
        assert!(scene.is_collection(split), "extracting some pieces dissolved the collection");

        assert_eq!(scene.return_pieces(split, &pieces[..1]), 1);
        assert_eq!(scene.row_children(split), vec![pieces[1]]);
    }

    #[test]
    fn a_piece_that_is_not_this_collections_is_not_extracted_from_it() {
        // The list a panel hands over is what is ticked, and what is ticked can
        // outlive the collection it was ticked in.
        let mut scene = Scene::new();
        let (split, pieces) = collection_of(&mut scene, 2);
        let root = scene.root();
        let stranger = box_at(&mut scene, root, 20.0);
        assert_eq!(scene.extract_pieces(split, &[stranger, pieces[0]]), 1);
        assert!(!scene.node(stranger).extracted, "a node from elsewhere was marked");
    }

    #[test]
    fn emptying_a_collection_leaves_an_ordinary_union_group() {
        // What extracting the last piece comes to: with nothing left inside it a
        // collection is a container holding a list of objects, which is a union
        // group -- and the recipe goes with it, which is why the application
        // asks first.
        let mut scene = Scene::new();
        let (split, pieces) = collection_of(&mut scene, 3);
        assert!(scene.dissolve_collection(split));
        assert!(!scene.is_collection(split));
        assert_eq!(scene.node(split).group_op(), Some(GroupOp::Union));
        assert!(scene.node(split).split_original().is_none(), "the recipe outlived the collection");
        assert_eq!(scene.row_children(split), pieces, "the pieces did not become ordinary rows");
        for &piece in &pieces {
            assert!(scene.has_row(piece));
            assert!(!scene.node(piece).extracted, "the mark outlived the collection it meant something in");
        }
        assert!(!scene.dissolve_collection(split), "a union group is not a collection to dissolve");
    }

    #[test]
    fn something_dropped_into_a_collection_arrives_with_a_row() {
        // A collection hides its *pieces*. Something the user carried in by hand
        // is not one of them, and vanishing on release is not a move anybody
        // aimed for.
        let mut scene = Scene::new();
        let (split, _) = collection_of(&mut scene, 2);
        let root = scene.root();
        let boxed = box_at(&mut scene, root, 20.0);
        scene.reparent(boxed, split, 0).expect("a collection can hold children");
        assert!(scene.node(boxed).extracted);
        assert!(scene.has_row(boxed), "a shape dropped into a collection disappeared");

        // And dragged out again it loses a mark that means nothing there.
        scene.reparent(boxed, root, 0).expect("it can come out again");
        assert!(!scene.node(boxed).extracted);
        assert!(scene.has_row(boxed));
    }

    #[test]
    fn which_pieces_are_extracted_survives_the_portable_form() {
        let mut scene = Scene::new();
        let (split, pieces) = collection_of(&mut scene, 3);
        scene.extract_pieces(split, &pieces[1..2]);

        let data = scene.export_subtree(split).unwrap();
        let json = serde_json::to_string(&data).unwrap();
        // The mark is absent from every node that does not carry one, so a
        // project that has never split anything writes exactly what it used to.
        assert_eq!(json.matches("\"extracted\"").count(), 1, "{json}");
        let back: NodeData = serde_json::from_str(&json).unwrap();

        let mut other = Scene::new();
        let other_root = other.root();
        let copy = other.import_subtree(&back, other_root, 0).expect("a collection imports");
        assert_eq!(other.row_children(copy).len(), 1, "the extracted piece lost its row over the round trip");
        assert_eq!(other.node(other.row_children(copy)[0]).name, "Piece 1");
    }

    #[test]
    fn nothing_is_added_inside_a_collection_by_accident() {
        // The tree does not open a collection, so an Add with one selected must
        // not put a shape somewhere it cannot be seen. Dragging one in still
        // works: that is aimed at rather than defaulted to.
        let mut scene = Scene::new();
        let (split, _) = collection_of(&mut scene, 2);
        let root = scene.root();
        assert_eq!(scene.insertion_point(Some(split)), (root, 1), "an Add landed inside the collection");
    }

    #[test]
    fn a_duplicate_is_named_as_a_copy() {
        // A duplicate is the same thing the clipboard makes, so it reads the
        // same way; two siblings both called "Box" say nothing about which is
        // which.
        let mut scene = Scene::new();
        let root = scene.root();
        let a = box_at(&mut scene, root, 0.0);
        let copy = scene.duplicate(a).unwrap();
        assert_eq!(scene.node(a).name, "Box");
        assert_eq!(scene.node(copy).name, "Box copy");
        let again = scene.duplicate(a).unwrap();
        assert_eq!(scene.node(again).name, "Box copy 2");

        // Duplicating a duplicate is how a row of something actually gets laid
        // out, and each one is of the one just made. Without trimming the
        // suffix the fourth press of Ctrl+D reads "Box copy copy copy copy".
        let mut chained = copy;
        for expected in ["Box copy 3", "Box copy 4", "Box copy 5"] {
            chained = scene.duplicate(chained).unwrap();
            assert_eq!(scene.node(chained).name, expected);
        }
    }

    #[test]
    fn a_duplicated_group_renames_what_travels_inside_it() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        box_at(&mut scene, group, 0.0);
        let copy = scene.duplicate(group).unwrap();
        assert_eq!(scene.node(copy).name, "Group copy");
        let inside = scene.node(copy).children[0];
        assert_eq!(scene.node(inside).name, "Box 2");
    }

    #[test]
    fn a_name_is_free_across_the_tree_not_only_among_siblings() {
        // The outliner shows every depth at once, so a "Box 2" nested in a
        // pattern is a row the user has to tell apart from a "Box 2" beside it.
        let mut scene = Scene::new();
        let root = scene.root();
        let pattern = scene.add_pattern(root, 0);
        box_at(&mut scene, pattern, 0.0);
        let beside = box_at(&mut scene, root, 0.0);
        assert_eq!(scene.node(beside).name, "Box 2");

        let names: Vec<String> = scene.ids().map(|id| scene.node(id).name.clone()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "{names:?}");
    }

    #[test]
    fn add_targets_the_selected_group_or_follows_a_leaf() {
        let mut scene = Scene::new();
        let root = scene.root();
        let a = box_at(&mut scene, root, 0.0);
        let group = scene.add_group(GroupOp::Union, root, 1);

        assert_eq!(scene.insertion_point(Some(group)), (group, 0));
        assert_eq!(scene.insertion_point(Some(a)), (root, 1));
        assert_eq!(scene.insertion_point(None), (root, 2));
    }

    #[test]
    fn only_a_subtree_that_carries_a_colour_has_one_to_clear() {
        // What decides whether the Clear control is offered: a shape that
        // merely inherits a group's colour has nothing of its own to take
        // away, and a control that cannot do anything should not be live.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        let child = box_at(&mut scene, group, 0.0);
        assert!(!scene.subtree_is_painted(group));
        assert!(!scene.subtree_is_painted(child));

        scene.paint_subtree(group, Some(Colour([1, 2, 3])));
        assert!(scene.subtree_is_painted(group), "the group carries the colour");
        assert!(!scene.subtree_is_painted(child), "the child only inherits it");
        assert_eq!(scene.effective_colour(child), Some(Colour([1, 2, 3])));

        scene.paint_subtree(child, Some(Colour([9, 9, 9])));
        assert!(scene.subtree_is_painted(child));
        // And clearing the group takes the child's own colour with it, which is
        // what makes painting a group mean the whole group.
        scene.paint_subtree(group, None);
        assert!(!scene.subtree_is_painted(group));
        assert_eq!(scene.effective_colour(child), None);
    }

    #[test]
    fn a_colour_reads_back_from_the_hex_it_is_written_as() {
        assert_eq!(Colour::from_hex("#2e9aff"), Some(Colour([0x2E, 0x9A, 0xFF])));
        assert_eq!(Colour::from_hex("2E9AFF"), Some(Colour([0x2E, 0x9A, 0xFF])));
        assert_eq!(Colour([0x2E, 0x9A, 0xFF]).to_hex(), "#2e9aff");
        for text in ["", "#", "#12345", "#1234567", "#gggggg", "nonsense"] {
            assert_eq!(Colour::from_hex(text), None, "{text:?} is not a colour");
        }
    }

    #[test]
    fn duplicate_is_a_deep_copy_with_distinct_identity() {
        // Spec acceptance criterion 20/21.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        let child = box_at(&mut scene, group, 5.0);
        scene.get_mut(child).unwrap().anchor = Anchor::Base;
        scene.get_mut(child).unwrap().segments = Some(64);
        scene.get_mut(child).unwrap().visible = false;

        let copy = scene.duplicate(group).unwrap();
        assert_ne!(copy, group);
        assert_eq!(scene.node(root).children, vec![group, copy]);
        assert_eq!(scene.node(copy).group_op(), Some(GroupOp::Difference));
        let copied_child = scene.node(copy).children[0];
        assert_ne!(copied_child, child);
        assert_eq!(scene.node(copied_child).position, Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(scene.node(copied_child).anchor, Anchor::Base);
        assert_eq!(scene.node(copied_child).segments, Some(64));
        assert!(!scene.node(copied_child).visible);

        // Editing the copy leaves the original untouched.
        scene.get_mut(copied_child).unwrap().position = Vec3::new(99.0, 0.0, 0.0);
        assert_eq!(scene.node(child).position, Vec3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn reparent_refuses_cycles_and_protects_the_root() {
        let mut scene = Scene::new();
        let root = scene.root();
        let outer = scene.add_group(GroupOp::Union, root, 0);
        let inner = scene.add_group(GroupOp::Union, outer, 0);

        assert!(scene.reparent(outer, inner, 0).is_err());
        assert!(scene.reparent(outer, outer, 0).is_err());
        assert!(scene.reparent(root, outer, 0).is_err());
        assert!(scene.reparent(inner, root, 0).is_ok());
        assert_eq!(scene.node(inner).parent, Some(root));
    }

    #[test]
    fn reparent_within_one_parent_lands_where_the_indicator_showed() {
        let mut scene = Scene::new();
        let root = scene.root();
        let a = box_at(&mut scene, root, 0.0);
        let b = box_at(&mut scene, root, 1.0);
        let c = box_at(&mut scene, root, 2.0);
        // Drop `a` between `b` and `c`: index 2 in the pre-move list.
        scene.reparent(a, root, 2).unwrap();
        assert_eq!(scene.node(root).children, vec![b, a, c]);
    }

    #[test]
    fn reparenting_several_nodes_keeps_their_order_and_lands_where_one_would() {
        // Issue 43: a multi-node drag is one move, not a loop of single moves --
        // each of those would shift the index the next was measured against.
        let mut scene = Scene::new();
        let root = scene.root();
        let a = box_at(&mut scene, root, 0.0);
        let b = box_at(&mut scene, root, 1.0);
        let c = box_at(&mut scene, root, 2.0);
        let d = box_at(&mut scene, root, 3.0);
        let group = scene.add_group(GroupOp::Union, root, 4);

        scene.reparent_many(&[a, c], group, 0).unwrap();
        assert_eq!(scene.node(group).children, vec![a, c], "the two lost their order on the way in");
        assert_eq!(scene.node(root).children, vec![b, d, group]);

        // Back out, between `b` and `d`: the index is read against the list as
        // it stands before the move, exactly as for a single node.
        scene.reparent_many(&[a, c], root, 1).unwrap();
        assert_eq!(scene.node(root).children, vec![b, a, c, d, group]);

        // A run that moves within one parent counts what leaves from in front
        // of the target, so it lands where the indicator was drawn.
        scene.reparent_many(&[b, a], root, 3).unwrap();
        assert_eq!(scene.node(root).children, vec![c, b, a, d, group]);
    }

    #[test]
    fn a_multi_node_reparent_is_refused_whole_and_never_moves_a_carried_child() {
        let mut scene = Scene::new();
        let root = scene.root();
        let outer = scene.add_group(GroupOp::Union, root, 0);
        let inner = scene.add_group(GroupOp::Union, outer, 0);
        let leaf = box_at(&mut scene, inner, 0.0);
        let other = box_at(&mut scene, root, 1.0);

        // One illegal member fails the whole drag, and nothing has moved.
        assert!(scene.reparent_many(&[other, outer], inner, 0).is_err());
        assert_eq!(scene.node(root).children, vec![outer, other]);
        assert_eq!(scene.node(outer).children, vec![inner]);

        // A node inside another node being moved travels inside it; moving it
        // as well would tear it out of the group that carries it.
        scene.reparent_many(&[outer, leaf], root, 0).unwrap();
        assert_eq!(scene.node(inner).children, vec![leaf]);
        assert_eq!(scene.node(root).children, vec![outer, other]);
    }

    #[test]
    fn grouping_a_selection_preserves_relative_positions_and_order() {
        // Spec acceptance criterion 10.
        let mut scene = Scene::new();
        let root = scene.root();
        let ids: Vec<NodeId> = (0..5).map(|i| box_at(&mut scene, root, i as f64 * 10.0)).collect();
        let before: Vec<Vec3> = ids.iter().map(|&id| scene.node(id).position).collect();

        let group = scene.group_selection(&ids).unwrap();
        assert_eq!(scene.node(group).children, ids);
        assert_eq!(scene.node(group).position, Vec3::ZERO);
        for (&id, &pos) in ids.iter().zip(before.iter()) {
            assert_eq!(scene.node(id).position, pos);
            assert_eq!(scene.node(id).parent, Some(group));
        }
        assert_eq!(scene.node(root).children, vec![group]);
    }

    #[test]
    fn grouping_ignores_children_of_an_already_selected_group() {
        let mut scene = Scene::new();
        let root = scene.root();
        let outer = scene.add_group(GroupOp::Union, root, 0);
        let child = box_at(&mut scene, outer, 0.0);
        let sibling = box_at(&mut scene, root, 5.0);

        let group = scene.group_selection(&[outer, child, sibling]).unwrap();
        assert_eq!(scene.node(group).children, vec![outer, sibling]);
        assert_eq!(scene.node(child).parent, Some(outer));
    }

    #[test]
    fn deleting_the_root_is_refused_and_subtrees_go_entirely() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        let child = box_at(&mut scene, group, 0.0);
        assert!(!scene.remove(root));
        assert!(scene.remove(group));
        assert!(!scene.contains(child));
        assert_eq!(scene.len(), 1);
    }

    #[test]
    fn reorder_moves_within_the_parent_only() {
        let mut scene = Scene::new();
        let root = scene.root();
        let a = box_at(&mut scene, root, 0.0);
        let b = box_at(&mut scene, root, 1.0);
        assert!(scene.reorder(b, -1));
        assert_eq!(scene.node(root).children, vec![b, a]);
        assert!(!scene.reorder(b, -1));
        assert!(scene.reorder(b, 1));
        assert_eq!(scene.node(root).children, vec![a, b]);
    }

    #[test]
    fn difference_base_is_the_first_visible_child() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        let a = box_at(&mut scene, group, 0.0);
        let b = box_at(&mut scene, group, 1.0);
        assert_eq!(scene.difference_base(group), Some(a));
        scene.get_mut(a).unwrap().visible = false;
        assert_eq!(scene.difference_base(group), Some(b));
    }

    #[test]
    fn subtree_round_trips_through_the_portable_form() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Intersection, root, 0);
        let child = box_at(&mut scene, group, 7.0);
        scene.get_mut(child).unwrap().rotation = Vec3::new(0.0, 45.0, 0.0);
        scene.get_mut(child).unwrap().name = "Special".into();

        let data = scene.export_subtree(group).unwrap();
        let json = serde_json::to_string(&data).unwrap();
        let back: NodeData = serde_json::from_str(&json).unwrap();
        let pasted = scene.import_subtree(&back, root, 1).unwrap();
        let pasted_child = scene.node(pasted).children[0];
        assert_eq!(scene.node(pasted).group_op(), Some(GroupOp::Intersection));
        assert_eq!(scene.node(pasted_child).name, "Special");
        assert_eq!(scene.node(pasted_child).rotation, Vec3::new(0.0, 45.0, 0.0));
        assert_eq!(scene.node(pasted_child).position, Vec3::new(7.0, 0.0, 0.0));
    }

    #[test]
    fn a_pattern_holds_children_and_round_trips_through_the_portable_form() {
        use crate::primitive::ParamValue;
        let mut scene = Scene::new();
        let root = scene.root();
        let pat = scene.add_pattern(root, 0);
        assert!(scene.node(pat).is_pattern());
        assert!(scene.node(pat).can_hold_children(), "a pattern must be able to hold children");
        // A child can be reparented into it, the way a group takes one.
        let child = box_at(&mut scene, root, 3.0);
        scene.reparent(child, pat, 0).unwrap();
        assert_eq!(scene.node(pat).children, vec![child]);
        scene.get_mut(pat).unwrap().params_mut().unwrap().insert("count".into(), ParamValue::Count(5));

        let data = scene.export_subtree(pat).unwrap();
        assert_eq!(data.type_id, "pattern");
        let json = serde_json::to_string(&data).unwrap();
        let back: NodeData = serde_json::from_str(&json).unwrap();
        let copy = scene.import_subtree(&back, root, 1).unwrap();
        assert!(scene.node(copy).is_pattern());
        assert_eq!(scene.node(copy).params().unwrap().get("count"), Some(&ParamValue::Count(5)));
        assert_eq!(scene.node(copy).children.len(), 1, "the repeated child was lost");
    }

    #[test]
    fn importing_an_unknown_primitive_type_leaves_no_partial_subtree() {
        let mut scene = Scene::new();
        let root = scene.root();
        let before = scene.len();
        let data = NodeData {
            name: "Group".into(),
            type_id: "group".into(),
            op: Some(GroupOp::Union),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            ghost: false,
            colour: None,
            segments: None,
            export_body: None,
            extracted: false,
            mesh: None,
            original: None,
            tiling: None,
            params: Params::new(),
            children: vec![NodeData {
                name: "From the future".into(),
                type_id: "hyperboloid".into(),
                op: None,
                position: Vec3::ZERO,
                rotation: Vec3::ZERO,
                scale: Vec3::ONE,
                anchor: Anchor::Centre,
                visible: true,
                ghost: false,
                colour: None,
                segments: None,
                export_body: None,
                extracted: false,
                mesh: None,
                original: None,
                tiling: None,
                params: Params::new(),
                children: vec![],
            }],
        };
        assert!(scene.import_subtree(&data, root, 0).is_none());
        assert_eq!(scene.len(), before);
        assert!(scene.node(root).children.is_empty());
    }

    #[test]
    fn ids_are_never_reused_after_deletion() {
        let mut scene = Scene::new();
        let root = scene.root();
        let a = box_at(&mut scene, root, 0.0);
        scene.remove(a);
        let b = box_at(&mut scene, root, 0.0);
        assert_ne!(a, b);
    }
    #[test]
    fn a_node_has_three_visibility_states_and_they_survive_the_file() {
        // Issue 21: hidden used to mean "invisible, unless the document-wide
        // ghost switch is on, in which case it means translucent" -- so the only
        // states the interface could reach were visible and ghost, and there was
        // no way to say "this one is gone" while another was being positioned.
        let mut scene = Scene::new();
        let root = scene.root();
        let a = scene.add_primitive("plate", root, 0).unwrap();
        assert_eq!(scene.node(a).visibility(), Visibility::Visible);

        for state in Visibility::ALL {
            scene.get_mut(a).unwrap().set_visibility(state);
            assert_eq!(scene.node(a).visibility(), state);
            // Anything but visible is out of the model, ghost included: a ghost
            // is drawn, never built.
            assert_eq!(scene.node(a).visible, state == Visibility::Visible);

            let data = scene.export_subtree(a).unwrap();
            let mut other = Scene::new();
            let other_root = other.root();
            let copy = other.import_subtree(&data, other_root, 0).unwrap();
            assert_eq!(other.node(copy).visibility(), state, "{state:?} did not survive the round trip");
        }
    }
}
