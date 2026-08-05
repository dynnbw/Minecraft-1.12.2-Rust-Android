use super::ClippingHelper::ClippingHelper;

#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    clippingHelper: ClippingHelper,
    xPosition: f64,
    yPosition: f64,
    zPosition: f64,
}

impl Frustum {
    pub const fn new(clippingHelper: ClippingHelper) -> Self {
        Self {
            clippingHelper,
            xPosition: 0.0,
            yPosition: 0.0,
            zPosition: 0.0,
        }
    }

    pub fn setPosition(&mut self, x: f64, y: f64, z: f64) {
        self.xPosition = x;
        self.yPosition = y;
        self.zPosition = z;
    }

    pub fn isBoxInFrustum(
        &self,
        minX: f64,
        minY: f64,
        minZ: f64,
        maxX: f64,
        maxY: f64,
        maxZ: f64,
    ) -> bool {
        self.clippingHelper.isBoxInFrustum(
            minX - self.xPosition,
            minY - self.yPosition,
            minZ - self.zPosition,
            maxX - self.xPosition,
            maxY - self.yPosition,
            maxZ - self.zPosition,
        )
    }
}
