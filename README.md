# Simple 3D

Parametric 3D modelling with exact metric dimensions. Assemble models out of
primitives (boxes rounded or chamfered, prisms, spheres, cylinders, cones,
pyramids, tori, slots, regular polyhedra), combine them with booleans, and
export to 3MF, STL, OBJ or PLY for a slicer.

Nothing has to be entered as a scale factor: a 40 mm box is 40 mm because its
width parameter says 40, and dragging its right face rewrites that parameter
rather than stretching anything. Every dimension typed in is reproduced exactly
in the exported mesh. A scale tool exists alongside that, for the question
dimensions cannot answer — making a whole *group* a proportion of what it was —
and it is a factor the node carries, never something a resize writes.

One self-contained binary. No runtime, no installer, no network, no accelerated
graphics required.

## Download

Portable single-file executables for Linux (x86-64) and Windows (x64) are built
from the tag by `.github/workflows/release.yml` and attached to each release:

| Platform | File |
| --- | --- |
| Linux x86-64 | `simple-3d-linux-x86_64` (`chmod +x` and run) |
| Linux x86-64, Debian/Ubuntu | `simple-3d_<version>_amd64.deb` (`sudo apt install ./simple-3d_<version>_amd64.deb`) |
| Windows x64 | `simple-3d-windows-x86_64.exe` |

The `.deb` is the same executable, packaged: it adds only the desktop entry,
icon and MIME type that put Simple 3D in the applications menu and let a file
manager open `.simple3d` files with it. It is built by `packaging/deb/build.sh`
from the binary the same job produced.

Put a file named `portable` (or `portable.txt`) beside the executable and it
keeps its settings there instead of in the user profile — otherwise they live in
`%APPDATA%\Simple3D` or `$XDG_CONFIG_HOME/simple3d`.

Projects are `.simple3d` files (JSON, versioned, human-readable). Passing one
as an argument opens it, so file associations work on both platforms. The
`.deb` registers the type on Linux, icon included; on Windows, Help ▸
Associate .simple3d files writes the association for the current user, giving
those files the application's own icon.

## What it does

- **Typing is the primary interface.** Every numeric field takes an expression
  (`40/3`, `(2+3)*4`), a value in another unit (`4cm` in a millimetre document),
  or a delta (`+2`, `- 5`) that resolves against each selected shape separately.
  Dragging a field's label scrubs its value; Shift is fine, Ctrl is coarse.
- **Direct manipulation writes parameters.** Move, rotate and resize handles
  rewrite the shape's own dimensions and position — a completed drag is one undo
  step, and Escape during one puts everything back. The fourth tool, scale,
  writes a factor instead, and is the one that works on a group.
- **Paint what you are working on.** An object or a whole group takes a colour,
  and it follows each surface through a boolean: after a painted cutter drills a
  painted plate, the wall of the hole is the cutter's colour. 3MF export carries
  the colours; the other formats have nowhere to put them.
- **A palette you can add to.** Save a group, or a whole project, as a primitive
  and it is on the palette of every project afterwards. New shapes land where you
  choose: the origin, the 3D cursor, what the camera is looking at, or clear of
  the selection.
- **Booleans that hold up.** Union, difference, intersection and hull, nested
  arbitrarily. Results are checked for manifoldness on every evaluation; a
  boolean that cannot be evaluated names its own node in the outliner while the
  rest of the scene still previews, and export refuses while the error stands.
- **A window you can rearrange.** Panels move between the two docks by dragging
  their header and roll up by clicking it; Tab hides both docks and View ▸ Reset
  panel layout puts them back. The arrangement survives a restart. The
  orientation cube turns the camera to a face, and its centre dot switches
  perspective and orthographic.
- **Keys and mouse buttons are yours.** Three presets (Simple 3D default, mesh
  editor, CAD) and per-command rebinding, with conflicts named rather than
  silently taken. A rebinding applies to the next gesture, without a restart.
- **It starts on anything.** The viewport is a from-scratch software rasterizer:
  there is no shader to fail to compile and no GPU to be missing.

## Status

**v0.0.2, plus unreleased work.** All four crates are implemented and all 29 of
the spec's acceptance criteria are behaviourally met. What each release changed
is on its release page; the commit log is the record between them.

`cargo test --workspace` runs **383 tests**, none ignored or failing, and every
one of the 29 criteria is asserted by a test that cites it by name. Check that
last claim rather than trusting it:

```
python3 tools/criteria_audit.py
```

It prints the test covering each criterion and exits non-zero if any is
uncovered. Do not use `grep -rn "criterion" crates/` for this: it counts a doc
comment as coverage, and four criteria were once passing that check without a
test.

A passing suite is not the same as a working application. Several of the bugs
fixed so far — a drag that finished wherever the mouse button came up, an
orientation cube a quarter turn out of step with the viewport, a side wall the
orthographic camera culled although it faced the viewer — were all found by
running the application with the full suite passing over it. Each of them is now
covered by a test that fails without its fix.

## Why Rust, and why no external CSG/geometry crate

