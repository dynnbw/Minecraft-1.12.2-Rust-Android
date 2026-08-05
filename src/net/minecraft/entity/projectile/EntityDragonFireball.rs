use crate::net::minecraft::entity::projectile::EntityFireball::{
    EntityFireball, FireballParticleType,
};

/// Client-visible semantics owned by MCP 1.12.2 `EntityDragonFireball`.
/// Dragon-breath area-cloud creation remains server-authoritative and pending.
pub struct EntityDragonFireball;

impl EntityDragonFireball {
    pub const WIDTH: f32 = EntityFireball::DEFAULT_WIDTH;
    pub const HEIGHT: f32 = EntityFireball::DEFAULT_HEIGHT;
    pub const FIERY: bool = false;
    pub const COLLIDABLE: bool = false;
    pub const PARTICLE: FireballParticleType = FireballParticleType::DragonBreath;
}
