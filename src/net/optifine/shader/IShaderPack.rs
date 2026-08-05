use std::io;

/// Rust equivalent of OptiFine 1.12.2 `IShaderPack`.
///
/// Java returns a nullable `InputStream`. Rust returns owned bytes so the
/// caller cannot outlive a borrowed `ZipArchive`; `Ok(None)` is the exact
/// equivalent of Java's `null`. Implementations intentionally suppress
/// per-resource open errors, matching OptiFine's pack classes.
pub trait IShaderPack {
    fn getName(&self) -> &str;
    fn getResourceAsStream(&mut self, resName: &str) -> io::Result<Option<Vec<u8>>>;
    fn hasDirectory(&mut self, name: &str) -> bool;
    fn close(&mut self);
}
