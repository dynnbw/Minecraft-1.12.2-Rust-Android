use crate::net::minecraft::entity::Entity::Entity;

/// Client-owned constants and constructor semantics from MCP 1.12.2
/// `EntityFireball` and its four vanilla subclasses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireballParticleType {
    SmokeNormal,
    DragonBreath,
}

pub struct EntityFireball;

impl EntityFireball {
    pub const DEFAULT_WIDTH: f32 = 1.0;
    pub const DEFAULT_HEIGHT: f32 = 1.0;
    pub const SMALL_WIDTH: f32 = 0.3125;
    pub const SMALL_HEIGHT: f32 = 0.3125;
    pub const DEFAULT_MOTION_FACTOR: f64 = 0.95;
    pub const WATER_MOTION_FACTOR: f64 = 0.8;
    pub const INVULNERABLE_WITHER_MOTION_FACTOR: f64 = 0.73;
    pub const ACCELERATION_SCALE: f64 = 0.1;
    pub const PACKED_FULL_BRIGHT: u32 = 15_728_880;

    /// Constructor normalization used by `EntityFireball(World,x,y,z,ax,ay,az)`.
    pub fn normalizedAcceleration(raw: [f64; 3]) -> [f64; 3] {
        let magnitude = (raw[0] * raw[0] + raw[1] * raw[1] + raw[2] * raw[2]).sqrt();
        [
            raw[0] / magnitude * Self::ACCELERATION_SCALE,
            raw[1] / magnitude * Self::ACCELERATION_SCALE,
            raw[2] / magnitude * Self::ACCELERATION_SCALE,
        ]
    }

    /// Exact `EntityFireball#isInRangeToRenderDist` override. The base Entity
    /// range is multiplied by four before the normal 64-block weight.
    pub fn isInRangeToRenderDist(entity: &Entity, distanceSquared: f64) -> bool {
        let mut edge = entity.boundingBox.average_edge_length() * 4.0;
        if edge.is_nan() {
            edge = 4.0;
        }
        let range = edge * 64.0;
        distanceSquared < range * range
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_normalizes_packet_acceleration_to_one_tenth() {
        let acceleration = EntityFireball::normalizedAcceleration([3.0, 4.0, 0.0]);
        assert!((acceleration[0] - 0.06).abs() < 1.0e-12);
        assert!((acceleration[1] - 0.08).abs() < 1.0e-12);
        assert_eq!(acceleration[2], 0.0);
    }

    #[test]
    fn zero_acceleration_retains_java_nan_semantics() {
        let acceleration = EntityFireball::normalizedAcceleration([0.0; 3]);
        assert!(acceleration.into_iter().all(f64::is_nan));
    }
}
