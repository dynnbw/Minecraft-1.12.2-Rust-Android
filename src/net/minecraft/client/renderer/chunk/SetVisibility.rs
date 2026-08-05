use crate::net::minecraft::util::EnumFacing::EnumFacing;

/// Rust port of MCP 1.12.2 `SetVisibility`.
///
/// The six section boundary faces form a symmetric 6 x 6 visibility matrix.
/// A 64-bit mask keeps the exact class responsibility while avoiding heap
/// allocation in every compiled RenderChunk.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SetVisibility {
    bits: u64,
}

impl SetVisibility {
    const COUNT_FACES: usize = 6;

    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    pub const fn allVisible() -> Self {
        Self { bits: u64::MAX }
    }

    pub fn setManyVisible<I>(&mut self, facings: I)
    where
        I: IntoIterator<Item = EnumFacing>,
    {
        let facings = facings.into_iter().collect::<Vec<_>>();
        for first in &facings {
            for second in &facings {
                self.setVisible(*first, *second, true);
            }
        }
    }

    pub fn setVisible(&mut self, first: EnumFacing, second: EnumFacing, visible: bool) {
        let first_bit = Self::bit_index(first, second);
        let second_bit = Self::bit_index(second, first);
        self.setBit(first_bit, visible);
        self.setBit(second_bit, visible);
    }

    pub fn setAllVisible(&mut self, visible: bool) {
        self.bits = if visible { u64::MAX } else { 0 };
    }

    pub const fn isVisible(self, first: EnumFacing, second: EnumFacing) -> bool {
        let bit = Self::bit_index(first, second);
        (self.bits & (1_u64 << bit)) != 0
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }

    const fn bit_index(first: EnumFacing, second: EnumFacing) -> usize {
        first.index() as usize + second.index() as usize * Self::COUNT_FACES
    }

    fn setBit(&mut self, bit: usize, visible: bool) {
        if visible {
            self.bits |= 1_u64 << bit;
        } else {
            self.bits &= !(1_u64 << bit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_is_symmetric() {
        let mut visibility = SetVisibility::new();
        visibility.setVisible(EnumFacing::North, EnumFacing::Up, true);
        assert!(visibility.isVisible(EnumFacing::North, EnumFacing::Up));
        assert!(visibility.isVisible(EnumFacing::Up, EnumFacing::North));
        assert!(!visibility.isVisible(EnumFacing::South, EnumFacing::Up));
    }

    #[test]
    fn all_visible_covers_all_face_pairs() {
        let visibility = SetVisibility::allVisible();
        for first in EnumFacing::VALUES {
            for second in EnumFacing::VALUES {
                assert!(visibility.isVisible(first, second));
            }
        }
    }
}
