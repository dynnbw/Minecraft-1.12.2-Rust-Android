use crate::net::minecraft::client::entity::EntityOtherClient::MobEntityType;
use crate::net::minecraft::util::ResourceLocation::ResourceLocation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkeletonRenderVariant {
    Skeleton,
    Stray,
    WitherSkeleton,
}

pub struct RenderSkeleton;

impl RenderSkeleton {
    pub fn variant(entityType: MobEntityType) -> Option<SkeletonRenderVariant> {
        match entityType.registryName {
            "skeleton" => Some(SkeletonRenderVariant::Skeleton),
            "stray" => Some(SkeletonRenderVariant::Stray),
            "wither_skeleton" => Some(SkeletonRenderVariant::WitherSkeleton),
            _ => None,
        }
    }

    pub fn texture(variant: SkeletonRenderVariant) -> ResourceLocation {
        let path = match variant {
            SkeletonRenderVariant::Skeleton => "textures/entity/skeleton/skeleton.png",
            SkeletonRenderVariant::Stray => "textures/entity/skeleton/stray.png",
            SkeletonRenderVariant::WitherSkeleton => "textures/entity/skeleton/wither_skeleton.png",
        };
        ResourceLocation::new("minecraft", path)
    }

    pub fn overlayTexture(variant: SkeletonRenderVariant) -> Option<ResourceLocation> {
        matches!(variant, SkeletonRenderVariant::Stray).then(|| {
            ResourceLocation::new("minecraft", "textures/entity/skeleton/stray_overlay.png")
        })
    }

    pub const fn preScale(variant: SkeletonRenderVariant) -> f32 {
        match variant {
            SkeletonRenderVariant::WitherSkeleton => 1.2,
            SkeletonRenderVariant::Skeleton | SkeletonRenderVariant::Stray => 1.0,
        }
    }
}
