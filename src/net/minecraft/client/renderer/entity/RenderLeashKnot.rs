use crate::net::minecraft::util::ResourceLocation::ResourceLocation;

/// MCP 1.12.2 `RenderLeashKnot` constants.
pub struct RenderLeashKnot;

impl RenderLeashKnot {
    pub fn texture() -> ResourceLocation {
        ResourceLocation::parse("textures/entity/lead_knot.png")
    }
}
