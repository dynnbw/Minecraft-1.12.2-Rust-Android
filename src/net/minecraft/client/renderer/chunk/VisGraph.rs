use std::collections::{HashSet, VecDeque};

use crate::net::minecraft::client::renderer::chunk::SetVisibility::SetVisibility;
use crate::net::minecraft::util::EnumFacing::EnumFacing;

/// Rust port of MCP 1.12.2 `VisGraph` for one 16 x 16 x 16 RenderChunk.
#[derive(Debug, Clone)]
pub struct VisGraph {
    opaque: [u64; 64],
    empty: usize,
}

impl Default for VisGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl VisGraph {
    const BLOCK_COUNT: usize = 16 * 16 * 16;
    const DX: i32 = 1;
    const DZ: i32 = 16;
    const DY: i32 = 256;

    pub const fn new() -> Self {
        Self {
            opaque: [0; 64],
            empty: Self::BLOCK_COUNT,
        }
    }

    pub fn setOpaqueCube(&mut self, x: usize, y: usize, z: usize) {
        let index = Self::getIndex(x, y, z);
        if !Self::get_bit(&self.opaque, index) {
            Self::set_bit(&mut self.opaque, index);
            self.empty = self.empty.saturating_sub(1);
        }
    }

    pub fn computeVisibility(&self) -> SetVisibility {
        let opaque_count = Self::BLOCK_COUNT - self.empty;
        if opaque_count < 256 {
            return SetVisibility::allVisible();
        }
        if self.empty == 0 {
            return SetVisibility::new();
        }

        let mut visibility = SetVisibility::new();
        let mut visited = self.opaque;
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    if x != 0 && x != 15 && y != 0 && y != 15 && z != 0 && z != 15 {
                        continue;
                    }
                    let index = Self::getIndex(x, y, z);
                    if !Self::get_bit(&visited, index) {
                        visibility.setManyVisible(Self::floodFill(index, &mut visited));
                    }
                }
            }
        }
        visibility
    }

    pub fn getVisibleFacings(&self, x: usize, y: usize, z: usize) -> HashSet<EnumFacing> {
        let mut visited = self.opaque;
        let index = Self::getIndex(x & 15, y & 15, z & 15);
        if Self::get_bit(&visited, index) {
            return HashSet::new();
        }
        Self::floodFill(index, &mut visited)
    }

    const fn getIndex(x: usize, y: usize, z: usize) -> usize {
        x | (y << 8) | (z << 4)
    }

    fn floodFill(start: usize, visited: &mut [u64; 64]) -> HashSet<EnumFacing> {
        let mut result = HashSet::new();
        let mut queue = VecDeque::with_capacity(384);
        queue.push_back(start);
        Self::set_bit(visited, start);

        while let Some(index) = queue.pop_front() {
            Self::addEdges(index, &mut result);
            for facing in EnumFacing::VALUES {
                let neighbour = Self::getNeighborIndexAtFace(index, facing);
                if let Some(neighbour) = neighbour {
                    if !Self::get_bit(visited, neighbour) {
                        Self::set_bit(visited, neighbour);
                        queue.push_back(neighbour);
                    }
                }
            }
        }
        result
    }

    fn addEdges(index: usize, result: &mut HashSet<EnumFacing>) {
        let x = index & 15;
        if x == 0 {
            result.insert(EnumFacing::West);
        } else if x == 15 {
            result.insert(EnumFacing::East);
        }

        let y = (index >> 8) & 15;
        if y == 0 {
            result.insert(EnumFacing::Down);
        } else if y == 15 {
            result.insert(EnumFacing::Up);
        }

        let z = (index >> 4) & 15;
        if z == 0 {
            result.insert(EnumFacing::North);
        } else if z == 15 {
            result.insert(EnumFacing::South);
        }
    }

    fn getNeighborIndexAtFace(index: usize, facing: EnumFacing) -> Option<usize> {
        let x = index & 15;
        let y = (index >> 8) & 15;
        let z = (index >> 4) & 15;
        match facing {
            EnumFacing::Down if y > 0 => Some((index as i32 - Self::DY) as usize),
            EnumFacing::Up if y < 15 => Some((index as i32 + Self::DY) as usize),
            EnumFacing::North if z > 0 => Some((index as i32 - Self::DZ) as usize),
            EnumFacing::South if z < 15 => Some((index as i32 + Self::DZ) as usize),
            EnumFacing::West if x > 0 => Some((index as i32 - Self::DX) as usize),
            EnumFacing::East if x < 15 => Some((index as i32 + Self::DX) as usize),
            _ => None,
        }
    }

    const fn get_bit(bits: &[u64; 64], index: usize) -> bool {
        (bits[index >> 6] & (1_u64 << (index & 63))) != 0
    }

    fn set_bit(bits: &mut [u64; 64], index: usize) {
        bits[index >> 6] |= 1_u64 << (index & 63);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_section_is_fully_visible_like_vanilla() {
        let mut graph = VisGraph::new();
        for x in 0..15 {
            graph.setOpaqueCube(x, 0, 0);
        }
        let visibility = graph.computeVisibility();
        assert!(visibility.isVisible(EnumFacing::North, EnumFacing::South));
        assert!(visibility.isVisible(EnumFacing::Down, EnumFacing::Up));
    }

    #[test]
    fn solid_section_has_no_visibility() {
        let mut graph = VisGraph::new();
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    graph.setOpaqueCube(x, y, z);
                }
            }
        }
        let visibility = graph.computeVisibility();
        assert!(!visibility.isVisible(EnumFacing::North, EnumFacing::South));
    }

    #[test]
    fn opaque_wall_separates_east_and_west() {
        let mut graph = VisGraph::new();
        for y in 0..16 {
            for z in 0..16 {
                graph.setOpaqueCube(8, y, z);
            }
        }
        let visibility = graph.computeVisibility();
        assert!(!visibility.isVisible(EnumFacing::West, EnumFacing::East));
        assert!(visibility.isVisible(EnumFacing::West, EnumFacing::North));
    }
}
