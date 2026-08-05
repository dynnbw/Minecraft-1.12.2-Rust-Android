/// MCP 1.12.2 `BlockFaceShape` values. These describe the approximate shape of
/// a queried block face for actual-state connections and placement rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockFaceShape {
    SOLID,
    BOWL,
    CENTER_SMALL,
    MIDDLE_POLE_THIN,
    CENTER,
    MIDDLE_POLE,
    CENTER_BIG,
    MIDDLE_POLE_THICK,
    UNDEFINED,
}
