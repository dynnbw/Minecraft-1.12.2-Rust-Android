use crate::net::minecraft::client::model::ModelSign::ModelSign;
use crate::net::minecraft::client::model::ModelVehicleBox::VehicleModelMesh;
use crate::net::minecraft::util::ResourceLocation::ResourceLocation;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignPlacement {
    pub standing: bool,
    pub yawDegrees: f32,
    pub wallOffset: [f32; 3],
}

/// Vulkan semantic owner for MCP 1.12.2 `TileEntitySignRenderer`.
pub struct TileEntitySignRenderer;

impl TileEntitySignRenderer {
    pub fn texture() -> ResourceLocation {
        ResourceLocation::parse("textures/entity/sign.png")
    }

    pub fn placement(blockId: i32, metadata: i32) -> SignPlacement {
        if blockId == 63 {
            SignPlacement {
                standing: true,
                yawDegrees: -((metadata & 15) as f32 * 360.0 / 16.0),
                wallOffset: [0.0, 0.0, 0.0],
            }
        } else {
            let sourceYaw = match metadata {
                2 => 180.0,
                4 => 90.0,
                5 => -90.0,
                _ => 0.0,
            };
            SignPlacement {
                standing: false,
                yawDegrees: -sourceYaw,
                wallOffset: [0.0, -0.3125, -0.4375],
            }
        }
    }

    pub fn buildMesh(standing: bool) -> VehicleModelMesh {
        ModelSign::buildMesh(standing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_source_rotation_tables() {
        assert_eq!(TileEntitySignRenderer::placement(63, 4).yawDegrees, -90.0);
        assert_eq!(TileEntitySignRenderer::placement(68, 2).yawDegrees, -180.0);
        assert_eq!(TileEntitySignRenderer::placement(68, 4).yawDegrees, -90.0);
        assert_eq!(TileEntitySignRenderer::placement(68, 5).yawDegrees, 90.0);
    }
}
