use crate::net::minecraft::client::renderer::tileentity::TileEntityItemStackRenderer::{
    BuiltInItemMesh, TileEntityItemStackRenderer,
};

/// Immutable render input produced after MCP `TileEntityChest#checkForAdjacentChests`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChestRenderInput {
    pub trapped: bool,
    pub ender: bool,
    pub large: bool,
    pub metadata: i32,
    pub adjacentXPos: bool,
    pub adjacentZPos: bool,
    /// Cubic-eased lid progress from `TileEntityChestRenderer`.
    pub lidProgress: f32,
}

/// CPU/Vulkan equivalent of MCP 1.12.2 `TileEntityChestRenderer`.
pub struct TileEntityChestRenderer;

impl TileEntityChestRenderer {
    pub fn buildMesh(input: ChestRenderInput) -> BuiltInItemMesh {
        TileEntityItemStackRenderer::buildWorldChest(
            input.trapped,
            input.ender,
            input.large,
            input.metadata,
            input.adjacentXPos,
            input.adjacentZPos,
            input.lidProgress,
        )
    }
}
