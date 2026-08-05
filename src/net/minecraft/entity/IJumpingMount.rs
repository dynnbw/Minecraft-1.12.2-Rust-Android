/// MCP 1.12.2 `IJumpingMount` behavior required by `EntityPlayerSP`.
pub trait IJumpingMount {
    fn setJumpPower(&mut self, jumpPowerIn: i32);
    fn canJump(&self) -> bool;
}
