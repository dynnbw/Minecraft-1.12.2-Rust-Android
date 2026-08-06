use serde::{Deserialize, Serialize};
use std::path::Path;

/// Optional session configuration read from `<gameDir>/launcher.json`.
/// Android builds have no CLI args; this file provides the session identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AndroidLaunchConfig {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub player_id: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default = "default_user_type")]
    pub user_type: String,
}

fn default_user_type() -> String { "legacy".to_owned() }

impl Default for AndroidLaunchConfig {
    fn default() -> Self {
        Self {
            username: String::new(),
            player_id: String::new(),
            access_token: String::new(),
            user_type: default_user_type(),
        }
    }
}

impl AndroidLaunchConfig {
    /// Returns the parsed config, or Default when the file is absent or broken.
    pub fn load(game_dir: &Path) -> Self {
        let path = game_dir.join("launcher.json");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_config(dir: &std::path::Path, text: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("launcher.json"), text).unwrap();
    }

    #[test]
    fn parses_valid_config() {
        let dir = std::env::temp_dir().join(format!("launchcfg-test-{}", std::process::id()));
        write_config(&dir, r#"{"username":"Steve","player_id":"1234","access_token":"tok","user_type":"legacy"}"#);
        let config = AndroidLaunchConfig::load(&dir);
        assert_eq!(config.username, "Steve");
        assert_eq!(config.player_id, "1234");
        assert_eq!(config.access_token, "tok");
        assert_eq!(config.user_type, "legacy");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_defaults_to_legacy_player() {
        let dir = std::env::temp_dir().join(format!("launchcfg-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let config = AndroidLaunchConfig::load(&dir);
        assert!(config.username.is_empty());
        assert_eq!(config.user_type, "legacy");
    }

    #[test]
    fn broken_json_defaults() {
        let dir = std::env::temp_dir().join(format!("launchcfg-broken-{}", std::process::id()));
        write_config(&dir, "{not json");
        let config = AndroidLaunchConfig::load(&dir);
        assert_eq!(config.user_type, "legacy");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
