use crate::net::minecraft::entity::projectile::EntityFireball::{
    EntityFireball, FireballParticleType,
};

/// Client-visible semantics owned by MCP 1.12.2 `EntityWitherSkull`.
/// Impact damage, wither effect and block destruction remain server-owned.
pub struct EntityWitherSkull;

impl EntityWitherSkull {
    pub const WIDTH: f32 = EntityFireball::SMALL_WIDTH;
    pub const HEIGHT: f32 = EntityFireball::SMALL_HEIGHT;
    pub const FIERY: bool = false;
    pub const COLLIDABLE: bool = false;
    pub const PARTICLE: FireballParticleType = FireballParticleType::SmokeNormal;
    pub const INVULNERABLE_DATA_INDEX: u8 = 6;

    pub const fn motionFactor(invulnerable: bool) -> f64 {
        if invulnerable {
            EntityFireball::INVULNERABLE_WITHER_MOTION_FACTOR
        } else {
            EntityFireball::DEFAULT_MOTION_FACTOR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invulnerable_skull_uses_source_motion_factor() {
        assert_eq!(EntityWitherSkull::motionFactor(false), 0.95);
        assert_eq!(EntityWitherSkull::motionFactor(true), 0.73);
    }
}
