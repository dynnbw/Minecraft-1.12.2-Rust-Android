/// Port of MCP 1.12.2 `EnumHand`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnumHand {
    MainHand,
    OffHand,
}

impl EnumHand {
    pub const fn ordinal(self) -> i32 {
        match self {
            Self::MainHand => 0,
            Self::OffHand => 1,
        }
    }
}
