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

/// Which renderer draws the viewport.
///
/// The CPU rasterizer is the default and the fallback, and it is the one the
/// application's promise rests on: it needs no accelerated graphics and has no
/// shader to fail to compile (spec section 2.7, acceptance criterion 19). The
/// GPU renderer draws the same scene through the OpenGL context the window
/// already has, which costs nothing to have available and is a great deal
/// faster on a large viewport -- but it can fail on a driver, and when it does
/// the viewport falls back to the CPU rather than showing nothing.
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
