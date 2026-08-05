use crate::net::minecraft::client::entity::EntityOtherClient::MobEntityType;
use crate::net::minecraft::util::ResourceLocation::ResourceLocation;

/// MCP 1.12.2 `RenderBlaze`.
pub struct RenderBlaze;

impl RenderBlaze {
    pub const PACKED_FULL_BRIGHT: u32 = 15_728_880;

    pub fn supports(entityType: MobEntityType) -> bool {
        entityType.registryName == "blaze"
    }

    pub fn texture() -> ResourceLocation {
        ResourceLocation::new("minecraft", "textures/entity/blaze.png")
    }
}
