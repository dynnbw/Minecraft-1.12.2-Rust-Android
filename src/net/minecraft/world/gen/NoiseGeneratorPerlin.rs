use crate::compat::Java::JavaRandom;
use crate::net::minecraft::world::gen::NoiseGeneratorSimplex::NoiseGeneratorSimplex;

/// MCP 1.12.2 `NoiseGeneratorPerlin` value path.
#[derive(Debug, Clone)]
pub struct NoiseGeneratorPerlin {
    noiseLevels: Vec<NoiseGeneratorSimplex>,
    levels: usize,
}

impl NoiseGeneratorPerlin {
    pub fn new(random: &mut JavaRandom, levelsIn: usize) -> Self {
        let noiseLevels = (0..levelsIn)
            .map(|_| NoiseGeneratorSimplex::new(random))
            .collect();
        Self { noiseLevels, levels: levelsIn }
    }

    pub fn getValue(&self, x: f64, y: f64) -> f64 {
        let mut result = 0.0;
        let mut scale = 1.0;
        for level in 0..self.levels {
            result += self.noiseLevels[level].getValue(x * scale, y * scale) / scale;
            scale /= 2.0;
        }
        result
    }
}
