use crate::net::minecraft::client::entity::EntityOtherClient::{EntityOtherClient, MobEntityType};
use crate::net::minecraft::util::ResourceLocation::ResourceLocation;

/// MCP 1.12.2 `RenderGhast`.
pub struct RenderGhast;

impl RenderGhast {
    pub const PRE_SCALE: f32 = 4.5;

    pub fn supports(entityType: MobEntityType) -> bool {
        entityType.registryName == "ghast"
    }

    pub fn texture(entity: &EntityOtherClient) -> ResourceLocation {
        let path = if entity.dataManager.boolean(12, false) {
            "textures/entity/ghast/ghast_shooting.png"
        } else {
            "textures/entity/ghast/ghast.png"
        };
        ResourceLocation::new("minecraft", path)
    }

    pub fn allTextures() -> Vec<ResourceLocation> {
        vec![
            ResourceLocation::new("minecraft", "textures/entity/ghast/ghast.png"),
            ResourceLocation::new("minecraft", "textures/entity/ghast/ghast_shooting.png"),
        ]
    }
}
