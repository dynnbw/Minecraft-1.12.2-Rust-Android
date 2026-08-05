use rand::Rng;

use crate::net::minecraft::entity::Entity::Entity;

/// Client-visible state of MCP 1.12.2 `EntityLightningBolt`.
///
/// Block ignition and server-side strike damage are deliberately absent on the
/// remote client, exactly as guarded by `!world.isRemote` in the source. Sound
/// dispatch and `RenderLightningBolt` remain separate audio/rendering ports.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityLightningBolt {
    pub entity: Entity,
    pub entityId: i32,
    pub lightningState: i32,
    pub boltVertex: i64,
    pub boltLivingTime: i32,
    pub effectOnly: bool,
}

impl EntityLightningBolt {
    pub fn new(entityId: i32, x: f64, y: f64, z: f64, effectOnly: bool) -> Self {
        let mut random = rand::thread_rng();
        let mut entity = Entity::default();
        entity.setPositionAndRotation(x, y, z, 0.0, 0.0);
        Self {
            entity,
            entityId,
            lightningState: 2,
            boltVertex: random.gen(),
            boltLivingTime: random.gen_range(1..=3),
            effectOnly,
        }
    }

    /// Port of the client-relevant branch of `EntityLightningBolt.onUpdate`.
    /// Returns true while vanilla would set `World.lastLightningBolt` to 2.
    pub fn onUpdate(&mut self) -> bool {
        self.entity.ticksExisted = self.entity.ticksExisted.wrapping_add(1);
        self.entity.prevPosX = self.entity.posX;
        self.entity.prevPosY = self.entity.posY;
        self.entity.prevPosZ = self.entity.posZ;

        self.lightningState -= 1;
        if self.lightningState < 0 {
            if self.boltLivingTime == 0 {
                self.entity.isDead = true;
            } else {
                let threshold = -(rand::thread_rng().gen_range(0..10) as i32);
                if self.lightningState < threshold {
                    self.boltLivingTime -= 1;
                    self.lightningState = 1;
                    // MCP only changes boltVertex here on the server branch.
                }
            }
        }
        self.lightningState >= 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_matches_vanilla_initial_state_ranges() {
        let bolt = EntityLightningBolt::new(7, 1.0, 64.0, 2.0, false);
        assert_eq!(bolt.entityId, 7);
        assert_eq!(bolt.lightningState, 2);
        assert!((1..=3).contains(&bolt.boltLivingTime));
        assert_eq!(bolt.entity.posY, 64.0);
    }
}
