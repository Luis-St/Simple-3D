# ScadStudio

A desktop app for assembling 3D models out of parametric primitives (boxes,
prisms, spheres, cylinders, cones, pyramids, tori, regular polyhedra) with
boolean operations and exact metric dimensions, exporting to 3MF/STL/OBJ/PLY.
Full spec: `scadstudio-prompt.md`, checked in beside this file.

Nothing is entered as a scale factor and nothing is stored as one: a 40 mm box
is 40 mm because its width parameter says 40, and dragging its right face
rewrites that parameter rather than stretching anything.

## Why Rust, and why no external CSG/geometry crate

- Hard constraint: single self-contained native binary, no runtime/interpreter
  on the user's machine, <2 s cold start, works fully offline. A compiled Rust
  binary satisfies this directly.
- The obvious existing CSG crate (`csgrs`) currently fails to build from
  crates.io at *any* published version (0.18–0.20.1) because it has a
  mandatory, non-optional dependency on `core2`, and **every version of
  `core2` has been yanked from crates.io**. Rather than depend on a git fork
  (which would break reproducible builds), `crates/scadstudio-geom` has its own
  small, from-scratch BSP boolean CSG kernel and a QuickHull-style convex hull
  implementation.
- The viewport is a from-scratch software rasterizer, so the app starts and
  stays usable on a machine with no accelerated graphics (acceptance criterion
  19). `eframe` only has to provide a window and 2D drawing; there is no shader
  to fail to compile.

## Workspace layout

```
crates/
  scadstudio-geom/     Vec3, Mesh, primitive generators, BSP CSG boolean
                       kernel, convex hull, post-boolean mesh repair and
                       flat-region retriangulation. Pure math; depends on
                       none of the others.
  scadstudio-core/     Domain model: Node/Scene tree, the declarative
                       primitive parameter registry, scene evaluation with
                       per-subtree caching and cancellation, undo, clipboard,
                       units, project files, settings, keymaps.
  scadstudio-export/   STL (binary and ASCII), OBJ, PLY and 3MF writers, with
                       pre-write watertightness verification, progress
                       reporting and cancellation. Includes a minimal zip
                       writer for the 3MF container.
  scadstudio-app/      eframe/egui desktop UI: outliner, property editor,
                       software-rasterized viewport, direct-manipulation
                       handles, menus and dialogs, evaluation worker thread.
```

## Status

All four crates are implemented and all 29 of the spec's acceptance criteria are
behaviourally met. `cargo test --workspace` runs 272 tests, none ignored or
failing, and every one of the 29 criteria is asserted by a test that cites the
criterion by name.

Check that last claim rather than trusting it — `python3 tools/criteria_audit.py`
prints the test covering each criterion and exits non-zero if any is uncovered.
Do not use `grep -rn "criterion" crates/` for this: it counts a doc comment as
coverage, and four criteria were passing that check without a test.

Two items are open in `KNOWN_ISSUES.md`, which is this project's issue list:
a manipulator drag is not asserted to undo in one step, and `App::new` reads the
user's real config directory. Both are recorded there with what makes them hard.

Highlights worth knowing about:

- **Every primitive from the spec's table**: box, rounded box, wedge, regular
  prism, sphere/ellipsoid, spherical cap, cylinder, tube, capsule, torus (full
  and arc), cone, pyramid, regular pyramid, and all four regular polyhedra
  (tetrahedron/octahedron/icosahedron by hand, dodecahedron built as the
  icosahedron's dual). Plate/disc/ring are thin aliases onto box/cylinder/tube
  per the spec's "must produce identical geometry" requirement. Measured
  against the circumscribed convention and exact on flat axes.
- **Booleans are watertight and manifold** for the degenerate cases that are
  the normal case in practice — coplanar faces, coincident surfaces, operands
  touching at a single edge, fully contained and fully disjoint operands.
  `Mesh::manifold_issue` gates every evaluation result; a boolean that cannot
  be evaluated names its own node in the outliner while the rest of the scene
  still previews, and export refuses while the error stands.
- **Evaluation is deterministic, cached per subtree and cancellable.** The
  spec's 200-primitive scene (fifty assemblies, 100 nested boolean groups)
  evaluates cold in ~0.12 s and updates after a one-dimension edit in ~6 ms.
  See `crates/scadstudio-core/tests/performance.rs`.
- **Boolean output is retriangulated per flat region**, so a plate with a hole,
  a slot and a boss comes out at ~230 triangles rather than the ~1500 a
  plane-clipping BSP leaves behind. See `crates/scadstudio-geom/src/planar.rs`.

## Building

```
cargo build --workspace --release
cargo test --workspace
cargo run --release -p scadstudio-app          # or: target/release/scadstudio
cargo run --release -p scadstudio-app -- my-project.scadstudio

# What one boolean chain costs, step by step -- the first thing to run when a
# scene starts feeling slow or an export comes out unexpectedly large.
cargo run --release -p scadstudio-geom --example boolean_cost
```

The toolchain is pinned by `rust-toolchain.toml`. `.github/workflows/release.yml`
builds portable single-file executables for Linux and Windows from one commit,
with the version taken from the tag.

## Where things live

| If you are looking for | Start at |
|---|---|
| A new primitive type | `scadstudio-core/src/primitive.rs` — one declaration drives the Add menu, the property editor and the project file |
| Boolean semantics and operand ordering | `scadstudio-geom/src/lib.rs` (`evaluate_boolean`) |
| Why a boolean result is the shape it is | `scadstudio-geom/src/csg_bsp.rs`, then `repair.rs` and `planar.rs` |
| Caching and invalidation | `scadstudio-core/src/eval.rs` (`subtree_key`) |
| Manipulator handle behaviour and modifiers | `scadstudio-app/src/gizmo.rs` |
| Keymap presets, rebinding and conflicts | `scadstudio-core/src/keymap.rs` |
| Navigation bindings taking effect without a restart | `scadstudio-app/src/panel_viewport.rs` (`nav_gesture`) |
| Whether a criterion is really covered | `tools/criteria_audit.py` |
| File format and migration | `scadstudio-core/src/project.rs` |
