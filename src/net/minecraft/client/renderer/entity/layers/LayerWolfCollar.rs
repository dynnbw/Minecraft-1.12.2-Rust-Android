use crate::net::minecraft::client::entity::EntityOtherClient::EntityOtherClient;
use crate::net::minecraft::util::ResourceLocation::ResourceLocation;

pub struct LayerWolfCollar;
impl LayerWolfCollar {
    pub fn shouldRender(entity: &EntityOtherClient) -> bool {
        entity.tameableTamed() && (entity.dataManager.byte(0, 0) & 0x20) == 0
    }
    pub fn texture() -> ResourceLocation {
        ResourceLocation::new("minecraft", "textures/entity/wolf/wolf_collar.png")
    }
    pub fn color(entity: &EntityOtherClient) -> [f32; 4] {
        // EnumDyeColor.func_193349_f, indexed by dye damage.
        const COLORS: [[f32; 3]; 16] = [
            [0.9764706, 1.0, 0.99607843],
            [0.9764706, 0.5019608, 0.11372549],
            [0.78039217, 0.30588236, 0.7411765],
            [0.22745098, 0.7019608, 0.85490197],
            [0.99607843, 0.84705883, 0.23921569],
            [0.5019608, 0.78039217, 0.12156863],
            [0.9529412, 0.54509807, 0.6666667],
            [0.2784314, 0.30980393, 0.32156864],
            [0.6156863, 0.6156863, 0.5921569],
            [0.08627451, 0.6117647, 0.6117647],
            [0.5372549, 0.19607843, 0.72156864],
            [0.23529412, 0.26666668, 0.6666667],
            [0.5137255, 0.32941177, 0.19607843],
            [0.36862746, 0.4862745, 0.08627451],
            [0.6901961, 0.18039216, 0.14901961],
            [0.11372549, 0.11372549, 0.12941177],
        ];
        let c = COLORS[entity.wolfCollarColor() as usize & 15];
        [c[0], c[1], c[2], 1.0]
    }
}
