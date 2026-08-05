use crate::net::minecraft::client::renderer::entity::RenderLivingBase::LivingRenderInput;
use crate::net::minecraft::util::ResourceLocation::ResourceLocation;

pub struct RenderArmorStand;

impl RenderArmorStand {
    pub fn texture() -> ResourceLocation {
        ResourceLocation::new("minecraft", "textures/entity/armorstand/wood.png")
    }

    /// MCP `RenderArmorStand#rotateCorpse`: armor stands do not use the
    /// generic death roll. Status opcode 32 starts a five-tick yaw wobble.
    pub fn applyCorpseRotation(
        mut input: LivingRenderInput,
        ticksExisted: i32,
        punchTick: Option<i32>,
        partialTicks: f32,
    ) -> LivingRenderInput {
        input.deathRotation = 0.0;
        if let Some(punchTick) = punchTick {
            let elapsed = (ticksExisted - punchTick) as f32 + partialTicks.clamp(0.0, 1.0);
            if (0.0..5.0).contains(&elapsed) {
                let wobble = (elapsed / 1.5 * std::f32::consts::PI).sin() * 3.0;
                input.bodyYaw -= wobble;
            }
        }
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punch_status_applies_only_first_five_ticks_and_clears_death_roll() {
        let input = LivingRenderInput {
            position: [0.0; 3], bodyYaw: 90.0, headYaw: 90.0, headPitch: 0.0,
            limbSwing: 0.0, limbSwingAmount: 0.0, ageInTicks: 0.0,
            swingProgress: 0.0, sneaking: false, child: false,
            deathRotation: 90.0, preScale: 1.0,
            preScaleXYZ: [1.0; 3],
            childLayout: crate::net::minecraft::client::renderer::entity::RenderLivingBase::LivingChildLayout::BIPED,
            adultTranslation: [0.0; 3],
        };
        let active = RenderArmorStand::applyCorpseRotation(input, 102, Some(100), 0.5);
        assert_eq!(active.deathRotation, 0.0);
        assert_ne!(active.bodyYaw, 90.0);
        let expired = RenderArmorStand::applyCorpseRotation(input, 105, Some(100), 0.0);
        assert_eq!(expired.bodyYaw, 90.0);
    }
}
