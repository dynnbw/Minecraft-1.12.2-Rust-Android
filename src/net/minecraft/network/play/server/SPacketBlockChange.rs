use crate::net::minecraft::block::state::IBlockState::IBlockState;
use crate::net::minecraft::network::Packet::RawPacket;
use crate::net::minecraft::network::PacketBuffer::{read_i64_be, read_var_i32, CodecError};
use crate::net::minecraft::util::math::BlockPos::BlockPos;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SPacketBlockChange {
    blockPosition: BlockPos,
    blockState: IBlockState,
}
impl SPacketBlockChange {
    pub fn readPacketData(packet:&RawPacket)->Result<Self,CodecError>{
        let mut input=packet.payload.as_slice();
        let blockPosition=BlockPos::from_long(read_i64_be(&mut input)?);
        let blockState=IBlockState::fromGlobalStateId(read_var_i32(&mut input)?);
        if !input.is_empty(){return Err(CodecError::InvalidData(format!("{} trailing BlockChange bytes",input.len())));}
        Ok(Self{blockPosition,blockState})
    }
    pub const fn getBlockState(&self)->IBlockState{self.blockState}
    pub const fn getBlockPosition(&self)->BlockPos{self.blockPosition}
}
