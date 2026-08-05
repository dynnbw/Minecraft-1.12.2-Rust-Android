use crate::net::minecraft::client::model::ModelBiped::{BipedPose, PartPose};

/// MCP 1.12.2 `ModelArmorStandArmor` pose owner. The concrete wood model and
/// future armor-layer models consume the same six synchronized Rotations values.
pub struct ModelArmorStandArmor;

impl ModelArmorStandArmor {
    pub fn pose(
        head: [f32; 3],
        body: [f32; 3],
        leftArm: [f32; 3],
        rightArm: [f32; 3],
        leftLeg: [f32; 3],
        rightLeg: [f32; 3],
    ) -> BipedPose {
        BipedPose {
            head: pose_from_degrees([0.0, 1.0, 0.0], head),
            body: pose_from_degrees([0.0, 0.0, 0.0], body),
            leftArm: pose_from_degrees([5.0, 2.0, 0.0], leftArm),
            rightArm: pose_from_degrees([-5.0, 2.0, 0.0], rightArm),
            leftLeg: pose_from_degrees([1.9, 11.0, 0.0], leftLeg),
            rightLeg: pose_from_degrees([-1.9, 11.0, 0.0], rightLeg),
        }
    }
}

pub(crate) fn pose_from_degrees(pivot: [f32; 3], rotations: [f32; 3]) -> PartPose {
    PartPose {
        pivot,
        rotation: [
            rotations[0].to_radians(),
            rotations[1].to_radians(),
            rotations[2].to_radians(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armor_stand_metadata_rotations_are_degrees_and_use_source_pivots() {
        let pose = ModelArmorStandArmor::pose(
            [10.0, 20.0, 30.0],
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
        );
        assert_eq!(pose.head.pivot, [0.0, 1.0, 0.0]);
        assert!((pose.head.rotation[1] - 20.0_f32.to_radians()).abs() < 1.0e-6);
        assert_eq!(pose.rightLeg.pivot, [-1.9, 11.0, 0.0]);
    }
}
