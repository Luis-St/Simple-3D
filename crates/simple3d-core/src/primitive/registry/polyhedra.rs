//! The five regular solids.

use super::super::*;
use simple3d_geom::primitives as gen;

pub(super) const TETRAHEDRON: PrimitiveSpec = PrimitiveSpec {
    type_id: "tetrahedron",
    label: "Tetrahedron",
    category: POLY,
    params: &[
        ParamSpec::positive_length("size", "Size", 20.0),
        ParamSpec::choice("size_mode", "Size measured as", SIZE_MODE),
    ],
    segmented: false,
    build: |p, _seg| gen::tetrahedron_mesh(p.num("size"), p.int("size_mode") == 1),
    axes: |_p| [None, None, None],
};

pub(super) const OCTAHEDRON: PrimitiveSpec = PrimitiveSpec {
    type_id: "octahedron",
    label: "Octahedron",
    category: POLY,
    params: &[
        ParamSpec::positive_length("size", "Size", 20.0),
        ParamSpec::choice("size_mode", "Size measured as", SIZE_MODE),
    ],
    segmented: false,
    build: |p, _seg| gen::octahedron_mesh(p.num("size"), p.int("size_mode") == 1),
    axes: |_p| [None, None, None],
};

pub(super) const DODECAHEDRON: PrimitiveSpec = PrimitiveSpec {
    type_id: "dodecahedron",
    label: "Dodecahedron",
    category: POLY,
    params: &[
        ParamSpec::positive_length("size", "Size", 20.0),
        ParamSpec::choice("size_mode", "Size measured as", SIZE_MODE),
    ],
    segmented: false,
    build: |p, _seg| gen::dodecahedron_mesh(p.num("size"), p.int("size_mode") == 1),
    axes: |_p| [None, None, None],
};

pub(super) const ICOSAHEDRON: PrimitiveSpec = PrimitiveSpec {
    type_id: "icosahedron",
    label: "Icosahedron",
    category: POLY,
    params: &[
        ParamSpec::positive_length("size", "Size", 20.0),
        ParamSpec::choice("size_mode", "Size measured as", SIZE_MODE),
    ],
    segmented: false,
    build: |p, _seg| gen::icosahedron_mesh(p.num("size"), p.int("size_mode") == 1),
    axes: |_p| [None, None, None],
};
