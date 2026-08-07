//! Bedrock-style touch layer configuration. Persisted to options.txt
//! under the `touchEnabled` key.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TouchConfig {
    pub enabled: bool,
}

impl Default for TouchConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl TouchConfig {
    pub fn write_lines(&self, lines: &mut Vec<String>) {
        lines.push(format!("touchEnabled={}", self.enabled));
    }

    pub fn read_lines(lines: &[String]) -> Self {
        let mut config = Self::default();
        for line in lines {
            let Some((key, value)) = line.split_once('=') else { continue; };
            if key == "touchEnabled" {
                config.enabled = value.trim() == "true";
            }
        }
        config
    }
}

#[cfg(test)]
mod tests {
    use super::TouchConfig;

    fn roundtrip(config: &TouchConfig) -> TouchConfig {
        let mut lines = Vec::new();
        config.write_lines(&mut lines);
        TouchConfig::read_lines(&lines)
    }

    #[test]
    fn defaults_disabled() {
        let config = TouchConfig::default();
        assert!(!config.enabled);
    }

    #[test]
    fn roundtrip_preserves_enabled() {
        let mut config = TouchConfig::default();
        config.enabled = true;
        assert!(roundtrip(&config).enabled);
    }
}
