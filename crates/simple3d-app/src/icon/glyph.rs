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
    /// Squares spread along a curve: the align and distribute tool (issue 70).
    Arrange,
    Pattern,
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
    Bracket,
    Warning,
    /// Geometry a node owns outright, not a primitive (issue 80).
    Mesh,
    /// A shape cut into smaller pieces (issue 82).
    Split,
    /// A component placed in the tree (issue 113).
    Component,
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
            _ => Glyph::Box,
        }
    }
}
