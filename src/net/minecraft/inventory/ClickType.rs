/// Exact MCP 1.12.2 `ClickType` declaration order used on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickType {
    Pickup,
    QuickMove,
    Swap,
    Clone,
    Throw,
    QuickCraft,
    PickupAll,
}

impl ClickType {
    pub const fn ordinal(self) -> i32 {
        match self {
            Self::Pickup => 0,
            Self::QuickMove => 1,
            Self::Swap => 2,
            Self::Clone => 3,
            Self::Throw => 4,
            Self::QuickCraft => 5,
            Self::PickupAll => 6,
        }
    }
}
