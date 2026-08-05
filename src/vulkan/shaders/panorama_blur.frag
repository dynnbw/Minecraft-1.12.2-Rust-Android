#version 450
layout(set = 0, binding = 0) uniform sampler2D panorama_source;
layout(location = 0) in vec2 vertex_uv;
layout(location = 0) out vec4 fragment_color;
layout(push_constant) uniform BlurPush {
    ivec4 values;
} blur;
vec3 quantize_rgb(vec3 color) {
    return floor(clamp(color, 0.0, 1.0) * 255.0 + 0.5) / 255.0;
}
void main() {
    vec4 color = texture(panorama_source, vertex_uv);
    int count = max(blur.values.x, 0);
    for (int layer = 0; layer < count; ++layer) {
        float alpha = 1.0 / float(layer + 1);
        float offset = (float(layer) - 1.0) / 256.0;
        vec4 sampled = texture(panorama_source, vec2(vertex_uv.x + offset, vertex_uv.y));
        color.rgb = quantize_rgb(sampled.rgb * alpha + color.rgb * (1.0 - alpha));
    }
    fragment_color = vec4(color.rgb, 1.0);
}
