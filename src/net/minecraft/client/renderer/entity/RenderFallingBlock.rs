use crate::net::minecraft::block::state::IBlockState::IBlockState;
use crate::net::minecraft::client::renderer::entity::Render::RenderProperties;
use crate::net::minecraft::util::math::BlockPos::BlockPos;

pub struct RenderFallingBlock;

impl RenderFallingBlock {
    pub const PROPERTIES: RenderProperties = RenderProperties::new(0.5, 1.0);

    /// SPacketSpawnObject type 70 stores Block.getStateId in the low 16 bits.
    pub const fn getBlockState(spawnData: i32) -> IBlockState {
        IBlockState::fromGlobalStateId(spawnData & 0xFFFF)
    }

    /// Source lighting/model position uses entity X/Z and bounding-box maxY.
    pub fn renderBlockPos(position: [f32; 3], height: f32) -> BlockPos {
        BlockPos::new(
            position[0].floor() as i32,
            (position[1] + height).floor() as i32,
            position[2].floor() as i32,
        )
    }
}
