/// Immutable frame snapshot consumed by the Vulkan equivalent of
/// MCP 1.12.2 `ParticleManager#renderParticles` layer 0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleRenderState {
    pub prevPosition: [f64; 3],
    pub position: [f64; 3],
    pub textureIndex: i32,
    pub scale: f32,
    pub particleAngle: f32,
    pub prevParticleAngle: f32,
    pub color: [f32; 4],
    pub fullBright: bool,
    pub transparent: bool,
}
