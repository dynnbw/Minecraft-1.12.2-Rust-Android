//! First-launch extractor: copies bundled APK assets to the internal game dir.
//!
//! The packaging script writes two artifacts into APK assets:
//! - `mcassets.list`  : one relative path per line (newline terminated)
//! - `mcassets/<path>`: the actual game assets under runtime/assets
//!
//! Extraction happens once; a marker file skips repeat runs. The game code
//! only uses std::fs paths (File::open / fs::read_to_string), so extraction
//! is the minimal-adaptation path.

use std::io::Read;
use std::path::{Path, PathBuf};

const MARKER: &str = ".assets-extracted-v1";

/// Pure: decide whether extraction is needed for the given marker file.
pub fn needs_extraction(marker: &Path) -> bool {
    !marker.is_file()
}

/// Pure: resolve a manifest entry to its extraction target, rejecting
/// path traversal attempts.
pub fn extraction_target(game_dir: &Path, entry: &str) -> Option<PathBuf> {
    let clean = entry.trim();
    if clean.is_empty() { return None; }
    let path = Path::new(clean);
    if path.is_absolute() || clean.starts_with("..") || clean.contains("..\\") { return None; }
    let assets_dir = game_dir.join("assets");
    let target = assets_dir.join(path);
    if !target.starts_with(&assets_dir) { return None; }
    Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_marker_means_extraction_needed() {
        let marker = std::env::temp_dir().join(format!("marker-{}", std::process::id()));
        assert!(needs_extraction(&marker));
        std::fs::write(&marker, b"done").unwrap();
        assert!(!needs_extraction(&marker));
        let _ = std::fs::remove_file(&marker);
    }

    #[test]
    fn rejects_traversal_entries() {
        let dir = Path::new("/data/game");
        assert!(extraction_target(dir, "minecraft/lang/en_us.lang").is_some());
        assert!(extraction_target(dir, "../evil").is_none());
        assert!(extraction_target(dir, "/absolute/path").is_none());
        assert!(extraction_target(dir, "").is_none());
    }
}

#[cfg(target_os = "android")]
mod platform {
    use super::*;
    use ndk::asset::AssetManager;

    /// Extracts all entries listed in `mcassets.list` from APK assets into
    /// `<game_dir>/assets`. Returns Ok(()) on full success.
    pub fn extract_assets(manager: &AssetManager, game_dir: &Path) -> anyhow::Result<()> {
        use std::ffi::CString;

        let marker = game_dir.join(MARKER);
        if !needs_extraction(&marker) { return Ok(()); }
        let list_name = CString::new("mcassets.list").expect("static name");
        let mut list = manager.open(&list_name)
            .ok_or_else(|| anyhow::anyhow!("APK assets missing mcassets.list (did build_apk.py run?)"))?;
        let mut text = String::new();
        list.read_to_string(&mut text)?;
        let assets_dir = game_dir.join("assets");
        for line in text.lines() {
            let target = extraction_target(game_dir, line)
                .ok_or_else(|| anyhow::anyhow!("bad manifest entry: {line:?}"))?;
            if let Some(parent) = target.parent() { std::fs::create_dir_all(parent)?; }
            let asset_name = CString::new(format!("mcassets/{line}"))
                .map_err(|_| anyhow::anyhow!("asset name contains NUL: {line:?}"))?;
            let mut reader = manager.open(&asset_name)
                .ok_or_else(|| anyhow::anyhow!("missing APK asset: {line}"))?;
            let mut out = std::fs::File::create(&target)?;
            std::io::copy(&mut reader, &mut out)?;
        }
        let _ = std::fs::write(&marker, b"done");
        log::info!("extracted bundled assets to {}", assets_dir.display());
        Ok(())
    }
}

#[cfg(target_os = "android")]
pub use platform::*;
