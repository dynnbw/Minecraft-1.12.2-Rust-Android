use serde::{Deserialize, Serialize};

/// Nested `EntityPlayer.EnumChatVisibility` value used by
/// `CPacketClientSettings` in protocol 340.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnumChatVisibility {
    Full,
    System,
    Hidden,
}

impl EnumChatVisibility {
    pub const fn getChatVisibility(id: i32) -> Self {
        match id {
            1 => Self::System,
            2 => Self::Hidden,
            _ => Self::Full,
        }
    }

    pub const fn getChatVisibilityId(self) -> i32 {
        match self {
            Self::Full => 0,
            Self::System => 1,
            Self::Hidden => 2,
        }
    }
}
