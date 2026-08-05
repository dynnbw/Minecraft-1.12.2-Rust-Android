use crate::net::minecraft::util::EnumFacing::EnumFacing;

/// World-space identity of one MCP 1.12.2 RenderChunk (16 x 16 x 16 blocks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RenderChunkKey {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl RenderChunkKey {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub const fn offset(self, facing: EnumFacing) -> Self {
        let (dx, dy, dz) = facing.offsets();
        Self::new(self.x + dx, self.y + dy, self.z + dz)
    }

    pub const fn minBlock(self) -> [i32; 3] {
        [self.x * 16, self.y * 16, self.z * 16]
    }

    pub const fn maxBlock(self) -> [i32; 3] {
        [self.x * 16 + 16, self.y * 16 + 16, self.z * 16 + 16]
    }

    pub const fn isValidWorldHeight(self) -> bool {
        self.y >= 0 && self.y < 16
    }
}
