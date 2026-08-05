/// Per-frame values consumed by the OptiFine 1.12.2 shader uniform layer.
/// Matrices are column-major. `projectionMatrix` retains the shared Vulkan
/// clip convention and is converted exactly once at the OpenGL boundary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShaderFrameState {
    pub projectionMatrix: [f32; 16],
    pub modelViewMatrix: [f32; 16],
    pub cameraPosition: [f32; 3],
    pub clearColor: [f32; 4],
    pub fogColor: [f32; 3],
    pub skyColor: [f32; 3],
    /// `GameSettings.gammaSetting`, exposed by OptiFine as screenBrightness.
    pub screenBrightness: f32,
    /// Player-eye block/sky light in the 0..240 OpenGL lightmap convention.
    pub eyeBrightness: [i32; 2],
    /// Current terrain atlas dimensions used by all OptiFine programs.
    pub atlasSize: [i32; 2],
    pub celestialAngle: f32,
    pub dimension: i32,
    pub worldTime: i64,
    pub totalWorldTime: i64,
    pub partialTicks: f32,
    pub farPlane: f32,
}
