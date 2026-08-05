/// High-level rendering state corresponding to visual semantics used by the
/// 1.12.2 client. This is deliberately not an OpenGL emulator. Render code
/// records semantic state; the Vulkan backend selects compatible pipelines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompatibilityRenderState {
    pub depth_test: bool,
    pub depth_write: bool,
    pub blend: BlendMode,
    pub cull: CullMode,
    pub color_write: [bool; 4],
    pub alpha_cutoff: Option<f32>,
    pub fog: FogState,
}

impl Default for CompatibilityRenderState {
    fn default() -> Self {
        Self {
            depth_test: true,
            depth_write: true,
            blend: BlendMode::Disabled,
            cull: CullMode::Back,
            color_write: [true; 4],
            alpha_cutoff: Some(0.1),
            fog: FogState::Disabled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendMode {
    Disabled,
    Alpha,
    Additive,
    PremultipliedAlpha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CullMode {
    None,
    Front,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FogState {
    Disabled,
    Linear { start: f32, end: f32 },
    Exp { density: f32 },
    Exp2 { density: f32 },
}