- Hard constraint: single self-contained native binary, no runtime/interpreter
  on the user's machine, <2 s cold start, works fully offline. A compiled Rust
  binary satisfies this directly.
- The obvious existing CSG crate (`csgrs`) currently fails to build from
  crates.io at *any* published version (0.18–0.20.1) because it has a
  mandatory, non-optional dependency on `core2`, and **every version of
  `core2` has been yanked from crates.io**. Rather than depend on a git fork
  (which would break reproducible builds), `crates/simple3d-geom` has its own
  small, from-scratch BSP boolean CSG kernel and a QuickHull-style convex hull
  implementation.
- The viewport is a from-scratch software rasterizer, so the app starts and
  stays usable on a machine with no accelerated graphics (acceptance criterion
  19). `eframe` only has to provide a window and 2D drawing.

## Workspace layout

```
crates/
  simple3d-geom/     Vec3, Mesh, primitive generators, BSP CSG boolean
                       kernel, convex hull, post-boolean mesh repair and
                       flat-region retriangulation. Pure math; depends on
                       none of the others.
  simple3d-core/     Domain model: Node/Scene tree, the declarative
                       primitive parameter registry, scene evaluation with
                       per-subtree caching and cancellation, undo, clipboard,
                       units, project files, settings, keymaps.
  simple3d-export/   STL (binary and ASCII), OBJ, PLY and 3MF writers, with
                       pre-write watertightness verification, progress
                       reporting and cancellation. Includes a minimal zip
                       writer for the 3MF container.
  simple3d-app/      eframe/egui desktop UI: outliner, property editor,
                       software-rasterized viewport, direct-manipulation
                       handles, docks, menus and dialogs, evaluation worker
                       thread.
packaging/
  deb/               Debian package: desktop entry, icon, MIME type and the
                       script that assembles them around the built binary.
```

Engineering highlights worth knowing about:

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
- **Evaluation is deterministic, cached per subtree and cancellable.** The
  spec's 200-primitive scene (fifty assemblies, 100 nested boolean groups)
  evaluates cold in ~0.12 s and updates after a one-dimension edit in ~6 ms.
  See `crates/simple3d-core/tests/performance.rs`.
- **Boolean output is retriangulated per flat region**, so a plate with a hole,
  a slot and a boss comes out at ~230 triangles rather than the ~1500 a
  plane-clipping BSP leaves behind. See `crates/simple3d-geom/src/planar.rs`.
- **Pointer gestures are executed by tests**, not only reasoned about:
  `crates/simple3d-app/src/gestures.rs` replays real pointer events over a
  real frame with `egui_kittest`.

## Building

```
cargo build --workspace --release
cargo test --workspace
cargo run --release -p simple3d-app          # or: target/release/simple-3d
cargo run --release -p simple3d-app -- my-project.simple3d

# What one boolean chain costs, step by step -- the first thing to run when a
# scene starts feeling slow or an export comes out unexpectedly large.
cargo run --release -p simple3d-geom --example boolean_cost
```

The toolchain is pinned by `rust-toolchain.toml`. On Linux the usual X11/Wayland
development packages are needed (`libx11-dev libxcursor-dev libxi-dev
libxrandr-dev libxkbcommon-dev libwayland-dev` on Debian and Ubuntu); the
release workflow lists the same set.

Releases are cut by pushing a `v*` tag: `.github/workflows/release.yml` takes the
version from the tag, tests and builds both targets from that one commit, and
attaches the two executables and the Debian package to the GitHub release. The
package can be built by hand from any release binary:

```bash
packaging/deb/build.sh target/release/simple-3d 0.1.0 dist
```

## Where things live

| If you are looking for | Start at |
|---|---|
| A new primitive type | `simple3d-core/src/primitive.rs` — one declaration drives the Add menu, the property editor and the project file |
| Boolean semantics and operand ordering | `simple3d-geom/src/lib.rs` (`evaluate_boolean`) |
| Why a boolean result is the shape it is | `simple3d-geom/src/csg_bsp.rs`, then `repair.rs` and `planar.rs` |
| Caching and invalidation | `simple3d-core/src/eval.rs` (`subtree_key`) |
| Manipulator handle behaviour and modifiers | `simple3d-app/src/gizmo.rs` |
| What a numeric field accepts | `simple3d-core/src/unit.rs` |
| The docks, and what moves between them | `simple3d-app/src/dock.rs` |
| A colour, a row height or a type size | `simple3d-app/src/theme.rs` — nothing else names one |
| An icon, or a primitive's silhouette | `simple3d-app/src/icon.rs` |
| Keymap presets, rebinding and conflicts | `simple3d-core/src/keymap.rs` |
| Navigation bindings taking effect without a restart | `simple3d-app/src/panel_viewport.rs` (`nav_gesture`) |
| Driving a pointer gesture in a test | `simple3d-app/src/gestures.rs` |
| Whether a criterion is really covered | `tools/criteria_audit.py` |
| What the Debian package installs, and what it depends on | `packaging/deb/build.sh` |
| File format and migration | `simple3d-core/src/project.rs` |

## Licence

MIT. See `LICENSE`.
