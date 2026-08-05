use crate::net::minecraft::entity::projectile::EntityFireball::{
    EntityFireball, FireballParticleType,
};

/// Client-visible semantics owned by MCP 1.12.2 `EntitySmallFireball`.
/// Server-side ignition/damage impact handling remains pending with the
/// authoritative server entity layer.
pub struct EntitySmallFireball;

impl EntitySmallFireball {
    pub const WIDTH: f32 = EntityFireball::SMALL_WIDTH;
    pub const HEIGHT: f32 = EntityFireball::SMALL_HEIGHT;
    pub const FIERY: bool = true;
    pub const COLLIDABLE: bool = false;
    pub const PARTICLE: FireballParticleType = FireballParticleType::SmokeNormal;
}
