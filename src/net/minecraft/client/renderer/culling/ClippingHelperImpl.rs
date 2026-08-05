use super::ClippingHelper::ClippingHelper;

pub struct ClippingHelperImpl;

impl ClippingHelperImpl {
    /// Builds the six planes from the row-major Vulkan clip matrix. The side
    /// and far planes are the same row combinations as MCP's OpenGL helper;
    /// Vulkan's zero-to-one depth range uses row 2 directly for the near plane.
    pub fn fromClipMatrix(matrix: [[f32; 4]; 4]) -> ClippingHelper {
        let raw = [
            subtract(matrix[3], matrix[0]),
            add(matrix[3], matrix[0]),
            add(matrix[3], matrix[1]),
            subtract(matrix[3], matrix[1]),
            subtract(matrix[3], matrix[2]),
            matrix[2],
        ];
        ClippingHelper::new(raw.map(normalize))
    }
}

fn add(left: [f32; 4], right: [f32; 4]) -> [f32; 4] {
    [
        left[0] + right[0],
        left[1] + right[1],
        left[2] + right[2],
        left[3] + right[3],
    ]
}

fn subtract(left: [f32; 4], right: [f32; 4]) -> [f32; 4] {
    [
        left[0] - right[0],
        left[1] - right[1],
        left[2] - right[2],
        left[3] - right[3],
    ]
}

fn normalize(plane: [f32; 4]) -> [f32; 4] {
    let length = (plane[0] * plane[0] + plane[1] * plane[1] + plane[2] * plane[2]).sqrt();
    if length <= f32::EPSILON {
        plane
    } else {
        [
            plane[0] / length,
            plane[1] / length,
            plane[2] / length,
            plane[3] / length,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vulkan_zero_to_one_near_plane_rejects_negative_z() {
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let helper = ClippingHelperImpl::fromClipMatrix(identity);
        assert!(helper.isBoxInFrustum(-0.5, -0.5, 0.1, 0.5, 0.5, 0.9));
        assert!(!helper.isBoxInFrustum(-0.5, -0.5, -0.9, 0.5, 0.5, -0.1));
    }
}
