//! Bedrock-style touch layer (Android). Controls map directly onto the
//! vanilla 1.12.2 keyboard bindings (MovementKeyState / KeyBinding
//! semantics); layout follows the Bedrock default touch layout.

#[path = "TouchConfig.rs"] pub mod TouchConfig;

/// Runtime state of the active touch layer. Created lazily on the first
/// enabled touch and dropped when the app suspends.
#[cfg(target_os = "android")]
pub struct TouchRuntime;

#[cfg(target_os = "android")]
impl TouchRuntime {
    pub fn new() -> Self { Self }
}
