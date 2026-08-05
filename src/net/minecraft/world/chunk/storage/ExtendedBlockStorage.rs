use crate::net::minecraft::world::chunk::BlockStateContainer::BlockStateContainer;
use crate::net::minecraft::world::chunk::NibbleArray::NibbleArray;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtendedBlockStorage {
    yBase: i32,
    data: BlockStateContainer,
    blocklightArray: NibbleArray,
    skylightArray: Option<NibbleArray>,
}
impl ExtendedBlockStorage {
    pub fn fromNetwork(y: i32, data: BlockStateContainer, blocklightArray: NibbleArray, skylightArray: Option<NibbleArray>) -> Self { Self { yBase:y, data, blocklightArray, skylightArray } }
    pub fn getGlobalStateId(&self,x:usize,y:usize,z:usize)->i32{self.data.getGlobalStateId(x,y,z)}
    pub fn getExtBlocklightValue(&self,x:usize,y:usize,z:usize)->u8{self.blocklightArray.get(x,y,z)}
    pub fn getExtSkylightValue(&self,x:usize,y:usize,z:usize)->u8{self.skylightArray.as_ref().map(|a|a.get(x,y,z)).unwrap_or(0)}
    pub const fn getYLocation(&self)->i32{self.yBase}
    pub fn getData(&self)->&BlockStateContainer{&self.data}
    pub fn getDataMut(&mut self)->&mut BlockStateContainer{&mut self.data}
}
