use std::f32::consts::PI;

/// Rendering-relevant subset of MCP 1.12.2 `WorldProvider`.
///
/// The complete provider hierarchy will grow with the remaining dimension and
/// world systems. This stage ports only behaviour directly consumed by the
/// multiplayer light pipeline: sky presence, brightness table and celestial
/// angle.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldProvider {
    dimension: i32,
    lightBrightnessTable: [f32; 16],
}

impl WorldProvider {
    pub fn new(dimension: i32) -> Self {
        let minimum = if dimension == -1 { 0.1 } else { 0.0 };
        let mut lightBrightnessTable = [0.0; 16];
        for (level, brightness) in lightBrightnessTable.iter_mut().enumerate() {
            let inverse = 1.0 - level as f32 / 15.0;
            *brightness =
                (1.0 - inverse) / (inverse * 3.0 + 1.0) * (1.0 - minimum) + minimum;
        }
        Self {
            dimension,
            lightBrightnessTable,
        }
    }

    pub const fn getDimension(&self) -> i32 {
        self.dimension
    }

    /// MCP `WorldProvider.func_191066_m`: true only when the provider's
    /// `createBiomeProvider` enables the skylight storage flag. In vanilla
    /// 1.12.2 this is the surface provider only; both the Nether and the End
    /// leave the flag false, so their chunk sections omit the 2048-byte sky
    /// nibble array.
    pub const fn hasSkyLight(&self) -> bool {
        self.dimension == 0
    }

    pub const fn getLightBrightnessTable(&self) -> &[f32; 16] {
        &self.lightBrightnessTable
    }

    /// MCP `WorldProvider.calculateCelestialAngle`, including fixed Nether/End
    /// overrides from `WorldProviderHell` and `WorldProviderEnd`.
    pub fn calculateCelestialAngle(&self, worldTime: i64, partialTicks: f32) -> f32 {
        match self.dimension {
            -1 => 0.5,
            1 => 0.0,
            _ => {
                let mut angle =
                    (worldTime.rem_euclid(24_000) as f32 + partialTicks) / 24_000.0 - 0.25;
                if angle < 0.0 {
                    angle += 1.0;
                }
                if angle > 1.0 {
                    angle -= 1.0;
                }
                let eased = 1.0 - ((angle * PI).cos() + 1.0) / 2.0;
                angle + (eased - angle) / 3.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_table_matches_vanilla_endpoints() {
        let overworld = WorldProvider::new(0);
        assert!((overworld.getLightBrightnessTable()[0] - 0.0).abs() < 1.0e-6);
        assert!((overworld.getLightBrightnessTable()[15] - 1.0).abs() < 1.0e-6);
        let nether = WorldProvider::new(-1);
        assert!((nether.getLightBrightnessTable()[0] - 0.1).abs() < 1.0e-6);
        assert!((nether.getLightBrightnessTable()[15] - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn fixed_dimension_angles_match_provider_overrides() {
        assert_eq!(
            WorldProvider::new(-1).calculateCelestialAngle(12_000, 0.0),
            0.5
        );
        assert_eq!(
            WorldProvider::new(1).calculateCelestialAngle(12_000, 0.0),
            0.0
        );
    }

    #[test]
    fn only_surface_provider_has_chunk_skylight_arrays() {
        assert!(WorldProvider::new(0).hasSkyLight());
        assert!(!WorldProvider::new(-1).hasSkyLight());
        assert!(!WorldProvider::new(1).hasSkyLight());
    }
}
