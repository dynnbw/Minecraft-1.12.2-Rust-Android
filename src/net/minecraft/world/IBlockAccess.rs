use crate::net::minecraft::block::state::IBlockState::IBlockState;
use crate::net::minecraft::util::math::BlockPos::BlockPos;

/// Read-only block access used by neighbour-derived actual-state logic.
/// Mirrors MCP `IBlockAccess` at the boundary needed by rendering and collision.
pub trait IBlockAccess {
    fn getBlockState(&self, pos: BlockPos) -> IBlockState;

    fn isAirBlock(&self, pos: BlockPos) -> bool {
        self.getBlockState(pos).isAir()
    }
}
