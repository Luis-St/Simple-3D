//! Which icon is which.

/// Every glyph the interface draws: tools, then outliner marks, then primitive silhouettes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    // Tools.
    Move,
    Rotate,
    Resize,
    Scale,
    Measure,
    /// A face pulled out of a slab: the push/pull tool (issue 73).
    PushPull,
    /// A square with one corner rounded off: the edge rounding tool (issue 88).
    Round,
    /// Squares spread along a curve: the align and distribute tool (issue 70).
    Arrange,
    /// A shape and its mirror image across a dashed line: the mirror tool (issue 122).
    Mirror,
    Pattern,
    /// An outline of a line, an arc and corner points on a grid: the sketch tool (issue 74).
    Sketch,
    /// A pencil: change what a list entry holds, such as a workplane (issue 125).
    Edit,
    // Sketch tools (issue 74), drawn as what each puts down, with its clicked points as dots.
    SketchLine,
    SketchRectangle,
    SketchCentreRectangle,
    SketchCircle,
    SketchThreePointCircle,
    SketchArc,
    SketchThreePointArc,
    SketchPolygon,
    SketchSpline,
    SketchControlSpline,
    SketchPoint,
    /// The pointer: select and drag what is drawn.
    SketchSelect,
    SketchTrim,
    SketchFillet,
    SketchDimension,
    /// A closed area filled in: pick the regions a solid is made from.
    SketchRegions,
    SketchAxis,
    SketchCopyEdges,
    // Object actions.
    Group,
    Delete,
    // Booleans.
    Union,
    Difference,
    Intersection,
    // View.
    Shaded,
    ShadedEdges,
    Wireframe,
    Grid,
    /// The section plane cutting the model (issue 71).
    Section,
    // Outliner marks.
    Eye,
    EyeOff,
    /// A hole: the negative part toggle (issue 119).
    Negative,
    Bracket,
    Warning,
    /// Geometry a node owns outright, not a primitive (issue 80).
    Mesh,
    /// A shape cut into smaller pieces (issue 82).
    Split,
    /// A component placed in the tree (issue 113).
    Component,
    /// An outline swept into a solid by push/pull (issue 73).
    Extrusion,
    /// A sketch and the solid made from it (issue 74).
    SketchSolid,
    // Primitive silhouettes.
    Box,
    RoundedBox,
    ChamferedBox,
    Wedge,
    Prism,
    Sphere,
    Cap,
    Cylinder,
    Tube,
    Capsule,
    Torus,
    Cone,
    Pyramid,
    Polyhedron,
    Plate,
    Disc,
    Ring,
    Slot,
    HexNut,
    HexStandoff,
    ScrewHole,
    ThreadedRod,
    Gear,
    /// A ring with teeth on its inside.
    InternalGear,
    /// A straight bar of teeth.
    Rack,
    /// A cone of teeth seen from the side.
    BevelGear,
    /// A screw with deep, broad threads.
    Worm,
    /// A wheel with round seats between its teeth.
    Sprocket,
    Spring,
}

impl Glyph {
    /// A primitive's silhouette by registry id; unknown ids fall back to the box so the tile is clickable.
    pub fn for_primitive(type_id: &str) -> Glyph {
        match type_id {
            "box" => Glyph::Box,
            "rounded_box" => Glyph::RoundedBox,
            "chamfered_box" => Glyph::ChamferedBox,
            "wedge" => Glyph::Wedge,
            "prism" => Glyph::Prism,
            "sphere" => Glyph::Sphere,
            "spherical_cap" => Glyph::Cap,
            "cylinder" => Glyph::Cylinder,
            "tube" => Glyph::Tube,
            "capsule" => Glyph::Capsule,
            "torus" => Glyph::Torus,
            "cone" => Glyph::Cone,
            "pyramid" | "regular_pyramid" => Glyph::Pyramid,
            "tetrahedron" | "octahedron" | "dodecahedron" | "icosahedron" => Glyph::Polyhedron,
            "plate" => Glyph::Plate,
            "disc" => Glyph::Disc,
            "ring" => Glyph::Ring,
            "slot" => Glyph::Slot,
            "hex_nut" => Glyph::HexNut,
            "hex_standoff" => Glyph::HexStandoff,
            "screw_hole" => Glyph::ScrewHole,
            "threaded_rod" => Glyph::ThreadedRod,
            "spur_gear" => Glyph::Gear,
            "internal_gear" => Glyph::InternalGear,
            "gear_rack" => Glyph::Rack,
            "bevel_gear" => Glyph::BevelGear,
            "worm" => Glyph::Worm,
            "sprocket" => Glyph::Sprocket,
            "spring" => Glyph::Spring,
            _ => Glyph::Box,
        }
    }
}
