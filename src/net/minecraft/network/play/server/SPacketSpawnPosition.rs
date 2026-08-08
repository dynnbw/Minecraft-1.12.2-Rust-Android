use crate::net::minecraft::network::Packet::RawPacket;
use crate::net::minecraft::network::PacketBuffer::{read_i64_be, CodecError};
use crate::net::minecraft::util::math::BlockPos::BlockPos;

/// MCP 1.12.2 `SPacketSpawnPosition` (0x46): the world spawn point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SPacketSpawnPosition {
    pos: BlockPos,
}

impl SPacketSpawnPosition {
    pub fn readPacketData(packet: &RawPacket) -> Result<Self, CodecError> {
        let mut input = packet.payload.as_slice();
        // `PacketBuffer.readBlockPos` is a fixed 8-byte long in 1.12.2.
        let pos = BlockPos::from_long(read_i64_be(&mut input)?);
        if !input.is_empty() {
            return Err(CodecError::InvalidData(format!(
                "{} unread spawn-position bytes",
                input.len()
            )));
        }
        Ok(Self { pos })
    }

    pub const fn getSpawnPos(&self) -> BlockPos { self.pos }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::minecraft::network::PacketBuffer::write_i64_be;

    #[test]
    fn spawn_position_decodes_fixed_long_coordinates() {
        let pos = BlockPos::new(-12, 64, 345);
        let mut payload = Vec::new();
        write_i64_be(pos.to_long(), &mut payload);
        let packet = SPacketSpawnPosition::readPacketData(&RawPacket::new(0x46, payload)).unwrap();
        assert_eq!(packet.getSpawnPos(), pos);
    }
}
