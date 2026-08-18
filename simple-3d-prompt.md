# Simple 3D — Project Prompt

## 0. How to read this document

This is a complete specification for a desktop application. It deliberately
names **no programming language, no UI toolkit, no libraries and no external
programs** — those are free choices for whoever implements it, subject only to
the constraints in section 2. Everything else described here is required
behaviour.

The application is **fully self-contained**. It computes its own geometry,
renders its own preview and writes its own export files. It does not shell out
to, bundle, or require any other application to be installed.

---

## 1. Goal

Build a desktop application that lets a user assemble 3D models out of
parametric primitives — boxes, prisms, spheres, cylinders, cones, pyramids,
tori, regular polyhedra — by typing exact metric dimensions and organising
them in a named tree with boolean operations, then export the result as a mesh
file for slicers and other 3D tools.

The target user finds full CAD packages overwrought and mesh modellers
imprecise. They want to say "a 40 × 20 × 4 mm plate with a 6 mm hole 12 mm
from the left edge" and get exactly that, in a program that opens instantly
and has no learning curve beyond typing numbers into labelled fields.

Every dimension the user enters must be reproduced exactly in the exported
mesh. Precision of stated dimensions is the product's single most important
property; everything else is negotiable.

---

## 2. Hard constraints

1. **Deliverable is a working binary for Linux (x86-64) and Windows (x64).**
   The end user downloads one artefact and runs it. No runtime, interpreter,
   package manager, dependency installation, companion application or build
   step on the user's machine.
2. **No administrator or root rights** required to run. The application must
   work when unpacked into a user-writable directory (portable mode).
3. **No network access** at any point. Fully offline, no update check, no
   telemetry.
4. Both builds must be producible from a single source tree by an automated,
   reproducible build. Cross-building or a two-platform build matrix are both
   acceptable; a manual per-platform ritual is not.
5. Cold start to an interactive window in under two seconds on ordinary
   hardware.
6. The user interface must never freeze. Geometry evaluation and export run
   off the interaction path, with progress shown and a working cancel.
7. Hardware requirements stay modest: the preview must work on integrated
   graphics, and the application must degrade gracefully rather than refuse to
   start if accelerated rendering is unavailable.

---

## 3. Domain model

### 3.1 Nodes

The scene is a tree of **nodes**. Every node, of any type, has:

| Property | Notes |
|---|---|
| Identity | Stable, unique, generated on creation, preserved across save/load |
| Name | User-editable free text, non-unique, defaults to the type name |
| Position | Three signed lengths (X, Y, Z), default 0 |
| Rotation | Three angles in degrees (X, Y, Z), applied in that order, default 0 |
| Anchor | Where the node's origin sits: **centre** (default) or **base** (minimum Z at the origin) |
| Visibility | Hidden nodes are excluded from evaluation and export entirely |
| Parent | Every node except the root has exactly one parent group |

The anchor exists because both conventions are constantly useful: centre for
symmetric placement, base for standing things on a build plate. Changing the
anchor must not change the shape, only where its origin sits.

Scaling is deliberately **not** a node property. Dimensions are edited as
dimensions; a scale factor would make the numbers in the property editor stop
meaning what they say. The resize handles described in section 6.2 therefore
write to a primitive's dimension parameters directly — they are not a scale
transform, and after a drag the property editor shows the new real
dimensions.

### 3.2 Primitives

Primitives are leaf nodes. Rules that hold for every one of them:

- The stated dimensions are **outer bounding dimensions in the object's own
  frame**, before rotation. A box declared 40 × 20 × 4 has a bounding box of
  exactly 40 × 20 × 4. A sphere of diameter 50 measures exactly 50 across.
- Where a shape has a natural axis, that axis is **Z**.
- Curved surfaces are approximated by a per-object **segment count** that
  overrides the scene default (section 5.1).
- Every primitive is **closed and watertight** on its own.

#### Boxes and prisms

