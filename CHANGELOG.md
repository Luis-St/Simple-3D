# Changelog

Released versions, and what a user gets from each. Engineering write-ups of the
work behind a release live in `CHANGES.md` (most recent pass) and in git history;
bugs, fixed and open, live in `KNOWN_ISSUES.md`.

Versions follow [semantic versioning](https://semver.org). Until 1.0 the project
file format may still change; when it does, a file written by an older version
will still open, and `format` in the file says which version wrote it.

## [0.0.2] — 2026-08-18

### Fixed

- **The application icon showed as an empty tile** in the GNOME applications
  menu after installing the Debian package. GNOME loads menu icons through
  gdk-pixbuf, which recognises an SVG by sniffing the start of the file, and the
  comment the icon opened with pushed the `<svg>` tag past what is sniffed. The
  packaging script now refuses to build a package whose icon has the same fault.

## [0.0.1] — 2026-08-18

- **A Debian package** (`simple-3d_<version>_amd64.deb`) is built alongside the
  Linux executable and attached to each release. It installs the same binary,
  and adds the desktop entry, icon and MIME type that put Simple 3D in the
  applications menu and let a file manager open `.simple3d` files with it. The
  portable executable is unchanged and remains the plain way to run it.

## [0.0.0] — 2026-08-18

First release. Portable single-file executables for Linux (x86-64) and Windows
(x64): no runtime, no installer, no network access, and no accelerated graphics
required.

### Modelling

- **Primitives**, each defined by named dimensions rather than a scale: box,
  rounded box, wedge, regular prism, sphere and ellipsoid, spherical cap,
  cylinder, tube, capsule, torus (full and arc), cone, pyramid, regular pyramid,
  and the four regular polyhedra. Plate, disc and ring are aliases that produce
  identical geometry to box, cylinder and tube.
- **Booleans**: union, difference, intersection and convex hull, nested to any
  depth, with the first child of a difference as its base. Every result is
  checked for manifoldness; a group that cannot be evaluated names itself in the
  outliner while the rest of the scene still previews, and export refuses while
  the error stands.
- **A named tree**: group, ungroup, reorder, reparent, duplicate, cut, copy and
  paste, hide and show. Hidden nodes are excluded from evaluation and export, and
  can be shown as ghosts while a tool body is positioned.
- **Exact dimensions**, stored in millimetres. The display unit (mm, cm, m)
  changes only what fields read, never the model.

### Editing

- **Numeric fields take arithmetic** (`40/3`, `(2+3)*4`), **a value in another
  unit** (`4cm`), and **a delta** (`+2`, `- 5`) that resolves against each
  selected shape separately. A value that cannot be read is refused: the typed
  text stays put, the field is marked, and the model is untouched.
- **Dragging a field's label scrubs its value** — Shift for fine, Ctrl for
  coarse — and the whole drag is one undo step.
- **Direct manipulation** of move, rotate and resize handles, which rewrite the
  shape's own parameters. Escape cancels a drag in progress; a completed drag is
  a single undo step. Arrow keys nudge by the snap.
- **Multi-selection editing**: a field shows the shared value or an em dash, and
  typing over the dash applies to everything selected.
- **Undo and redo** to a depth of 200 steps, each named after what it did, with
  rapid edits to one field coalescing into a single step.

### The window

- Two docks whose panels move by dragging their header and roll up by clicking
  it; Tab hides both; View ▸ Reset panel layout puts them back. The arrangement
  survives a restart.
- An orientation cube that turns the camera to a face over 200 ms and switches
  perspective and orthographic from its centre dot.
- A 3D cursor placed with Shift+right-click; new shapes land there.
- A status bar carrying the selection, its size, the snap, the unit, the last
  message and what the evaluation cost.
- Three keymap presets (Simple 3D default, mesh editor, CAD) plus per-command
  rebinding, with conflicts named rather than silently taken. Rebinding takes
  effect immediately, without a restart.
- Reduced-motion and ghost-display preferences, a scene settings window, and an
  About window that reports where settings are stored.

### Files

- **Projects** are `.simple3d`: versioned, human-readable JSON that round trips
  exactly. A truncated or damaged file reports where it stopped rather than
  failing generically. Passing a path as an argument opens it.
- **Export** to 3MF (with units recorded), STL (binary and ASCII), OBJ and PLY,
  with an optional export-time scale that never touches the model, a
  selection-only option, progress and cancellation, and a watertightness check
  before anything is written.
- **Settings and keymaps** live in `%APPDATA%\Simple3D` or
  `$XDG_CONFIG_HOME/simple3d`, or beside the executable in portable mode.

### Performance

- The specification's 200-primitive scene (fifty assemblies inside 100 nested
  boolean groups) evaluates cold in ~0.12 s and updates after a single dimension
  edit in ~6 ms. Evaluation runs on a worker thread, is cached per subtree, is
  cancellable, and is deterministic.

### Verified

- 328 tests across the workspace, none ignored or failing.
- All 29 acceptance criteria of the functional specification asserted by a test that
  cites the criterion by name, checked by `tools/criteria_audit.py`.
- Pointer gestures — the header drag, the label scrub, the cube click, the cursor
  placement — replayed against a real frame by `egui_kittest`.
