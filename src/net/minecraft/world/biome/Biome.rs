use crate::net::minecraft::util::math::BlockPos::BlockPos;
use crate::net::minecraft::world::ColorizerFoliage::ColorizerFoliage;
use crate::net::minecraft::world::ColorizerGrass::ColorizerGrass;
use crate::net::minecraft::world::gen::NoiseGeneratorPerlin::NoiseGeneratorPerlin;
use crate::compat::Java::JavaRandom;
use std::sync::OnceLock;

/// Rendering subset of MCP 1.12.2 `Biome`.
///
/// The table below preserves the registered vanilla biome temperature,
/// rainfall, water colour and the source-confirmed colour overrides used by
/// the block colour pipeline. Terrain generation and spawning remain outside
/// this client-side port.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Biome {
    id: u8,
    temperature: f32,
    rainfall: f32,
    waterColor: i32,
    colorKind: BiomeColorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BiomeColorKind {
    Default,
    Swamp,
    RoofedForest,
    Mesa,
}

impl Biome {
    const fn new(
        id: u8,
        temperature: f32,
        rainfall: f32,
        waterColor: i32,
        colorKind: BiomeColorKind,
    ) -> Self {
        Self { id, temperature, rainfall, waterColor, colorKind }
    }

    pub fn getBiome(id: u8) -> Self {
        // Values mirror `Biome.registerBiomes`. Mutated biomes inherit the
        // source biome's colour parameters unless their registration supplies
        // replacements.
        let (temperature, rainfall, waterColor, colorKind) = match id {
            1 | 16 | 129 => (0.8, 0.4, 0xFFFFFF, BiomeColorKind::Default),
            2 | 17 | 130 => (2.0, 0.0, 0xFFFFFF, BiomeColorKind::Default),
            3 | 20 | 25 | 34 | 131 | 162 => (0.2, 0.3, 0xFFFFFF, BiomeColorKind::Default),
            4 | 18 | 132 => (0.7, 0.8, 0xFFFFFF, BiomeColorKind::Default),
            5 | 19 | 133 => (0.25, 0.8, 0xFFFFFF, BiomeColorKind::Default),
            6 | 134 => (0.8, 0.9, 14_745_518, BiomeColorKind::Swamp),
            8 => (2.0, 0.0, 0xFFFFFF, BiomeColorKind::Default),
            10 | 11 | 12 | 13 | 140 => (0.0, 0.5, 0xFFFFFF, BiomeColorKind::Default),
            14 | 15 => (0.9, 1.0, 0xFFFFFF, BiomeColorKind::Default),
            21 | 22 | 149 => (0.95, 0.9, 0xFFFFFF, BiomeColorKind::Default),
            23 | 151 => (0.95, 0.8, 0xFFFFFF, BiomeColorKind::Default),
            26 => (0.05, 0.3, 0xFFFFFF, BiomeColorKind::Default),
            27 | 28 | 155 | 156 => (0.6, 0.6, 0xFFFFFF, BiomeColorKind::Default),
            29 | 157 => (0.7, 0.8, 0xFFFFFF, BiomeColorKind::RoofedForest),
            30 | 31 | 158 => (-0.5, 0.4, 0xFFFFFF, BiomeColorKind::Default),
            32 | 33 => (0.3, 0.8, 0xFFFFFF, BiomeColorKind::Default),
            160 | 161 => (0.25, 0.8, 0xFFFFFF, BiomeColorKind::Default),
            35 => (1.2, 0.0, 0xFFFFFF, BiomeColorKind::Default),
            36 | 164 => (1.0, 0.0, 0xFFFFFF, BiomeColorKind::Default),
            163 => (1.1, 0.0, 0xFFFFFF, BiomeColorKind::Default),
            37..=39 | 165..=167 => (2.0, 0.0, 0xFFFFFF, BiomeColorKind::Mesa),
            // Ocean, river, sky, void and unspecified registry holes use
            // BiomeProperties' vanilla defaults.
            _ => (0.5, 0.5, 0xFFFFFF, BiomeColorKind::Default),
        };
        Self::new(id, temperature, rainfall, waterColor, colorKind)
    }

