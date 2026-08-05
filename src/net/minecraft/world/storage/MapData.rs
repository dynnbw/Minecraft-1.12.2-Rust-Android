use crate::net::minecraft::world::storage::MapDecoration::MapDecoration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapData {
    pub mapName: String,
    pub scale: i8,
    pub trackingPosition: bool,
    pub colors: Vec<u8>,
    pub mapDecorations: Vec<MapDecoration>,
    pub revision: u64,
}

impl MapData {
    pub const WIDTH: usize = 128;
    pub const HEIGHT: usize = 128;
    pub const PIXEL_COUNT: usize = Self::WIDTH * Self::HEIGHT;

    pub fn new(mapId: i32) -> Self {
        Self {
            mapName: format!("map_{mapId}"),
            scale: 0,
            trackingPosition: false,
            colors: vec![0; Self::PIXEL_COUNT],
            mapDecorations: Vec::new(),
            revision: 0,
        }
    }
}
