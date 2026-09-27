//! How the model is drawn, and by what.

use serde::{Deserialize, Serialize};

/// How the viewport draws geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    #[default]
    Shaded,
    ShadedWithEdges,
    Wireframe,
}

impl DisplayMode {
    pub const ALL: [DisplayMode; 3] = [DisplayMode::Shaded, DisplayMode::ShadedWithEdges, DisplayMode::Wireframe];

    pub fn label(self) -> &'static str {
        match self {
            DisplayMode::Shaded => "Shaded",
            DisplayMode::ShadedWithEdges => "Shaded with edges",
            DisplayMode::Wireframe => "Wireframe",
        }
    }
}

/// Which renderer draws the viewport. The CPU rasterizer is the default and fallback, needing no GPU
/// (spec section 2.7, criterion 19); the faster GPU renderer falls back to it if the driver fails.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderEngine {
    #[default]
    Cpu,
    Gpu,
}

impl RenderEngine {
    pub const ALL: [RenderEngine; 2] = [RenderEngine::Cpu, RenderEngine::Gpu];

    pub fn label(self) -> &'static str {
        match self {
            RenderEngine::Cpu => "CPU",
            RenderEngine::Gpu => "GPU",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            RenderEngine::Cpu => "Draws the viewport in software, on every core there is. Works anywhere.",
            RenderEngine::Gpu => {
                "Draws the viewport through OpenGL. Faster on a large viewport; needs a working driver."
            }
        }
    }
}
