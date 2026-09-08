//! The colours, sizes and fonts the interface is drawn from.

use egui::Color32;

/// One named colour. Everything drawn by the interface comes from here.
pub mod token {
    use egui::Color32;

    /// Viewport background (top of a subtle vertical gradient to [`SURFACE_0B`]).
    pub const SURFACE_0: Color32 = Color32::from_rgb(0x15, 0x18, 0x1C);
    /// The bottom of that gradient.
    pub const SURFACE_0B: Color32 = Color32::from_rgb(0x1B, 0x1F, 0x24);
    /// Dock background.
    pub const SURFACE_1: Color32 = Color32::from_rgb(0x1E, 0x22, 0x28);
    /// Panel headers, tool rail, input fields.
    pub const SURFACE_2: Color32 = Color32::from_rgb(0x26, 0x2B, 0x33);
    /// Hover, dividers, grid minor lines.
    pub const SURFACE_3: Color32 = Color32::from_rgb(0x32, 0x38, 0x44);

    /// Values, names.
    pub const TEXT_HI: Color32 = Color32::from_rgb(0xE6, 0xEA, 0xF0);
    /// Labels, units, disabled.
    pub const TEXT_LO: Color32 = Color32::from_rgb(0x8B, 0x95, 0xA5);

    /// The scrollbar handle at rest. The divider grey the scrollbar has always
    /// been described as, rather than the widget fill it was taking, which sat
    /// one shade off the dock behind it and left a list with more rows than fit
    /// looking complete.
    pub const SCROLL_HANDLE: Color32 = Color32::from_rgb(0x3E, 0x46, 0x54);
    /// And under the pointer, on its way to [`TEXT_LO`] while it is dragged.
    pub const SCROLL_HANDLE_HOVER: Color32 = Color32::from_rgb(0x55, 0x5F, 0x70);

    /// Selection, active tool, focus ring -- a machined-brass amber.
    pub const ACCENT: Color32 = Color32::from_rgb(0xE8, 0xA3, 0x3D);
    /// Dimension readouts, the measure tool, snap indicators.
    pub const MEASURE: Color32 = Color32::from_rgb(0x4F, 0xC3, 0xD9);
    /// Difference operands, destructive actions, errors.
    pub const DANGER: Color32 = Color32::from_rgb(0xD4, 0x57, 0x4E);

    pub const AXIS_X: Color32 = Color32::from_rgb(0xD4, 0x57, 0x4E);
    pub const AXIS_Y: Color32 = Color32::from_rgb(0x6F, 0xBF, 0x5B);
    pub const AXIS_Z: Color32 = Color32::from_rgb(0x55, 0x90, 0xD9);
}

/// Row and control metrics. Density over comfort: this is a tool used for
/// hours, so rows are tight and there is no decorative whitespace.
pub mod metric {
    /// Height of one outliner row.
    pub const ROW: f32 = 22.0;
    /// Height of one input row.
    pub const INPUT_ROW: f32 = 24.0;
    /// Padding inside a panel.
    pub const PANEL_PAD: f32 = 8.0;
    /// Gap between adjacent controls.
    pub const GAP: f32 = 4.0;
    /// Width of the tool rail.
    pub const RAIL: f32 = 40.0;
    /// Height of the menu bar. The window's own title bar sits above it and is
    /// the window system's business, so this row holds nothing but menus.
    pub const MENU_BAR: f32 = 28.0;
    /// The row of document tabs under it (issue 61).
    pub const TAB_BAR: f32 = 26.0;
    /// The row of buttons along the foot of every dialog, and the size each of
    /// those buttons is: one place, so no dialog can drift from the rest.
    pub const DIALOG_ACTIONS: f32 = 44.0;
    /// The margin inside a dialog, the same on all four sides of the contents
    /// and inside the row of buttons, so what a dialog says sits the same
    /// distance from the window's edge as from the rule over its buttons.
    pub const DIALOG_PAD: f32 = 12.0;
    pub const DIALOG_BUTTON: f32 = 26.0;
    pub const DIALOG_BUTTON_WIDTH: f32 = 96.0;
    /// Height of the status bar.
    pub const STATUS_BAR: f32 = 24.0;
    /// Room kept clear at the right end of the status bar for the timing, node
    /// and triangle readout, so a long message is elided rather than being drawn
    /// straight over it.
    pub const STATUS_READOUT: f32 = 260.0;
    /// Side of the orientation cube in the viewport's bottom-right corner.
    pub const VIEW_CUBE: f32 = 72.0;
}

/// Type sizes. Labels are one step below values so a column of numbers reads
/// as the content and the words around it as the frame.
pub mod font {
    pub const LABEL: f32 = 12.0;
    pub const VALUE: f32 = 13.0;
    pub const HEADER: f32 = 12.0;
    /// A dialog's own title, where one is written into the body: bigger than the
    /// text under it, or it is not a title at all.
    pub const TITLE: f32 = 17.0;
    pub const SMALL: f32 = 11.0;
}

/// The colours offered as one click in the outliner's context menu.
///
/// A menu cannot hold egui's colour picker -- the picker is a popup, and
/// opening one closes the menu underneath it before anything can be chosen --
/// so the menu offers a short palette of ordinary buttons instead and the
/// property editor keeps the full picker. These are chosen to stay legible as
/// shaded solids against the viewport's dark ground and its light one, and to
/// stay apart from the accent amber a selection is drawn in.
pub const PAINT_PRESETS: [(&str, Color32); 8] = [
    ("Slate", Color32::from_rgb(0x8C, 0x97, 0xA8)),
    ("Blue", Color32::from_rgb(0x2E, 0x7D, 0xD2)),
    ("Teal", Color32::from_rgb(0x2F, 0xA8, 0xA0)),
    ("Green", Color32::from_rgb(0x5A, 0xB0, 0x54)),
    ("Yellow", Color32::from_rgb(0xC9, 0xB0, 0x3A)),
    ("Orange", Color32::from_rgb(0xE0, 0x7A, 0x2B)),
    ("Red", Color32::from_rgb(0xCC, 0x4B, 0x45)),
    ("Violet", Color32::from_rgb(0x8E, 0x6F, 0xD0)),
];
