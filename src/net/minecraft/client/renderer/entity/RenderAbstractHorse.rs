use crate::net::minecraft::client::entity::EntityOtherClient::MobEntityType;
use crate::net::minecraft::client::model::ModelHorse::HorseModelVariant;
use crate::net::minecraft::util::ResourceLocation::ResourceLocation;

pub struct RenderAbstractHorse;

impl RenderAbstractHorse {
    pub fn variant(entityType: MobEntityType) -> Option<HorseModelVariant> {
        Some(match entityType.registryName {
            "donkey" => HorseModelVariant::Donkey,
            "mule" => HorseModelVariant::Mule,
            "skeleton_horse" => HorseModelVariant::Skeleton,
            "zombie_horse" => HorseModelVariant::Zombie,
            _ => return None,
        })
    }

    pub fn texture(variant: HorseModelVariant) -> ResourceLocation {
        let path = match variant {
            HorseModelVariant::Donkey => "textures/entity/horse/donkey.png",
            HorseModelVariant::Mule => "textures/entity/horse/mule.png",
            HorseModelVariant::Skeleton => "textures/entity/horse/horse_skeleton.png",
            HorseModelVariant::Zombie => "textures/entity/horse/horse_zombie.png",
            HorseModelVariant::Horse => "textures/entity/horse/horse_white.png",
        };
        ResourceLocation::new("minecraft", path)
    }
}
