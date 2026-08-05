use crate::net::minecraft::util::ResourceLocation::ResourceLocation;

/// MCP 1.12.2 `RenderPainting` constants.
pub struct RenderPainting;

impl RenderPainting {
    pub const MODEL_SCALE: f32 = 0.0625;
    pub fn texture() -> ResourceLocation {
        ResourceLocation::parse("textures/painting/paintings_kristoffer_zetterstrand.png")
    }
}
