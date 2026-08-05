use crate::net::minecraft::client::model::ModelBook::{BookMesh, ModelBook};

/// Vulkan-backed semantic owner for MCP 1.12.2
/// `TileEntityEnchantmentTableRenderer`.
pub struct TileEntityEnchantmentTableRenderer;

impl TileEntityEnchantmentTableRenderer {
    /// Produces the exact `ModelBook#render` geometry parameters resolved by
    /// the tile-entity renderer. World translation, bob, yaw and fixed tilt
    /// remain Vulkan backend transforms.
    pub fn buildBookMesh(
        ticks: f32,
        pageFlipRight: f32,
        pageFlipLeft: f32,
        spread: f32,
    ) -> BookMesh {
        ModelBook::buildMesh(ticks, pageFlipRight, pageFlipLeft, spread)
    }
}
