#version 450
layout(set = 0, binding = 0) uniform sampler2D gui_texture;
layout(location = 0) in vec2 vertex_uv;
layout(location = 1) in vec4 vertex_color;
layout(location = 0) out vec4 fragment_color;
layout(push_constant) uniform GuiPush {
    vec2 gui_size;
    int use_texture;
    int padding;
} gui;
void main() {
    vec4 sampled = gui.use_texture != 0 ? texture(gui_texture, vertex_uv) : vec4(1.0);
    vec4 color = sampled * vertex_color;
    if (gui.use_texture != 0 && color.a <= 0.1) discard;
    fragment_color = color;
}
