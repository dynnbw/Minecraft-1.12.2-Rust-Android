use crate::net::minecraft::util::ResourceLocation::ResourceLocation;

/// MCP 1.12.2 `RenderItemFrame` model and texture constants. The Java renderer
/// fetches these two block textures through TextureMap; Vulkan registers the
/// same resources in its shared atlas for the dynamic entity pass.
pub struct RenderItemFrame;

impl RenderItemFrame {
    pub const ITEM_TRANSLATE_Z: f32 = 0.4375;
    pub const ITEM_SCALE: f32 = 0.5;

    pub fn woodTexture() -> ResourceLocation {
        ResourceLocation::parse("textures/blocks/planks_birch.png")
    }

    pub fn backgroundTexture() -> ResourceLocation {
        ResourceLocation::parse("textures/blocks/itemframe_background.png")
    }

    pub fn allTextures() -> Vec<ResourceLocation> {
        vec![Self::woodTexture(), Self::backgroundTexture()]
    }
}
