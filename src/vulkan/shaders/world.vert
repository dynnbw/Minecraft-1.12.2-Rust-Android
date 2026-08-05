#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color;
layout(location = 3) in vec2 in_lightmap;

layout(push_constant) uniform WorldPushConstants {
    mat4 view_projection;
    vec4 camera_position;
    vec4 fog_color;
    vec4 fog_parameters;
    vec4 lightmap_parameters;
} world;

layout(location = 0) out vec2 vertex_uv;
layout(location = 1) out vec4 vertex_color;
layout(location = 2) out float vertex_fog_distance;
layout(location = 3) out vec2 vertex_lightmap;

void main() {
    gl_Position = world.view_projection * vec4(in_position, 1.0);
    vertex_uv = in_uv;
    vertex_color = in_color;
    vertex_fog_distance = distance(in_position, world.camera_position.xyz);
    vertex_lightmap = in_lightmap;
}