| Primitive | Parameters |
|---|---|
| Box | Width (X), Depth (Y), Height (Z) |
| Rounded box | Width, Depth, Height, Corner radius, Corner segments |
| Wedge | Width (X), Depth (Y), Height (Z), Top width (0 gives a sharp ridge) |
| Regular prism | Number of sides (3–128), Diameter, Height, Diameter measured *across corners* or *across flats* (selectable) |

The across-corners / across-flats choice is not a detail: it is the difference
between a hexagonal prism that fits a 10 mm spanner and one that does not.

#### Round solids

| Primitive | Parameters |
|---|---|
| Sphere | Diameter X, Diameter Y, Diameter Z (equal by default, with a lock toggle; unequal gives an ellipsoid) |
| Spherical cap | Diameter, Cap height (half the diameter gives a hemisphere) |
| Cylinder | Diameter X, Diameter Y (locked equal by default; unequal gives an elliptical cylinder), Height |
| Tube | Outer diameter, Wall thickness *or* Inner diameter (selectable), Height |
| Capsule | Diameter, Total length including both caps |
| Torus | Ring diameter (centre-line), Tube diameter, Sweep angle (360° by default; less gives an arc) |

#### Cones and pyramids

| Primitive | Parameters |
|---|---|
| Cone | Bottom diameter, Top diameter (0 gives a true cone, non-zero a frustum), Height |
| Pyramid | Base width, Base depth, Top width, Top depth (both tops 0 give an apex, non-zero a frustum), Height |
| Regular pyramid | Number of sides (3–128), Base diameter, Top diameter, Height, measured across corners or flats |

#### Regular polyhedra

| Primitive | Parameters |
|---|---|
| Tetrahedron | Size, measured as circumscribed-sphere diameter *or* edge length (selectable) |
| Octahedron | as above |
| Dodecahedron | as above |
| Icosahedron | as above |

A cube is a box with three equal sides and needs no separate type.

#### Flat shapes

| Primitive | Parameters |
|---|---|
| Plate | Width, Depth, Thickness — a box, offered separately because it is the most common starting shape |
| Disc | Diameter X, Diameter Y, Thickness |
| Ring | Outer diameter, Wall thickness or Inner diameter, Thickness |

These are conveniences over the general forms and must produce identical
geometry to their equivalents. They exist because the alternative is the user
mentally converting "a 2 mm washer" into "a very short tube" every time.

#### Extensibility

Adding a further primitive type must require declaring its parameters and its
geometry generator, and nothing else. The Add menu, the property editor, the
project file format and undo must all derive themselves from that
declaration. **No per-primitive user-interface code.** This is a structural
requirement, not a preference: the primitive list above will grow.

### 3.3 Groups

Groups are the only branch nodes. A group applies an **operation** to its
children:

- **Union** — children merged into one solid.
- **Difference** — the first visible child is the base; every later visible
  child is subtracted from it.
- **Intersection** — only the volume common to all visible children remains.
- **Hull** — the convex hull enclosing all visible children.

Groups nest arbitrarily. A group's own position, rotation and anchor apply to
the combined result, not to each child individually. The scene root is a
group.

Because grouping and boolean operations are the same mechanism, hiding one
child of a difference group is a fast, non-destructive way to compare the cut
and uncut result. This must work, and must be instant.

Child order is semantic inside a difference group and must therefore be
user-controllable and stably preserved.

### 3.4 Scene-level settings

- Display unit (section 4).
- Default segment count for curved surfaces.
- Optional free-text notes on the scene.
- Grid spacing and visibility for the viewport.

---

## 4. Units

- All lengths are stored internally in **millimetres**.
- The user chooses a **display unit** — millimetres, centimetres or metres —
  affecting only what is shown in and read from input fields, the viewport
  grid labels and the measurement readout.
- Switching the display unit **never rescales the model**. With metres
  selected, typing `1.8` stores 1800 mm and the field afterwards reads `1.8`.
- Input fields accept both decimal separators (`1.8` and `1,8`) and reject
  anything unparseable by silently restoring the previous value rather than
  raising a dialog.
