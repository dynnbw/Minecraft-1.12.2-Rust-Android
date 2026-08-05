use uuid::Uuid;

use crate::com::mojang::authlib::GameProfile::GameProfile;
use crate::net::minecraft::network::Packet::RawPacket;
use crate::net::minecraft::network::PacketBuffer::{read_string, CodecError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SPacketLoginSuccess { profile: GameProfile }
impl SPacketLoginSuccess {
    pub fn readPacketData(packet: &RawPacket) -> Result<Self, SPacketLoginSuccessError> {
        let mut input = packet.payload.as_slice();
        let uuidText = read_string(&mut input, 36)?;
        let name = read_string(&mut input, 16)?;
        let id = Uuid::parse_str(&uuidText).map_err(|error| SPacketLoginSuccessError::InvalidUuid(error.to_string()))?;
        Ok(Self { profile: GameProfile::new(Some(id), name) })
    }
    pub fn getProfile(&self) -> &GameProfile { &self.profile }
}

#[derive(Debug, thiserror::Error)]
pub enum SPacketLoginSuccessError {
    #[error(transparent)] Codec(#[from] CodecError),
    #[error("invalid login UUID: {0}")] InvalidUuid(String),
}
