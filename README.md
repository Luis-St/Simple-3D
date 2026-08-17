# ScadStudio

A desktop app for assembling 3D models out of parametric primitives (boxes,
prisms, spheres, cylinders, cones, pyramids, tori, regular polyhedra) with
boolean operations and exact metric dimensions, exporting to 3MF/STL/OBJ/PLY.
Full spec: see the original project prompt (not checked into this repo).

This is a from-scratch, in-progress implementation. **It does not build a
runnable application yet** — see "Status" below.

## Why Rust, and why no external CSG/geometry crate

- Hard constraint: single self-contained native binary, no runtime/interpreter
  on the user's machine, <2s cold start, works fully offline. A compiled
  Rust binary satisfies this directly.
- The obvious existing CSG crate (`csgrs`) currently fails to build from
  crates.io at *any* published version (0.18–0.20.1) because it has a
  mandatory, non-optional dependency on `core2`, and **every version of
  `core2` has been yanked from crates.io**. Rather than depend on a git fork
  (which would break reproducible builds), `crates/scadstudio-geom` has its
  own small, from-scratch BSP boolean CSG kernel and a QuickHull-style
  convex hull implementation. See `KNOWN_ISSUES.md` — this kernel has an
  open, actively-debugged correctness bug in `subtract`/`intersect`.

## Workspace layout

```
crates/
  scadstudio-geom/    Vec3, Mesh, primitive geometry generators, BSP CSG
                       boolean kernel, convex hull. No dependency on the
                       other crates; pure math + mesh generation.
  scadstudio-core/     (stub) domain model: Node/Scene tree, primitive
                       parameter registry, undo, project save/load. Not
                       started yet.
  scadstudio-export/   (stub) STL/OBJ/PLY/3MF writers. Not started yet.
  scadstudio-app/      (stub) eframe/egui desktop UI: outliner, property
                       editor, viewport, manipulators. Not started yet.
```

## Status

### Done and tested (`crates/scadstudio-geom`)

- Core math (`vec3.rs`, `mesh.rs`): `Vec3`, indexed triangle `Mesh`,
  transform/anchor helpers, vertex welding, a manifoldness checker.
- Every primitive generator from the spec's primitive table: box, rounded
  box, wedge, regular prism, sphere/ellipsoid, spherical cap, cylinder, tube,
  capsule, torus (full and arc), cone, pyramid, regular pyramid, and all four
  regular polyhedra (tetrahedron/octahedron/icosahedron by hand, dodecahedron
  built as the icosahedron's dual). Plate/disc/ring are thin aliases onto
  box/cylinder/tube per the spec's "must produce identical geometry"
  requirement. All measured against the circumscribed convention and exact
  on flat axes (see `src/tests.rs`).
- Convex hull (`hull.rs`) via randomized incremental construction.
- A hand-written BSP boolean kernel (`csg_bsp.rs`) with a coplanar-triangle
  merge pass feeding it, to avoid T-vertices between independently
  triangulated faces. **Known bug**: `subtract`/`intersect` can produce
  non-manifold output on ordinary geometry (not just pathological/degenerate
  cases) — see `KNOWN_ISSUES.md` for the full writeup, a reliable repro, and
  what's already been ruled out. `union` and `hull` are solid in testing.

Run `cargo test -p scadstudio-geom` — 22/24 tests pass; the 2 failures are
the known CSG bug above, not primitive-generation bugs.

### Not started

Everything in `scadstudio-core`, `scadstudio-export`, and `scadstudio-app` is
an empty stub crate (compiles, does nothing) — the domain model (node tree,
groups, undo, save/load), mesh exporters, and the entire UI (viewport,
outliner, property editor, manipulators, keymap system) described in the
spec still need to be built. None of the 29 acceptance criteria in the
original spec are met yet; this covers only the geometry-generation half of
section 5.

## Suggested order of work from here

1. Fix the CSG kernel bug (`KNOWN_ISSUES.md`) — everything downstream
   (boolean groups, most of the acceptance criteria) depends on booleans
   actually being correct, not just "usually correct."
2. `scadstudio-core`: node/scene tree, the declarative primitive parameter
   system (so the Add menu / property editor / project file can all derive
   from one declaration per primitive, per the spec's extensibility
   requirement), scene evaluation with per-subtree caching, undo stack.
3. `scadstudio-export`: STL (binary) and OBJ first (simplest), then PLY and
   3MF (which needs a zip writer + minimal XML — the `zip` crate is a
   reasonable pure-Rust dependency for that).
4. `scadstudio-app`: start with a static viewport + outliner + property
   panel wired to a hardcoded scene, then add editing, then manipulators,
   then the keymap system last (it's the most self-contained piece).

## Building

```
cargo build --workspace
cargo test -p scadstudio-geom
cargo run -p scadstudio-geom --example csg_bug_repro   # see the known bug
```
