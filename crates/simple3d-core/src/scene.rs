//! The node tree (spec section 3): nodes, groups, the scene and every
//! structural edit the outliner offers.
//!
//! The tree is an arena of nodes keyed by a stable id that survives save/load,
//! reparenting and undo. `BTreeMap` rather than `HashMap` so iteration order is
//! deterministic, which matters because evaluation must be (section 5.2).

mod naming;
pub use naming::{copy_name, free_name};
mod colour;
pub use colour::{colour_tag, Colour};
mod group_op;
pub use group_op::{Anchor, GroupOp};
mod body;
pub use body::{Body, ExportBody};
mod node;
pub use node::{Node, Visibility};
mod node_read;
mod view_settings;
pub use view_settings::{AxisStyle, PreviewViewport, SectionView};
mod settings;
pub use settings::SceneSettings;
mod camera;
pub use camera::Camera;
mod access;
mod add;
mod arrange;
mod collection;
mod edit;
mod export_body;
mod node_data;
mod paint;
mod split;
mod subtree;
pub use node_data::NodeData;
#[cfg(test)]
mod tests;

use simple3d_geom::Vec3;
use std::collections::BTreeMap;

pub type NodeId = u64;

fn is_off(section: &SectionView) -> bool {
    *section == SectionView::default()
}

fn is_no_change(mode: &PreviewViewport) -> bool {
    *mode == PreviewViewport::NoChange
}

fn default_snap_step() -> f64 {
    1.0
}

fn all_axes() -> [bool; 3] {
    [true; 3]
}

#[derive(Clone, Debug)]
pub struct Scene {
    nodes: BTreeMap<NodeId, Node>,
    root: NodeId,
    next_id: NodeId,
    pub settings: SceneSettings,
    pub camera: Camera,
}

impl Default for Scene {
    fn default() -> Self {
        Scene::new()
    }
}

fn default_true() -> bool {
    true
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[allow(non_snake_case)]
fn Vec3_zero() -> Vec3 {
    Vec3::ZERO
}

#[allow(non_snake_case)]
fn Vec3_one() -> Vec3 {
    Vec3::ONE
}

fn is_unit_scale(scale: &Vec3) -> bool {
    *scale == Vec3::ONE
}
