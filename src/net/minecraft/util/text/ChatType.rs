#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatType {
    #[default]
    Chat,
    System,
    GameInfo,
}

impl ChatType {
    pub const fn func_192583_a(self) -> i8 {
        match self {
            Self::Chat => 0,
            Self::System => 1,
            Self::GameInfo => 2,
        }
    }

    pub const fn func_192582_a(value: i8) -> Self {
        match value {
            1 => Self::System,
            2 => Self::GameInfo,
            _ => Self::Chat,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_ids_match_mcp() {
        assert_eq!(ChatType::Chat.func_192583_a(), 0);
        assert_eq!(ChatType::System.func_192583_a(), 1);
        assert_eq!(ChatType::GameInfo.func_192583_a(), 2);
        assert_eq!(ChatType::func_192582_a(99), ChatType::Chat);
    }
}
