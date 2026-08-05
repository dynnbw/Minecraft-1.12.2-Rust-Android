use crate::net::minecraft::block::state::IBlockState::IBlockState;

pub const BLOCK_ID: i32 = 84;

pub const fn hasRecord(state: IBlockState) -> bool {
    state.getBlockId() == BLOCK_ID && state.getMetadata() != 0
}

/// Client-visible part of `BlockJukebox#onBlockActivated`. `dropRecord` only
/// spawns/removes the record TileEntity contents on the logical server, but
/// HAS_RECORD is cleared through `WorldClient#setBlockState` on both sides.
pub fn onBlockActivatedState(state: IBlockState) -> Option<IBlockState> {
    hasRecord(state).then(|| IBlockState::fromGlobalStateId(BLOCK_ID << 4))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occupied_jukebox_clears_has_record() {
        let occupied = IBlockState::fromGlobalStateId((BLOCK_ID << 4) | 1);
        assert_eq!(onBlockActivatedState(occupied).unwrap().getMetadata(), 0);
        assert!(onBlockActivatedState(IBlockState::fromGlobalStateId(BLOCK_ID << 4)).is_none());
    }
}
