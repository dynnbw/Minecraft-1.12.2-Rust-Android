use crate::net::minecraft::network::Packet::RawPacket;
use crate::net::minecraft::network::PacketBuffer::{read_var_i32, read_var_i64, CodecError};
use crate::net::minecraft::util::math::BlockPos::BlockPos;

/// MCP 1.12.2 `SPacketSpawnPosition` (0x46): the world spawn point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SPacketSpawnPosition {
    pos: BlockPos,
}

impl SPacketSpawnPosition {
    pub fn readPacketData(packet: &RawPacket) -> Result<Self, CodecError> {
        let mut input = packet.payload.as_slice();
        // `PacketBuffer.readBlockPos`: x varLong, y varInt, z varLong.
        let x = read_var_i64(&mut input)? as i32;
        let y = read_var_i32(&mut input)?;
        let z = read_var_i64(&mut input)? as i32;
        if !input.is_empty() {
            return Err(CodecError::InvalidData(format!(
                "{} unread spawn-position bytes",
                input.len()
            )));
        }
        Ok(Self {
            pos: BlockPos::new(x, y, z),
        })
    }

    pub const fn getSpawnPos(&self) -> BlockPos {
        self.pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::minecraft::network::PacketBuffer::{write_var_i32, write_var_i64};

    #[test]
    fn spawn_position_decodes_var_long_coordinates() {
        let pos = BlockPos::new(-12, 64, 345);
        let mut payload = Vec::new();
        write_var_i64(pos.x as i64, &mut payload);
        write_var_i32(pos.y, &mut payload);
        write_var_i64(pos.z as i64, &mut payload);
        let packet = SPacketSpawnPosition::readPacketData(&RawPacket::new(0x46, payload)).unwrap();
        assert_eq!(packet.getSpawnPos(), pos);
    }
}
