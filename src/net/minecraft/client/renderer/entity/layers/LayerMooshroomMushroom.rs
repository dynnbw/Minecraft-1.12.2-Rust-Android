/// MCP 1.12.2 `LayerMooshroomMushroom` state and transform owner. Vulkan applies
/// these constants to the real red-mushroom baked block model.
pub struct LayerMooshroomMushroom;

impl LayerMooshroomMushroom {
    pub const RED_MUSHROOM_GLOBAL_STATE: i32 = 40 << 4;
    pub const BODY_FLIP: [f32; 3] = [1.0, -1.0, 1.0];
    pub const FIRST_BODY_TRANSLATION: [f32; 3] = [0.2, 0.35, 0.5];
    pub const FIRST_BODY_ROTATION_Y: f32 = 42.0;
    pub const FIRST_MODEL_TRANSLATION: [f32; 3] = [-0.5, -0.5, 0.5];
    /// OpenGL switches to FRONT culling after the additional Y flip, so the
    /// delegate renderer must emit reversed quad winding for the three mushroom
    /// block models to preserve vanilla-facing geometry.
    pub const REVERSE_WINDING: bool = true;
    pub const BLOCK_MODEL_ROTATION_Y: f32 = 90.0;
    pub const SECOND_BODY_TRANSLATION: [f32; 3] = [0.1, 0.0, -0.6];
    pub const SECOND_BODY_ROTATION_Y: f32 = 42.0;
    pub const HEAD_POST_RENDER_SCALE: f32 = 0.0625;
    pub const HEAD_TRANSLATION: [f32; 3] = [0.0, 0.7, -0.2];
    pub const HEAD_ROTATION_Y: f32 = 12.0;

    pub const fn shouldRender(child: bool, invisible: bool) -> bool {
        !child && !invisible
    }
}
