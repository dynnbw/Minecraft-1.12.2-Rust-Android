use serde::{Deserialize, Serialize};

/// Exhibition-Reborn account entry. Field names intentionally retain the
/// original config schema so existing `account.json` data can be imported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    #[serde(default)]
    pub refreshToken: String,
    #[serde(default)]
    pub accessToken: String,
    #[serde(default)]
    pub username: String,
    #[serde(default = "current_time_millis")]
    pub timestamp: u64,
    #[serde(default)]
    pub uuid: String,
}

impl Account {
    pub fn new(
        refreshToken: impl Into<String>,
        accessToken: impl Into<String>,
        username: impl Into<String>,
        timestamp: u64,
        uuid: impl Into<String>,
    ) -> Self {
        Self {
            refreshToken: refreshToken.into(),
            accessToken: accessToken.into(),
            username: username.into(),
            timestamp,
            uuid: uuid.into(),
        }
    }

    pub fn accountTypeLabel(&self) -> &'static str {
        if !self.refreshToken.trim().is_empty() {
            "Microsoft"
        } else if !self.accessToken.trim().is_empty() {
            "Token"
        } else {
            "Offline"
        }
    }
}

pub fn current_time_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
