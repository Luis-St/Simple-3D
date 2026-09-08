//! Which icon is which.

/// Every glyph the interface draws. Ordered by where it is used: tools first,
/// then outliner marks, then the primitive silhouettes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    // Tools.
    Move,
    Rotate,
    Resize,
    Scale,
    Frame,
    Measure,
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
    /// The body that is not a primitive: geometry a node owns outright
    /// (issue 80).
    Mesh,
    /// A shape cut into smaller pieces (issue 82).
    Split,
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
    /// The silhouette for a primitive type, by its registry id. An unknown id
    /// falls back to the box rather than drawing nothing, so a primitive added
    /// to the registry without a glyph still has a tile that can be clicked.
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
