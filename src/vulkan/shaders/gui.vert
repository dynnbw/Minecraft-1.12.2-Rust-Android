#version 450
layout(location = 0) in vec3 in_position;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color;
layout(location = 0) out vec2 vertex_uv;
layout(location = 1) out vec4 vertex_color;
layout(push_constant) uniform GuiPush {
    vec2 gui_size;
    int use_texture;
    int padding;
} gui;
void main() {
    vec2 safe_size = max(gui.gui_size, vec2(1.0));
    // Vulkan uses a positive-height viewport in VulkanGuiPipeline. Under the
    // Vulkan viewport transform NDC y=-1 maps to framebuffer row 0, so MCP GUI
    // coordinates (0 at the top) must map directly from -1 to +1. The previous
    // OpenGL-style 1-y conversion vertically mirrored the complete interface.
    vec2 ndc = vec2(in_position.x / safe_size.x * 2.0 - 1.0,
                    in_position.y / safe_size.y * 2.0 - 1.0);
    gl_Position = vec4(ndc, 0.0, 1.0);
    vertex_uv = in_uv;
    vertex_color = in_color;
}
