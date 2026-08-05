use std::sync::Arc;

use crate::net::minecraft::block::state::IBlockState::IBlockState;
use crate::net::minecraft::world::chunk::BlockStateContainer::BlockStateContainer;
use crate::net::minecraft::world::chunk::NibbleArray::NibbleArray;
use crate::net::minecraft::world::chunk::storage::ExtendedBlockStorage::ExtendedBlockStorage;

/// Rust equivalent of MCP `Chunk`'s section ownership.
///
/// RenderChunk jobs need an immutable snapshot while the network thread may
/// replace or mutate a section. `Arc::make_mut` provides copy-on-write section
/// snapshots: cloning a Chunk for background tessellation is cheap, while a
/// later server block update cannot alter the worker's captured data.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub xPosition: i32,
    pub zPosition: i32,
    storageArrays: Vec<Option<Arc<ExtendedBlockStorage>>>,
    blockBiomeArray: [u8; 256],
    revision: u64,
    sectionRevisions: [u64; 16],
}

impl Chunk {
    pub fn new(x: i32, z: i32) -> Self {
        Self {
            xPosition: x,
            zPosition: z,
            storageArrays: vec![None; 16],
            blockBiomeArray: [0; 256],
            revision: 0,
            sectionRevisions: [0; 16],
        }
    }

    pub fn setStorage(&mut self, index: usize, storage: Option<ExtendedBlockStorage>) {
        if index < 16 {
            self.storageArrays[index] = storage.map(Arc::new);
            self.revision = self.revision.wrapping_add(1);
            self.sectionRevisions[index] = self.sectionRevisions[index].wrapping_add(1);
        }
    }

    pub fn getBlockStorageArray(&self) -> &[Option<Arc<ExtendedBlockStorage>>] {
        &self.storageArrays
    }

    pub fn setBiomeArray(&mut self, data: &[u8]) {
        if data.len() >= 256 {
            self.blockBiomeArray.copy_from_slice(&data[..256]);
            self.revision = self.revision.wrapping_add(1);
            for sectionRevision in &mut self.sectionRevisions {
                *sectionRevision = sectionRevision.wrapping_add(1);
            }
        }
    }

    pub fn getBiomeArray(&self) -> &[u8; 256] {
        &self.blockBiomeArray
    }

    pub fn getGlobalStateId(&self, x: usize, y: usize, z: usize) -> i32 {
        if x >= 16 || z >= 16 || y >= 256 {
            return 0;
        }
        self.storageArrays[y >> 4]
            .as_ref()
            .map(|storage| storage.getGlobalStateId(x, y & 15, z))
            .unwrap_or(0)
    }

    pub fn getBlockState(&self, x: usize, y: usize, z: usize) -> IBlockState {
        IBlockState::fromGlobalStateId(self.getGlobalStateId(x, y, z))
    }

    pub fn setBlockState(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        state: IBlockState,
        hasSkyLight: bool,
    ) -> Result<IBlockState, String> {
        if x >= 16 || z >= 16 || y >= 256 {
            return Err("block coordinate outside chunk".to_owned());
        }
        let sectionIndex = y >> 4;
        if self.storageArrays[sectionIndex].is_none() {
            self.storageArrays[sectionIndex] = Some(Arc::new(ExtendedBlockStorage::fromNetwork(
                (sectionIndex * 16) as i32,
                BlockStateContainer::new(),
                NibbleArray::new(),
                hasSkyLight.then(NibbleArray::new),
            )));
        }
        let storage = Arc::make_mut(
            self.storageArrays[sectionIndex]
                .as_mut()
                .expect("created section"),
        );
        let old = storage
            .getDataMut()
            .setGlobalStateId(x, y & 15, z, state.getGlobalStateId())?;
        self.revision = self.revision.wrapping_add(1);
        self.sectionRevisions[sectionIndex] = self.sectionRevisions[sectionIndex].wrapping_add(1);
        Ok(IBlockState::fromGlobalStateId(old))
    }

    /// Render-only invalidation for tile entities whose NBT changes the
    /// baked block model without changing the compact block state (notably
    /// `BlockFlowerPot.CONTENTS`).
    pub fn markSectionDirty(&mut self, sectionIndex: usize) {
        if sectionIndex < 16 {
            self.revision = self.revision.wrapping_add(1);
            self.sectionRevisions[sectionIndex] = self.sectionRevisions[sectionIndex].wrapping_add(1);
        }
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Revision of one 16-block-high storage section. RenderChunk invalidation
    /// uses this instead of rebuilding the complete 16 x 256 x 16 column.
    pub const fn sectionRevision(&self, sectionIndex: usize) -> u64 {
        if sectionIndex < 16 {
            self.sectionRevisions[sectionIndex]
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_snapshot_is_copy_on_write_after_block_update() {
        let mut chunk = Chunk::new(0, 0);
        chunk
            .setBlockState(1, 2, 3, IBlockState::fromGlobalStateId(16), true)
            .unwrap();
        let snapshot = chunk.clone();
        chunk
            .setBlockState(1, 2, 3, IBlockState::fromGlobalStateId(32), true)
            .unwrap();
        assert_eq!(snapshot.getGlobalStateId(1, 2, 3), 16);
        assert_eq!(chunk.getGlobalStateId(1, 2, 3), 32);
    }
    #[test]
    fn block_update_only_advances_its_render_section_revision() {
        let mut chunk = Chunk::new(0, 0);
        let before_low = chunk.sectionRevision(1);
        let before_high = chunk.sectionRevision(9);
        chunk
            .setBlockState(1, 20, 3, IBlockState::fromGlobalStateId(16), true)
            .unwrap();
        assert_ne!(chunk.sectionRevision(1), before_low);
        assert_eq!(chunk.sectionRevision(9), before_high);
    }

}