- Angles are always degrees regardless of the length unit.
- Displayed numbers must not show floating-point noise: `1.8`, never
  `1.7999999999`. Internally, precision must be sufficient that a chain of
  transforms does not visibly move a dimension.

---

## 5. Geometry

The application evaluates the scene tree into a single mesh. This is the heart
of the product and the part most likely to be got wrong.

### 5.1 Tessellation

- Curved surfaces are approximated by segments. A scene default applies to all
  objects; any object may override it.
- The default must be sensible at both extremes: a 3 mm pin and a 2 m cylinder
  should both look and export acceptably without the user touching the
  setting.
- **Preview may tessellate more coarsely than export.** The export resolution
  is what the numbers promise; the preview only has to look right.
- Tessellating a shape never changes its stated bounding dimensions: a
  cylinder of diameter 50 with 12 segments still measures 50 at its widest.
  State clearly in the interface which measurement convention this follows
  (circumscribed by default) since it determines whether a printed hole fits.

### 5.2 Boolean evaluation

- Union, difference, intersection and hull, on arbitrary nested groups.
- The result must be **watertight and manifold** for any input the user can
  construct through the interface — including coplanar faces, exactly
  coincident surfaces, shapes touching at a single edge or vertex, and
  fully-contained or fully-disjoint operands. These degenerate cases are the
  normal case in practice, not the exception: cutting a hole through a plate
  produces coincident surfaces every time.
- A boolean that cannot be evaluated must fail loudly on that node, naming it
  in the outliner, while the rest of the scene still previews. Never silently
  emit broken geometry.
- Evaluation is **deterministic**: the same tree always produces the same
  mesh.
- Results are cached per subtree and invalidated only where the tree actually
  changed, so editing one dimension does not re-evaluate the whole scene.
- Evaluation runs off the interaction path, is cancellable, and is superseded
  cleanly when the user makes a further edit while one is running.

### 5.3 Performance targets

- A scene of 200 primitives with nested booleans previews interactively.
- A single-value edit updates the preview in well under a second for typical
  scenes.
- The application must remain usable — pannable, editable, saveable — while a
  long evaluation runs.

---

## 6. Viewport

A 3D view is part of the main window, not an optional extra.

### 6.1 View and navigation

- Orbit, pan and zoom with the mouse. The default scheme must not require a
  three-button mouse, and every navigation binding is remappable (section 8).
- Perspective and orthographic projection, toggleable.
- Standard view presets: top, bottom, front, back, left, right, isometric, and
  a frame-selection / frame-all command.
- A ground grid with spacing in the display unit, plus origin axes with
  consistent, labelled colours.
- Shaded, shaded-with-edges and wireframe display modes.
- The selected node is visibly highlighted, and clicking geometry in the
  viewport selects the corresponding node in the outliner.
- An optional bounding-box overlay for the selection and for the whole scene,
  with its dimensions shown numerically — the fastest way to answer "will this
  fit".
- Optional display of hidden nodes as translucent ghosts, so a subtracted tool
  body can be seen while positioning it.
- The camera position is part of the saved project.

### 6.2 Direct manipulation

The selected node carries an on-screen manipulator, in the manner of a
lightweight mesh editor rather than a CAD package. Three modes, switchable by
single keys and from a toolbar:

- **Move** — three axis arrows plus three plane handles for two-axis dragging.
- **Rotate** — three axis rings, reading out in degrees.
- **Resize** — handles on the faces and corners of the selection's bounding
  box.

Common to all three:

- Handles use the same axis colours as the origin axes, keep a constant
  on-screen size regardless of zoom, and highlight on hover.
- A drag shows its live numeric value at the cursor, in the display unit.
- The property editor updates continuously **during** the drag, and typing in
  the property editor moves the handles. One source of truth, both directions.
- A whole drag is **one undo step**, never one per frame.
- Dragging snaps to a configurable increment, with one modifier to drag freely
  and another to snap coarsely. Defaults: grid spacing for move and resize,
  15° for rotate.
- Handles work in the object's own frame by default, with a toggle for world
  frame, and the active frame is shown.
- Escape during a drag cancels it and restores the pre-drag values exactly.

