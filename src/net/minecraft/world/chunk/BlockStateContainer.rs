use crate::net::minecraft::network::PacketBuffer::{read_i64_be, read_u8, read_var_i32, CodecError};
use crate::net::minecraft::util::BitArray::BitArray;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Palette { Local(Vec<i32>), Registry }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockStateContainer {
    storage: BitArray,
    palette: Palette,
    bits: usize,
}

impl Default for BlockStateContainer {
    fn default() -> Self { Self::new() }
}

impl BlockStateContainer {
    pub fn new() -> Self {
        Self {
            storage: BitArray::new(4, 4096).expect("fixed BlockStateContainer dimensions"),
            palette: Palette::Local(vec![0]),
            bits: 4,
        }
    }

    pub fn read(buf: &mut &[u8]) -> Result<Self, CodecError> {
        let packetBits = read_u8(buf)? as usize;
        let bits = if packetBits <= 4 { 4 } else { packetBits };
        let palette = if bits <= 8 {
            let count = read_var_i32(buf)?;
            if count < 0 { return Err(CodecError::NegativeLength(count)); }
            let mut entries = Vec::with_capacity(count as usize);
            for _ in 0..count { entries.push(read_var_i32(buf)?); }
            Palette::Local(entries)
        } else { Palette::Registry };
        let longCount = read_var_i32(buf)?;
        if longCount < 0 { return Err(CodecError::NegativeLength(longCount)); }
        let mut longs = Vec::with_capacity(longCount as usize);
        for _ in 0..longCount { longs.push(read_i64_be(buf)? as u64); }
        let storage = BitArray::fromBacking(bits, 4096, longs)
            .map_err(CodecError::InvalidData)?;
        Ok(Self { storage, palette, bits })
    }

    const fn getIndex(x: usize, y: usize, z: usize) -> usize { y << 8 | z << 4 | x }

    pub fn getGlobalStateId(&self, x: usize, y: usize, z: usize) -> i32 {
        self.getGlobalStateIdAt(Self::getIndex(x, y, z))
    }

    pub fn getGlobalStateIdAt(&self, index: usize) -> i32 {
        let value = self.storage.getAt(index).unwrap_or(0) as usize;
        match &self.palette {
            Palette::Local(entries) => entries.get(value).copied().unwrap_or(0),
            Palette::Registry => value as i32,
        }
    }

    /// Mutable equivalent of MCP `BlockStateContainer.set`. Palette growth and
    /// the transition to the global registry preserve the Java container rules.
    pub fn setGlobalStateId(&mut self, x: usize, y: usize, z: usize, stateId: i32) -> Result<i32, String> {
        self.setGlobalStateIdAt(Self::getIndex(x, y, z), stateId)
    }

    pub fn setGlobalStateIdAt(&mut self, index: usize, stateId: i32) -> Result<i32, String> {
        if index >= 4096 { return Err(format!("index out of bounds: {index}")); }
        let stateId = stateId.max(0);
        let old = self.getGlobalStateIdAt(index);
        let paletteIndex = match &mut self.palette {
            Palette::Registry => Some(stateId as u32),
            Palette::Local(entries) => {
                if let Some(existing) = entries.iter().position(|entry| *entry == stateId) {
                    Some(existing as u32)
                } else if entries.len() < (1_usize << self.bits) {
                    entries.push(stateId);
                    Some((entries.len() - 1) as u32)
                } else {
                    None
                }
            }
        };
        // End the mutable palette borrow before resizing the whole container.
        let paletteIndex = match paletteIndex {
            Some(index) => index,
            None => self.resizeForState(stateId)?,
        };
        self.storage.setAt(index, paletteIndex)?;
        Ok(old)
    }

    fn resizeForState(&mut self, stateId: i32) -> Result<u32, String> {
        let oldValues = (0..4096).map(|index| self.getGlobalStateIdAt(index)).collect::<Vec<_>>();
        if self.bits < 8 {
            let mut entries = match &self.palette {
                Palette::Local(entries) => entries.clone(),
                Palette::Registry => unreachable!(),
            };
            entries.push(stateId);
            self.bits += 1;
            self.palette = Palette::Local(entries.clone());
            self.storage = BitArray::new(self.bits, 4096)?;
            for (index, value) in oldValues.into_iter().enumerate() {
                let paletteIndex = entries.iter().position(|entry| *entry == value).unwrap_or(0) as u32;
                self.storage.setAt(index, paletteIndex)?;
            }
            Ok((entries.len() - 1) as u32)
        } else {
            let maximum = oldValues.iter().copied().chain(std::iter::once(stateId)).max().unwrap_or(0) as u32;
            self.bits = (32 - maximum.leading_zeros()).max(9) as usize;
            self.palette = Palette::Registry;
            self.storage = BitArray::new(self.bits, 4096)?;
            for (index, value) in oldValues.into_iter().enumerate() {
                self.storage.setAt(index, value.max(0) as u32)?;
            }
            Ok(stateId as u32)
        }
    }

    pub fn bits(&self) -> usize { self.bits }
    pub fn palette(&self) -> &Palette { &self.palette }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_palette_grows_and_preserves_existing_states() {
        let mut container = BlockStateContainer::new();
        for index in 0..20 {
            container.setGlobalStateIdAt(index, index as i32 + 1).unwrap();
        }
        assert!(container.bits() >= 5);
        for index in 0..20 { assert_eq!(container.getGlobalStateIdAt(index), index as i32 + 1); }
    }

    #[test]
    fn palette_switches_to_registry_after_eight_bits() {
        let mut container = BlockStateContainer::new();
        for index in 0..300 {
            container.setGlobalStateIdAt(index, index as i32 + 1).unwrap();
        }
        assert!(matches!(container.palette(), Palette::Registry));
        assert_eq!(container.getGlobalStateIdAt(299), 300);
    }
}
