use crate::net::minecraft::network::datasync::EntityDataManager::EntityDataManager;
use crate::net::minecraft::util::math::BlockPos::BlockPos;

/// Source constants and synchronized accessors from MCP 1.12.2
/// `EntityEnderCrystal`. The base `Entity` data parameters occupy 0..=5.
pub struct EntityEnderCrystal;

impl EntityEnderCrystal {
    pub const WIDTH: f32 = 2.0;
    pub const HEIGHT: f32 = 2.0;
    pub const BEAM_TARGET_DATA_INDEX: u8 = 6;
    pub const SHOW_BOTTOM_DATA_INDEX: u8 = 7;

    pub fn beamTarget(dataManager: &EntityDataManager) -> Option<BlockPos> {
        dataManager.optionalBlockPos(Self::BEAM_TARGET_DATA_INDEX)
    }

    pub fn shouldShowBottom(dataManager: &EntityDataManager) -> bool {
        dataManager.boolean(Self::SHOW_BOTTOM_DATA_INDEX, true)
    }
}