**Resize semantics.** A resize handle writes the primitive's own dimension
parameters — dragging the right face of a box changes its width. It never
introduces a scale factor (section 3.1). Dragging a face moves that face
alone, leaving the opposite one fixed; with the symmetry modifier held, the
shape grows about its centre instead. A corner handle resizes the axes it
touches, with a modifier to preserve proportions.

Where a shape has no parameter for the axis being dragged — the X extent of a
sphere with locked diameters, the height of a flat disc's diameter axis — the
handle must either drive the parameter that actually governs that extent, or
not be shown at all. It must never appear and silently do nothing, and it must
never produce a shape the property editor cannot represent.

**Groups** get move and rotate handles. Resize handles on groups are out of
scope for the first version: scaling a group truthfully means rewriting every
descendant's dimensions and relative positions, and doing that badly is worse
than not offering it.

**Keyboard nudging.** With something selected, the arrow keys nudge it along
the two axes most closely aligned with the screen, and a further pair of keys
handles the third. One press equals one snap increment, with a modifier for a
fine step and another for a coarse one. In rotate and resize modes the same
keys rotate and resize instead. Held keys repeat, and a run of repeats
coalesces into a single undo step.

---

## 7. User interface

### 7.1 Layout

A single main window: **outliner** (tree), **property editor**, **viewport**,
and a **status bar** showing the display unit, default segment count, the
export action and a message area. Panel sizes, window geometry and the display
mode persist between sessions.

### 7.2 Outliner

- Add a primitive or group: inserted into the selected group, or as a sibling
  of the selected leaf.
- Delete, with the root protected. Duplicate, producing a deep copy with a
  distinct identity, inserted directly after the original.
- Reorder within a parent, since order is semantic in a difference group.
- **Reparent by dragging**, with cycles prevented and a clear drop indicator.
- Rename in place. Toggle visibility directly in the tree.
- Multi-selection for delete, visibility toggling and grouping.
- **Group the current selection** into a new group in one action, preserving
  relative positions — the single most-used structural operation.

### 7.3 Property editor

- Name, visibility, anchor, and for groups the boolean operation.
- Dimension fields for the selected primitive, labelled with the current unit,
  built from that type's declared parameters.
- Position and rotation fields.
- Segment count override for curved shapes.
- Values commit on Enter and on leaving the field; the preview updates
  immediately.
- Lock toggles where a type offers them (equal sphere diameters, equal
  cylinder diameters), and radio-style choices where a measurement is
  ambiguous (across corners versus flats, wall thickness versus inner
  diameter).
- When a difference group is selected, state plainly which child is the base.

### 7.4 General

- Full menu bar; keyboard shortcuts for new, open, save, export, copy, cut,
  paste, duplicate, delete, group, frame-selection and undo — every one of
  them remappable (section 8).
- **Undo and redo across every model-mutating action**, including reparenting
  and numeric edits, with a sensible depth. Rapid edits to one field coalesce
  into a single undo step.
- Unsaved-changes indicator in the title bar and confirmation on quit.
- Legible on high-DPI displays; usable under both light and dark system
  themes.
- Consistent across both platforms: same shortcuts, same layout, native file
  dialogs.

---

## 8. Input, keymaps and the clipboard

### 8.1 Clipboard

- **Copy, cut and paste** on the current selection, using the platform's
  standard bindings, operating on whole subtrees including every child.
- **Duplicate** as a separate action: copy and paste in one step, inserted as
  a sibling directly after the original, without disturbing the clipboard.
- Pasted nodes receive fresh identities and keep every property — dimensions,
  anchor, rotation, visibility, segment overrides — and are left selected, so
  a nudge or a drag can follow immediately.
- Paste target follows the same rule as Add: into the selected group, or as a
  sibling of the selected leaf.
- Pasting into the same parent applies **no offset** — the copy lands exactly
  on the original. This is deliberate; it is what makes copy, move, repeat
  work. Names get a suffix so the outliner stays readable.
- Multi-selection copies as a set and pastes as a set, preserving relative
  positions and original order.
