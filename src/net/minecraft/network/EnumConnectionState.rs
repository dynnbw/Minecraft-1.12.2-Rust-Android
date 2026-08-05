#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ConnectionState {
    Handshaking = -1,
    Play = 0,
    Status = 1,
    Login = 2,
}
