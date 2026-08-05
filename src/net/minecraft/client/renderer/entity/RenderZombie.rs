use crate::net::minecraft::client::entity::EntityOtherClient::MobEntityType;
use crate::net::minecraft::util::ResourceLocation::ResourceLocation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZombieRenderVariant {
    Zombie,
    Husk,
    ZombiePigman,
}

pub struct RenderZombie;

impl RenderZombie {
    pub fn variant(entityType: MobEntityType) -> Option<ZombieRenderVariant> {
        match entityType.registryName {
            "zombie" => Some(ZombieRenderVariant::Zombie),
            "husk" => Some(ZombieRenderVariant::Husk),
            "zombie_pigman" => Some(ZombieRenderVariant::ZombiePigman),
            _ => None,
        }
    }

    pub fn texture(variant: ZombieRenderVariant) -> ResourceLocation {
        let path = match variant {
            ZombieRenderVariant::Zombie => "textures/entity/zombie/zombie.png",
            ZombieRenderVariant::Husk => "textures/entity/zombie/husk.png",
            ZombieRenderVariant::ZombiePigman => "textures/entity/zombie_pigman.png",
        };
        ResourceLocation::new("minecraft", path)
    }

    pub const fn preScale(variant: ZombieRenderVariant) -> f32 {
        match variant {
            ZombieRenderVariant::Husk => 1.0625,
            ZombieRenderVariant::Zombie | ZombieRenderVariant::ZombiePigman => 1.0,
        }
    }
}
