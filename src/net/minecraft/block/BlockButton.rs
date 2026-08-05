use crate::net::minecraft::block::state::IBlockState::IBlockState;
use crate::net::minecraft::util::EnumFacing::EnumFacing;
use crate::net::minecraft::util::math::AxisAlignedBB::AxisAlignedBB;

pub const fn isBlockButton(state: IBlockState) -> bool {
    matches!(state.getBlockId(), 77 | 143)
}

/// Exact `BlockButton#getStateFromMeta` facing decode.
pub const fn facing(state: IBlockState) -> EnumFacing {
    match state.getMetadata() & 7 {
        0 => EnumFacing::Down,
        1 => EnumFacing::East,
        2 => EnumFacing::West,
        3 => EnumFacing::South,
        4 => EnumFacing::North,
        _ => EnumFacing::Up,
    }
}

pub const fn isPowered(state: IBlockState) -> bool { state.getMetadata() & 8 != 0 }

/// Exact state mutation in `BlockButton#onBlockActivated`. An already
/// powered button still consumes the click but does not change state.
pub fn onBlockActivatedState(state: IBlockState) -> Option<IBlockState> {
    if !isBlockButton(state) || isPowered(state) {
        return None;
    }
    Some(IBlockState::fromGlobalStateId(
        (state.getBlockId() << 4) | (state.getMetadata() | 8),
    ))
}

/// Exact local-space result of `BlockButton#getBoundingBox`, including the
/// depressed 1/16-depth powered forms.
pub fn getBoundingBox(state: IBlockState) -> AxisAlignedBB {
    let powered = isPowered(state);
    match facing(state) {
        EnumFacing::East => {
            if powered {
                AxisAlignedBB::new(0.0, 0.375, 0.3125, 0.0625, 0.625, 0.6875)
            } else {
                AxisAlignedBB::new(0.0, 0.375, 0.3125, 0.125, 0.625, 0.6875)
            }
        }
        EnumFacing::West => {
            if powered {
                AxisAlignedBB::new(0.9375, 0.375, 0.3125, 1.0, 0.625, 0.6875)
            } else {
                AxisAlignedBB::new(0.875, 0.375, 0.3125, 1.0, 0.625, 0.6875)
            }
        }
        EnumFacing::South => {
            if powered {
                AxisAlignedBB::new(0.3125, 0.375, 0.0, 0.6875, 0.625, 0.0625)
            } else {
                AxisAlignedBB::new(0.3125, 0.375, 0.0, 0.6875, 0.625, 0.125)
            }
        }
        EnumFacing::North => {
            if powered {
                AxisAlignedBB::new(0.3125, 0.375, 0.9375, 0.6875, 0.625, 1.0)
            } else {
                AxisAlignedBB::new(0.3125, 0.375, 0.875, 0.6875, 0.625, 1.0)
            }
        }
        EnumFacing::Up => {
            if powered {
                AxisAlignedBB::new(0.3125, 0.0, 0.375, 0.6875, 0.0625, 0.625)
            } else {
                AxisAlignedBB::new(0.3125, 0.0, 0.375, 0.6875, 0.125, 0.625)
            }
        }
        EnumFacing::Down => {
            if powered {
                AxisAlignedBB::new(0.3125, 0.9375, 0.375, 0.6875, 1.0, 0.625)
            } else {
                AxisAlignedBB::new(0.3125, 0.875, 0.375, 0.6875, 1.0, 0.625)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powered_button_uses_depressed_bounds() {
        let off = IBlockState::fromGlobalStateId((77 << 4) | 4);
        let on = IBlockState::fromGlobalStateId((77 << 4) | 12);
        assert_eq!(facing(off), EnumFacing::North);
        assert_eq!(getBoundingBox(off).min_z, 0.875);
        assert_eq!(getBoundingBox(on).min_z, 0.9375);
    }
}
