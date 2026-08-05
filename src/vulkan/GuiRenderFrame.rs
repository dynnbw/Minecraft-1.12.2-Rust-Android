use std::collections::HashMap;
use std::sync::Arc;

use crate::net::minecraft::util::ResourceLocation::ResourceLocation;
use crate::vulkan::GuiCompiler::CompiledGuiFrame;
use crate::vulkan::TextureSource::TextureSource;

/// Renderer-ready GUI command stream for native GPU backends.
///
/// The command order and logical coordinates are produced by the MCP-facing
/// GuiScreen/Gui classes. Texture sources are resolved by the same
/// TextureManager-equivalent cache used by the software fallback, so moving
/// submission to the GPU does not change resource-pack precedence or metadata.
#[derive(Debug, Clone)]
pub struct GuiRenderFrame {
    pub compiled: CompiledGuiFrame,
    pub textures: HashMap<ResourceLocation, Arc<TextureSource>>,
    pub guiWidth: i32,
    pub guiHeight: i32,
    pub outputWidth: u32,
    pub outputHeight: u32,
}
