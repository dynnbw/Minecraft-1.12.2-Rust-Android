use crate::net::minecraft::network::Packet::RawPacket;
use crate::net::minecraft::network::PacketBuffer::{read_i32_be,CodecError};
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct SPacketUnloadChunk{x:i32,z:i32}
impl SPacketUnloadChunk{
    pub fn readPacketData(packet:&RawPacket)->Result<Self,CodecError>{let mut input=packet.payload.as_slice();Ok(Self{x:read_i32_be(&mut input)?,z:read_i32_be(&mut input)?})}
    pub const fn getX(&self)->i32{self.x}
    pub const fn getZ(&self)->i32{self.z}
}