- The clipboard persists across the application's own windows and across
  projects opened one after another. Exchanging with other applications is not
  required, but the payload should be text in the same schema as the project
  file, so a selection can be pasted into a text editor and back again.
- Cut must not be able to lose work: either it removes on paste rather than on
  cut, or it is an undoable step of its own.

### 8.2 Keymaps

- **Every command and every navigation binding is remappable**, not a chosen
  subset. That includes the mouse buttons and modifiers for orbit, pan and
  zoom, the wheel direction for zoom, and the manipulator mode keys.
- Ships with **selectable presets** so someone arriving from another program
  is productive immediately: the application's own default, plus presets
  imitating the common mesh-editor and CAD navigation conventions. A preset is
  a starting point the user can then modify.
- A keymap editor listing every command grouped by area, with a search box,
  the current binding shown, and click-to-record for setting a new one.
- **Conflict detection**: assigning a binding already in use warns, names the
  command currently holding it, and offers to reassign or cancel. Silently
  overwriting an existing binding is not acceptable.
- Reset a single binding, or the whole map, to the preset default.
- Keymaps are stored per user, outside the project file, and export and import
  as one file so a user can carry theirs between machines.
- Wherever a shortcut is displayed — menus, tooltips, help — the interface
  shows the **current** binding, never a hardcoded label.
- Modifier conventions follow the host platform, and a keymap saved on one
  platform loads sensibly on the other.

---

## 9. Export

- Formats: **3MF** (default, because it records units), **STL**, **OBJ** and
  **PLY**. Binary variants where the format has one.
- Export the whole scene or only the current selection.
- Written meshes must be watertight, manifold, correctly wound and with
  outward-facing normals. Verify before writing and warn — naming the offending
  node — rather than shipping a file that fails in a slicer.
- Units written correctly where the format carries them; for formats that do
  not, state the assumed unit in the export dialog.
- An optional uniform scale factor at export time, defaulting to 1, for
  producing scaled prints without touching the model.
- Runs off the interaction path with progress and a working cancel; a time
  limit with a clear message rather than an indefinite hang.
- On failure, show the specific reason in a scrollable, copyable dialog and
  name the node responsible. Never a generic message.
- Remember the last export directory, format and scale factor between
  sessions.

---

## 10. Project files

- One user-visible project file holding the entire scene, the display unit,
  scene settings and camera.
- **Human-readable and text-based**, so projects are diffable and can go under
  version control.
- Carries a format version. Loading a newer version fails with a clear
  message; loading an older one migrates silently where possible.
- Malformed or truncated files produce a specific error naming what was wrong.
  Never crash, never silently produce an empty scene.
- Recently opened projects in the file menu.
- Opening a project by passing its path as a command-line argument, so file
  associations work on both platforms.

---

## 11. Packaging and distribution

**Linux:** a self-contained executable running on distributions still
receiving support at build time, without the user installing anything. A
portable single-file bundle is preferred. No specific desktop environment
required.

**Windows:** a self-contained executable. A portable build is required; an
optional installer registering the project file association may be provided in
addition, but must never be the only option.

Both:

- Version number in an About dialog and embedded in the binary metadata.
- Settings in the platform-appropriate per-user location, with a portable mode
  keeping them beside the executable.
- The binary must not require the source tree, a working directory or sibling
  files to be present.
- Automated build producing both artefacts from one commit, version derived
  from the tag.

---

## 12. Acceptance criteria

Done when all of the following pass on both platforms, from a downloaded
binary, on a machine that has never had the development environment installed:

1. Add a box, set it to 40 × 20 × 4 mm, export 3MF, open the result in a
   slicer: exactly those dimensions, in millimetres, no scaling prompt.
2. Repeat for every primitive in section 3.2: the exported bounding box
   matches the entered dimensions to within tessellation tolerance on curved
   axes and exactly on flat ones.
3. Create a hexagonal prism 10 mm across flats and confirm the exported
   distance across flats is 10 mm, not 10 mm across corners.