    /// MCP `BiomeProperties#biomeName` from the vanilla 1.12.2 registry.
    pub const fn getBiomeName(self) -> &'static str {
        match self.id {
            0 => "Ocean", 1 => "Plains", 2 => "Desert", 3 => "Extreme Hills",
            4 => "Forest", 5 => "Taiga", 6 => "Swampland", 7 => "River",
            8 => "Hell", 9 => "The End", 10 => "FrozenOcean", 11 => "FrozenRiver",
            12 => "Ice Plains", 13 => "Ice Mountains", 14 => "MushroomIsland",
            15 => "MushroomIslandShore", 16 => "Beach", 17 => "DesertHills",
            18 => "ForestHills", 19 => "TaigaHills", 20 => "Extreme Hills Edge",
            21 => "Jungle", 22 => "JungleHills", 23 => "JungleEdge",
            24 => "Deep Ocean", 25 => "Stone Beach", 26 => "Cold Beach",
            27 => "Birch Forest", 28 => "Birch Forest Hills", 29 => "Roofed Forest",
            30 => "Cold Taiga", 31 => "Cold Taiga Hills", 32 => "Mega Taiga",
            33 => "Mega Taiga Hills", 34 => "Extreme Hills+", 35 => "Savanna",
            36 => "Savanna Plateau", 37 => "Mesa", 38 => "Mesa Plateau F",
            39 => "Mesa Plateau", 127 => "The Void", 129 => "Sunflower Plains",
            130 => "Desert M", 131 => "Extreme Hills M", 132 => "Flower Forest",
            133 => "Taiga M", 134 => "Swampland M", 140 => "Ice Plains Spikes",
            149 => "Jungle M", 151 => "JungleEdge M", 155 => "Birch Forest M",
            156 => "Birch Forest Hills M", 157 => "Roofed Forest M",
            158 => "Cold Taiga M", 160 => "Mega Spruce Taiga",
            161 => "Redwood Taiga Hills M", 162 => "Extreme Hills+ M",
            163 => "Savanna M", 164 => "Savanna Plateau M", 165 => "Mesa (Bryce)",
            166 => "Mesa Plateau F M", 167 => "Mesa Plateau M",
            _ => "Ocean",
        }
    }

    pub const fn getId(self) -> u8 { self.id }
    pub const fn getRainfall(self) -> f32 { self.rainfall }
    pub const fn getWaterColor(self) -> i32 { self.waterColor }

    /// MCP 1.12.2 `Biome#getSkyColorByTemp`.
    pub fn getSkyColorByTemp(self, currentTemperature: f32) -> i32 {
        let temperature = (currentTemperature / 3.0).clamp(-1.0, 1.0);
        hsv_to_rgb(
            0.622_222_24 - temperature * 0.05,
            0.5 + temperature * 0.1,
            1.0,
        )
    }

    /// Source-equivalent base branch of `Biome.getFloatTemperature`.
    /// The high-altitude noise term is deliberately isolated until
    /// `NoiseGeneratorPerlin` is ported; below y=65 this is exact.
    pub fn getFloatTemperature(self, pos: BlockPos) -> f32 {
        if pos.y <= 64 {
            self.temperature
        } else {
            let noise = temperature_noise().getValue(pos.x as f64 / 8.0, pos.z as f64 / 8.0);
            let perturbation = (noise * 4.0) as f32;
            self.temperature - (perturbation + pos.y as f32 - 64.0) * 0.05 / 30.0
        }
    }

    pub fn getGrassColorAtPos(
        self,
        pos: BlockPos,
        grass: &ColorizerGrass,
    ) -> i32 {
        match self.colorKind {
            BiomeColorKind::Swamp => {
                let noise = grass_color_noise().getValue(pos.x as f64 * 0.0225, pos.z as f64 * 0.0225);
                if noise < -0.1 { 5_011_004 } else { 6_975_545 }
            }
            BiomeColorKind::Mesa => 9_470_285,
            BiomeColorKind::RoofedForest => {
                let base = grass.getGrassColor(
                    self.getFloatTemperature(pos).clamp(0.0, 1.0) as f64,
                    self.rainfall.clamp(0.0, 1.0) as f64,
                );
                ((base & 16_711_422) + 2_634_762) >> 1
            }
            BiomeColorKind::Default => grass.getGrassColor(
                self.getFloatTemperature(pos).clamp(0.0, 1.0) as f64,
                self.rainfall.clamp(0.0, 1.0) as f64,
            ),
        }
    }

    pub fn getFoliageColorAtPos(
        self,
        pos: BlockPos,
        foliage: &ColorizerFoliage,
    ) -> i32 {
        match self.colorKind {
            BiomeColorKind::Swamp => 6_975_545,
            BiomeColorKind::Mesa => 10_387_789,
            _ => foliage.getFoliageColor(
                self.getFloatTemperature(pos).clamp(0.0, 1.0) as f64,
                self.rainfall.clamp(0.0, 1.0) as f64,
            ),
        }
    }
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> i32 {
    let sector = (hue * 6.0) as i32 % 6;
    let fraction = hue * 6.0 - sector as f32;
    let minimum = value * (1.0 - saturation);
    let descending = value * (1.0 - fraction * saturation);
    let ascending = value * (1.0 - (1.0 - fraction) * saturation);
    let (red, green, blue) = match sector {
        0 => (value, ascending, minimum),
        1 => (descending, value, minimum),
        2 => (minimum, value, ascending),
        3 => (minimum, descending, value),
        4 => (ascending, minimum, value),
        5 => (value, minimum, descending),
        _ => unreachable!("hue sector must be 0..5"),
    };
    let red = (red * 255.0) as i32;
    let green = (green * 255.0) as i32;
    let blue = (blue * 255.0) as i32;
    red.clamp(0, 255) << 16 | green.clamp(0, 255) << 8 | blue.clamp(0, 255)
}

static TEMPERATURE_NOISE: OnceLock<NoiseGeneratorPerlin> = OnceLock::new();
static GRASS_COLOR_NOISE: OnceLock<NoiseGeneratorPerlin> = OnceLock::new();

fn temperature_noise() -> &'static NoiseGeneratorPerlin {
    TEMPERATURE_NOISE.get_or_init(|| {
        let mut random = JavaRandom::new(1234);
        NoiseGeneratorPerlin::new(&mut random, 1)
    })
}

fn grass_color_noise() -> &'static NoiseGeneratorPerlin {
    GRASS_COLOR_NOISE.get_or_init(|| {
        let mut random = JavaRandom::new(2345);
        NoiseGeneratorPerlin::new(&mut random, 1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_biome_parameters_match_source_values() {
        let plains = Biome::getBiome(1);
        assert_eq!(plains.getRainfall(), 0.4);
        let swamp = Biome::getBiome(6);
        assert_eq!(swamp.getWaterColor(), 14_745_518);
        assert_eq!(Biome::getBiome(35).getFloatTemperature(BlockPos::ORIGIN), 1.2);
    }

    #[test]
    fn sky_color_uses_source_temperature_hsv_formula() {
        assert_eq!(Biome::getBiome(1).getSkyColorByTemp(0.8), 7_907_327);
    }
}
