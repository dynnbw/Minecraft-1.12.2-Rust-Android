use crate::net::minecraft::entity::Entity::Entity;
use crate::net::minecraft::util::math::AxisAlignedBB::AxisAlignedBB;
use crate::net::minecraft::util::math::BlockPos::BlockPos;

/// Client geometry from MCP 1.12.2 `EntityLeashKnot`.
pub struct EntityLeashKnot;

impl EntityLeashKnot {
    pub const WIDTH_PIXELS: i32 = 9;
    pub const HEIGHT_PIXELS: i32 = 9;
    pub const EYE_HEIGHT: f32 = -0.0625;
    pub const MAX_RENDER_DISTANCE_SQ: f64 = 1024.0;

    pub fn setHangingPosition(entity: &mut Entity, position: BlockPos) {
        let x = position.x as f64 + 0.5;
        let y = position.y as f64 + 0.5;
        let z = position.z as f64 + 0.5;
        entity.posX = x;
        entity.posY = y;
        entity.posZ = z;
        entity.prevPosX = x;
        entity.prevPosY = y;
        entity.prevPosZ = z;
        entity.boundingBox = AxisAlignedBB::new(
            x - 0.1875,
            y - 0.125,
            z - 0.1875,
            x + 0.1875,
            y + 0.375,
            z + 0.1875,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knot_is_snapped_to_fence_center_with_source_box() {
        let mut entity = Entity::default();
        EntityLeashKnot::setHangingPosition(&mut entity, BlockPos::new(2, 3, 4));
        assert_eq!([entity.posX, entity.posY, entity.posZ], [2.5, 3.5, 4.5]);
        assert!((entity.boundingBox.min_y - 3.375).abs() < 1.0e-9);
        assert!((entity.boundingBox.max_y - 3.875).abs() < 1.0e-9);
    }
}
