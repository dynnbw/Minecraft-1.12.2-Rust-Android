use crate::net::minecraft::entity::projectile::EntityFireball::{
    EntityFireball, FireballParticleType,
};

/// Client-visible semantics owned by MCP 1.12.2 `EntityLargeFireball`.
/// Server-side impact, explosion and NBT ownership remain intentionally absent
/// until the authoritative server entity layer is ported.
pub struct EntityLargeFireball;

impl EntityLargeFireball {
    pub const WIDTH: f32 = EntityFireball::DEFAULT_WIDTH;
    pub const HEIGHT: f32 = EntityFireball::DEFAULT_HEIGHT;
    pub const FIERY: bool = true;
    pub const COLLIDABLE: bool = true;
    pub const PARTICLE: FireballParticleType = FireballParticleType::SmokeNormal;
}
