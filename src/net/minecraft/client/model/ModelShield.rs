use crate::net::minecraft::client::model::ModelBoxGeometry::{
    model_box_geometry, MODEL_BOX_FACE_INDICES,
};

/// CPU geometry port of MCP 1.12.2 `ModelShield`.
///
/// Positions are emitted after the exact TEISR `scale(1, -1, -1)` transform
/// and are therefore ready for `RenderItem`'s enclosing model transform.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShieldVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShieldMesh {
    pub vertices: Vec<ShieldVertex>,
    pub indices: Vec<u32>,
}

pub struct ModelShield;

impl ModelShield {
    pub fn buildMesh() -> ShieldMesh {
        let mut mesh = ShieldMesh {
            vertices: Vec::with_capacity(48),
            indices: Vec::with_capacity(72),
        };
        // ModelShield.plate: texture offset (0, 0), 12x22x1.
        add_box(&mut mesh, [0, 0], [-6.0, -11.0, -2.0], [12, 22, 1]);
        // ModelShield.handle: texture offset (26, 0), 2x6x6.
        add_box(&mut mesh, [26, 0], [-1.0, -3.0, -1.0], [2, 6, 6]);
        mesh
    }
}

fn add_box(mesh: &mut ShieldMesh, texture: [i32; 2], origin: [f32; 3], size: [i32; 3]) {
    let geometry = model_box_geometry(texture, origin, size, 0.0, false, 64.0, 64.0);
    let base = mesh.vertices.len() as u32;
    mesh.vertices.reserve(geometry.len());
    for vertex in geometry.iter() {
        let point = vertex.position;
        // ModelRenderer.render(scale=0.0625), then TEISR scale(1,-1,-1).
        mesh.vertices.push(ShieldVertex {
            position: [point[0] * 0.0625, -point[1] * 0.0625, -point[2] * 0.0625],
            uv: vertex.uv,
        });
    }
    mesh.indices.reserve(MODEL_BOX_FACE_INDICES.len());
    mesh.indices
        .extend(MODEL_BOX_FACE_INDICES.iter().map(|index| base + index));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shield_contains_two_six_face_model_boxes() {
        let mesh = ModelShield::buildMesh();
        assert_eq!(mesh.vertices.len(), 2 * 6 * 4);
        assert_eq!(mesh.indices.len(), 2 * 6 * 6);
    }
}
