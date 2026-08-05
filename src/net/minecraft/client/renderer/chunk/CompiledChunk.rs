use crate::net::minecraft::client::renderer::chunk::SetVisibility::SetVisibility;
use crate::net::minecraft::util::BlockRenderLayer::BlockRenderLayer;
use crate::net::minecraft::util::EnumFacing::EnumFacing;

/// Render metadata corresponding to MCP 1.12.2 `CompiledChunk`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompiledChunk {
    layersUsed: u8,
    layersStarted: u8,
    empty: bool,
    visibility: SetVisibility,
}

impl Default for CompiledChunk {
    fn default() -> Self {
        Self::new()
    }
}

impl CompiledChunk {
    pub const fn new() -> Self {
        Self {
            layersUsed: 0,
            layersStarted: 0,
            empty: true,
            visibility: SetVisibility::allVisible(),
        }
    }

    pub const fn emptyVisible() -> Self {
        Self::new()
    }

    pub const fn isEmpty(self) -> bool {
        self.empty
    }

    pub fn setLayerUsed(&mut self, layer: BlockRenderLayer) {
        self.empty = false;
        self.layersUsed |= 1_u8 << layer.index();
    }

    pub const fn isLayerEmpty(self, layer: BlockRenderLayer) -> bool {
        (self.layersUsed & (1_u8 << layer.index())) == 0
    }

    pub fn setLayerStarted(&mut self, layer: BlockRenderLayer) {
        self.layersStarted |= 1_u8 << layer.index();
    }

    pub const fn isLayerStarted(self, layer: BlockRenderLayer) -> bool {
        (self.layersStarted & (1_u8 << layer.index())) != 0
    }

    pub const fn isVisible(self, first: EnumFacing, second: EnumFacing) -> bool {
        self.visibility.isVisible(first, second)
    }

    pub fn setVisibility(&mut self, visibility: SetVisibility) {
        self.visibility = visibility;
    }

    pub const fn visibility(self) -> SetVisibility {
        self.visibility
    }
}
