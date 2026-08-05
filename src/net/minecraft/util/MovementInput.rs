/// Port of MCP `net.minecraft.util.MovementInput`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MovementInput {
    /// Positive values strafe left; negative values strafe right.
    pub moveStrafe: f32,
    /// MCP 1.12.2 field name for forward/back movement.
    pub field_192832_b: f32,
    pub forwardKeyDown: bool,
    pub backKeyDown: bool,
    pub leftKeyDown: bool,
    pub rightKeyDown: bool,
    pub jump: bool,
    pub sneak: bool,
}
