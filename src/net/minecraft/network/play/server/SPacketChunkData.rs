use crate::net::minecraft::nbt::NBTTagCompound::NBTTagCompound;
use crate::net::minecraft::network::Packet::RawPacket;
use crate::net::minecraft::network::PacketBuffer::{read_bool, read_i32_be, read_nbt_compound, read_var_i32, CodecError};

#[derive(Debug, Clone, PartialEq)]
pub struct SPacketChunkData {
    chunkX:i32,
    chunkZ:i32,
    availableSections:i32,
    buffer:Vec<u8>,
    tileEntityTags:Vec<NBTTagCompound>,
    loadChunk:bool,
}
impl SPacketChunkData {
    pub fn readPacketData(packet:&RawPacket)->Result<Self,CodecError>{
        let mut input=packet.payload.as_slice();
        let chunkX=read_i32_be(&mut input)?;
        let chunkZ=read_i32_be(&mut input)?;
        let loadChunk=read_bool(&mut input)?;
        let availableSections=read_var_i32(&mut input)?;
        let length=read_var_i32(&mut input)?;
        if length<0{return Err(CodecError::NegativeLength(length));}
        if length>2_097_152{return Err(CodecError::PacketTooLarge{actual:length as usize,maximum:2_097_152});}
        if input.len()<length as usize{return Err(CodecError::UnexpectedEof);}
        let (buffer,remainder)=input.split_at(length as usize); input=remainder;
        let count=read_var_i32(&mut input)?;
        if count<0{return Err(CodecError::NegativeLength(count));}
        let mut tileEntityTags=Vec::with_capacity(count as usize);
        for _ in 0..count { if let Some(tag)=read_nbt_compound(&mut input)?{tileEntityTags.push(tag);} }
        Ok(Self{chunkX,chunkZ,availableSections,buffer:buffer.to_vec(),tileEntityTags,loadChunk})
    }
    pub const fn getChunkX(&self)->i32{self.chunkX}
    pub const fn getChunkZ(&self)->i32{self.chunkZ}
    pub const fn getExtractedSize(&self)->i32{self.availableSections}
    pub const fn doChunkLoad(&self)->bool{self.loadChunk}
    pub fn getReadBuffer(&self)->&[u8]{&self.buffer}
    pub fn getTileEntityTags(&self)->&[NBTTagCompound]{&self.tileEntityTags}
}
