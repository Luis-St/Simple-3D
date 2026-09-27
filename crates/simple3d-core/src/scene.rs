//! The node tree (spec section 3) and every structural edit. An arena keyed by stable ids that survive
//! save/load, reparenting and undo; `BTreeMap` for deterministic evaluation (section 5.2).

mod naming;
pub use naming::{copy_name, free_name, is_default_name};
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
pub use view_settings::{AxisStyle, PreviewViewport, SectionKeep, SectionView};
mod settings;
pub use settings::SceneSettings;
mod camera;
pub use camera::Camera;
mod component;
pub use component::{link, reaches, ComponentId, Components, ROOT_COMPONENT};
mod access;
mod add;
mod arrange;
mod collection;
mod edit;
mod export_body;
mod node_data;
mod paint;
mod rebuild;
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
    /// Integrated components, recursively (issue 113; see [`component`]). Filled from the project, kept by
    /// undo, never saved with the scene.
    pub components: std::sync::Arc<Components>,
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
