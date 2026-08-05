use crate::com::mojang::authlib::GameProfile::GameProfile;
use crate::net::minecraft::network::Packet::RawPacket;
use crate::net::minecraft::network::PacketBuffer::{write_string, CodecError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CPacketLoginStart { profile: GameProfile }

impl CPacketLoginStart {
    pub fn new(profile: GameProfile) -> Self { Self { profile } }
    pub fn writePacketData(&self) -> Result<RawPacket, CodecError> {
        let mut payload = Vec::new();
        write_string(self.profile.getName(), 16, &mut payload)?;
        Ok(RawPacket::new(0, payload))
    }
    pub fn getProfile(&self) -> &GameProfile { &self.profile }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_start_contains_only_the_profile_name_in_1_12_2() {
        let profile = GameProfile::new(None, "TestPlayer");
        let packet = CPacketLoginStart::new(profile).writePacketData().unwrap();
        assert_eq!(packet.id, 0);
        assert_eq!(packet.payload[0], 10);
        assert_eq!(&packet.payload[1..], b"TestPlayer");
    }
}
