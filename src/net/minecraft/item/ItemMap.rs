use std::collections::HashMap;

use crate::net::minecraft::item::ItemStack::ItemStack;
use crate::net::minecraft::world::storage::MapData::MapData;

/// Client-visible MCP 1.12.2 `ItemMap` lookup semantics. Server-side map
/// generation and terrain sampling remain outside this client port.
pub struct ItemMap;

impl ItemMap {
    pub const FILLED_MAP_ITEM_ID: i16 = 358;

    pub const fn isFilledMap(stack: &ItemStack) -> bool {
        !stack.isEmpty() && stack.itemId == Self::FILLED_MAP_ITEM_ID
    }

    pub const fn getMapId(stack: &ItemStack) -> i32 {
        stack.itemDamage as i32
    }

    pub fn getMapName(stack: &ItemStack) -> String {
        format!("map_{}", Self::getMapId(stack))
    }

    /// Rust equivalent of `ItemMap#getMapData` on a remote world: no map is
    /// created client-side when the server has not yet sent SPacketMaps.
    pub fn getMapData<'a>(
        stack: &ItemStack,
        mapData: &'a HashMap<i32, MapData>,
    ) -> Option<&'a MapData> {
        if !Self::isFilledMap(stack) {
            return None;
        }
        mapData.get(&Self::getMapId(stack))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_name_and_id_use_item_metadata() {
        let stack = ItemStack {
            itemId: 358,
            count: 1,
            itemDamage: 42,
            tagCompound: None,
        };
        assert_eq!(ItemMap::getMapId(&stack), 42);
        assert_eq!(ItemMap::getMapName(&stack), "map_42");
    }
}
