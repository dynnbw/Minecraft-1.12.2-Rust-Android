use crate::net::minecraft::network::Packet::RawPacket;
use crate::net::minecraft::network::PacketBuffer::{read_string, CodecError};
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct SPacketCustomPayload{channel:String,data:Vec<u8>}
impl SPacketCustomPayload{
 pub fn readPacketData(packet:&RawPacket)->Result<Self,CodecError>{let mut input=packet.payload.as_slice();let channel=read_string(&mut input,20)?;if input.len()>1_048_576{return Err(CodecError::PacketTooLarge{actual:input.len(),maximum:1_048_576});}Ok(Self{channel,data:input.to_vec()})}
 pub fn getChannelName(&self)->&str{&self.channel} pub fn getBufferData(&self)->&[u8]{&self.data}
}