4. Cut a 6 mm cylinder through the 4 mm plate with a difference group,
   positioned 12 mm from the left edge, and confirm the hole is round, in the
   right place, and the mesh is watertight.
5. Subtract a shape whose face is exactly coplanar with the base's face, and
   one that touches it at a single edge: both produce valid manifold output.
6. Switch the display unit to metres and confirm the plate reads `0.04` ×
   `0.02` × `0.004` and that its geometry is unchanged; switch back and
   confirm the original numbers return exactly.
7. Switch a node's anchor from centre to base and confirm the shape is
   unchanged and only its origin moved.
8. Nest a group inside a group, move the outer one, and confirm the assembly
   moves as one.
9. Hide one child of a difference group and confirm the cut disappears from
   the preview immediately, with no other change.
10. Select five objects, group them in one action, and confirm their relative
    positions are unchanged.
11. Perform twenty mixed edits including a reparent, undo all of them, redo
    all of them, and end with the tree identical to the state after edit
    twenty.
12. Click geometry in the viewport and confirm the correct node is selected in
    the outliner.
13. Build a scene of 200 primitives with nested booleans and confirm the
    viewport stays interactive and single-value edits still feel immediate.
14. Enter garbage into a numeric field: previous value returns, no dialog, no
    corruption.
15. Construct a boolean that cannot be evaluated and confirm the failing node
    is named, the rest of the scene still previews, and export refuses with a
    clear reason.
16. Start a long export and confirm the window stays responsive, cancel works,
    and no partial file is left behind.
17. Save, quit, relaunch, reopen: tree structure, names, values, anchors,
    visibility, unit, segment counts and camera are exactly as they were.
18. Open a project file truncated mid-file and confirm a clear error rather
    than a crash.
19. Run on a machine with no accelerated graphics and confirm the application
    starts and remains usable.
20. Copy an object and paste it: the copy lands exactly on the original, has a
    distinct identity, is left selected, and editing it leaves the original
    untouched.
21. Copy a group with nested children into a different group: the whole
    subtree arrives with relative positions and child order intact.
22. Cut a subtree, paste it elsewhere, then undo twice and confirm the
    original tree is restored exactly.
23. Drag a move handle: the property editor tracks it live, the drag snaps to
    the grid increment, the free-drag modifier disables snapping, Escape
    restores the pre-drag position exactly, and a completed drag undoes in one
    step.
24. Drag the right face handle of a box: its width parameter changed, the left
    face did not move, the property editor shows the new width, and the saved
    project file contains no scale factor anywhere.
25. Drag a corner handle with the proportions modifier held and confirm the
    ratio between the affected dimensions is preserved.
26. Nudge a selection with the arrow keys, hold to repeat, and confirm the
    step matches the snap increment and the whole repeat run is a single undo
    step.
27. Switch to a different keymap preset, rebind a command onto a combination
    already in use, and confirm the conflict is reported by name rather than
    silently overwritten.
28. Restart after rebinding: the keymap persisted, and the menus and tooltips
    show the new binding rather than the default.
29. Remap the orbit mouse button and confirm navigation follows the new
    binding immediately, without a restart.

---

## 13. Explicitly out of scope for the first version

Mesh import, textures and materials, sketch-based or extrusion modelling,
lofting and sweeping, fillets and chamfers on arbitrary edges, animation,
scripting, expressions linking one dimension to another, resize handles on
groups, and any cloud or collaboration feature.

## 14. Candidates for later versions, in rough priority order

1. Named scene variables that dimensions can reference, so a model resizes
   from one place.
2. Reusable components: a subtree defined once and instantiated many times,
   with edits propagating to every instance.
3. Linear and radial arrays as a node type, rather than duplicating by hand.
4. Alignment and distribution helpers, and snapping to the grid.
5. Fillets and chamfers on selected edges.
6. Mesh import, so an existing file can be measured against or cut with.
7. Resize handles on groups, rewriting every descendant's dimensions and
   relative positions truthfully rather than applying a scale transform.
8. Helix and spring primitives, and lofted profiles.
9. macOS as a third target.
