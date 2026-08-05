/// Rust equivalent of MCP 1.12.2 `EnumSkyBlock`.
///
/// Vanilla stores sky and emitted block light as independent 4-bit channels.
/// Combining them with `max()` loses torch behaviour at night, so both channels
/// are preserved until the lightmap calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnumSkyBlock {
    Sky,
    Block,
}

impl EnumSkyBlock {
    pub const VALUES: [Self; 2] = [Self::Sky, Self::Block];

    pub const fn defaultLightValue(self) -> u8 {
        match self {
            Self::Sky => 15,
            Self::Block => 0,
        }
    }
}
