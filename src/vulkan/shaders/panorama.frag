#version 450
layout(set = 0, binding = 0) uniform sampler2D panorama_face_0;
layout(set = 0, binding = 1) uniform sampler2D panorama_face_1;
layout(set = 0, binding = 2) uniform sampler2D panorama_face_2;
layout(set = 0, binding = 3) uniform sampler2D panorama_face_3;
layout(set = 0, binding = 4) uniform sampler2D panorama_face_4;
layout(set = 0, binding = 5) uniform sampler2D panorama_face_5;
layout(location = 0) in vec2 vertex_uv;
layout(location = 0) out vec4 fragment_color;
layout(push_constant) uniform PanoramaPush {
    float pitch_radians;
    float yaw_radians;
    int sample_count;
    int padding;
} panorama;
vec4 sample_face(int face, vec2 uv) {
    if (face == 0) return texture(panorama_face_0, uv);
    if (face == 1) return texture(panorama_face_1, uv);
    if (face == 2) return texture(panorama_face_2, uv);
    if (face == 3) return texture(panorama_face_3, uv);
    if (face == 4) return texture(panorama_face_4, uv);
    return texture(panorama_face_5, uv);
}
vec3 rotate_inverse(vec3 value, float sin_pitch, float cos_pitch, float sin_yaw, float cos_yaw) {
    float x1 = value.x;
    float y1 = cos_pitch * value.y + sin_pitch * value.z;
    float z1 = -sin_pitch * value.y + cos_pitch * value.z;
    return vec3(cos_yaw * x1 - sin_yaw * z1, y1, sin_yaw * x1 + cos_yaw * z1);
}
void intersect_cube(vec3 origin, vec3 direction, out int face, out vec2 uv) {
    float distance_value = 1e20;
    int axis = 2;
    for (int candidate = 0; candidate < 3; ++candidate) {
        float component = direction[candidate];
        if (abs(component) < 1e-8) continue;
        float boundary = component >= 0.0 ? 1.0 : -1.0;
        float value = (boundary - origin[candidate]) / component;
        if (value > 0.0 && value < distance_value) {
            distance_value = value;
            axis = candidate;
        }
    }
    vec3 point = origin + direction * distance_value;
    vec2 local;
    if (axis == 0 && point.x >= 0.0) { face = 1; local = vec2(-point.z, point.y); }
    else if (axis == 0) { face = 3; local = vec2(point.z, point.y); }
    else if (axis == 1 && point.y >= 0.0) { face = 5; local = vec2(point.x, -point.z); }
    else if (axis == 1) { face = 4; local = vec2(point.x, point.z); }
    else if (point.z >= 0.0) { face = 0; local = vec2(point.x, point.y); }
    else { face = 2; local = vec2(-point.x, point.y); }
    uv = (local + vec2(1.0)) * 0.5;
}
vec3 quantize_rgb(vec3 color) {
    return floor(clamp(color, 0.0, 1.0) * 255.0 + 0.5) / 255.0;
}
void main() {
    float tangent = tan(radians(60.0));
    vec2 panorama_uv = vec2(vertex_uv.x, 1.0 - vertex_uv.y);
    vec2 normalized = panorama_uv * 2.0 - 1.0;
    vec3 base_ray = vec3(-normalized.y * tangent, -normalized.x * tangent, 1.0);
    float sin_pitch = sin(panorama.pitch_radians);
    float cos_pitch = cos(panorama.pitch_radians);
    float sin_yaw = sin(panorama.yaw_radians);
    float cos_yaw = cos(panorama.yaw_radians);
    vec3 direction = rotate_inverse(base_ray, sin_pitch, cos_pitch, sin_yaw, cos_yaw);
    vec3 accumulated = vec3(0.0);
    int count = max(panorama.sample_count, 0);
    for (int k = 0; k < count; ++k) {
        float translate_x = ((float(k % 8) / 8.0) - 0.5) / 64.0;
        float translate_y = ((float(k / 8) / 8.0) - 0.5) / 64.0;
        vec3 translated = rotate_inverse(vec3(-translate_x, -translate_y, 0.0),
                                         sin_pitch, cos_pitch, sin_yaw, cos_yaw);
        int face;
        vec2 uv;
        intersect_cube(translated, direction, face, uv);
        float alpha = float(255 / (k + 1)) / 255.0;
        vec3 sampled = sample_face(face, uv).rgb;
        accumulated = quantize_rgb(sampled * alpha + accumulated * (1.0 - alpha));
    }
    fragment_color = vec4(accumulated, 1.0);
}
