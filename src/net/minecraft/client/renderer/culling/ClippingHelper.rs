#[derive(Debug, Clone, Copy)]
pub struct ClippingHelper {
    pub frustum: [[f32; 4]; 6],
}

impl ClippingHelper {
    pub const fn new(frustum: [[f32; 4]; 6]) -> Self {
        Self { frustum }
    }

    /// MCP `ClippingHelper.isBoxInFrustum`: a box is outside only when all
    /// eight corners are on the non-positive side of one clipping plane.
    pub fn isBoxInFrustum(
        &self,
        minX: f64,
        minY: f64,
        minZ: f64,
        maxX: f64,
        maxY: f64,
        maxZ: f64,
    ) -> bool {
        let corners = [
            [minX, minY, minZ],
            [maxX, minY, minZ],
            [minX, maxY, minZ],
            [maxX, maxY, minZ],
            [minX, minY, maxZ],
            [maxX, minY, maxZ],
            [minX, maxY, maxZ],
            [maxX, maxY, maxZ],
        ];
        self.frustum.iter().all(|plane| {
            corners.iter().any(|corner| {
                plane[0] as f64 * corner[0]
                    + plane[1] as f64 * corner[1]
                    + plane[2] as f64 * corner[2]
                    + plane[3] as f64
                    > 0.0
            })
        })
    }
}
