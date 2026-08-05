use crate::net::minecraft::network::EnumConnectionState::ConnectionState;
use crate::net::minecraft::network::Packet::RawPacket;
use crate::net::minecraft::network::PacketBuffer::{write_string, write_u16_be, write_var_i32, CodecError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct C00Handshake {
    protocolVersion: i32,
    ip: String,
    port: u16,
    requestedState: ConnectionState,
}

impl C00Handshake {
    pub fn new(ip: impl Into<String>, port: u16, requestedState: ConnectionState) -> Self {
        Self { protocolVersion: 340, ip: ip.into(), port, requestedState }
    }
    pub fn writePacketData(&self) -> Result<RawPacket, CodecError> {
        let mut payload = Vec::new();
        write_var_i32(self.protocolVersion, &mut payload);
        write_string(&self.ip, 255, &mut payload)?;
        write_u16_be(self.port, &mut payload);
        write_var_i32(self.requestedState as i32, &mut payload);
        Ok(RawPacket::new(0, payload))
    }
    pub const fn getRequestedState(&self) -> ConnectionState { self.requestedState }
    pub const fn getProtocolVersion(&self) -> i32 { self.protocolVersion }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_340_status_handshake_bytes_match_packetbuffer() {
        let packet = C00Handshake::new("localhost", 25565, ConnectionState::Status).writePacketData().unwrap();
        assert_eq!(packet.id, 0);
        assert_eq!(packet.payload, vec![0xD4, 0x02, 9, b'l', b'o', b'c', b'a', b'l', b'h', b'o', b's', b't', 0x63, 0xDD, 1]);
    }
}
