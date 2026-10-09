// The outline shader (the Go shading/outline.go), for a hull drawn with its
// front faces culled: the model's back faces pushed out along their
// normals, an inverted hull, in a dark tint of the surface's own colour.

import bevy_pbr::render::{
    mesh_functions,
    skinning,
    mesh_view_bindings::view,
    forward_io::{Vertex, VertexOutput, decompress_vertex},
};

struct Outline {
    base_color: vec4<f32>, // the surface's own tint
    outline_color: vec4<f32>, // rgb, w = the outline's width in pixels
    fog_color: vec4<f32>, // rgb, w = fog distance
    params: vec4<f32>, // x = fog end, y = reach: how far away the line starts thinning with distance
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> outline: Outline;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var texture_sampler: sampler;

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3(0.0031308));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((max(c, vec3(0.0)) + 0.055) / 1.055, vec3(2.4));
    return select(hi, lo, c <= vec3(0.04045));
}

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);
    // The same pose as the mesh it's round: skinned the same way.
@if(SKINNED)
    let world_from_local = skinning::skin_model(
        vertex.joint_indices,
        vertex.joint_weights,
        vertex_no_morph.instance_index
    );
@else
    let world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);
    var world_normal = vec3(0.0);
@if(VERTEX_NORMALS && SKINNED)
    world_normal = skinning::skin_normals(world_from_local, vertex.normal);
@if(VERTEX_NORMALS && !SKINNED)
    world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_no_morph.instance_index);
    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    var clip = view.clip_from_world * world_position;
    // Outwards on screen, so the line is as wide wherever it is: the
    // width in clip space is two pixels' worth over the viewport.
    let pixel = 2.0 * outline.outline_color.w / view.viewport.zw;
    let along = (view.clip_from_world * vec4(world_normal, 0.0)).xy / pixel;
    if (length(along) > 0.00001) {
        clip = vec4(clip.xy + normalize(along) * pixel * min(clip.w, outline.params.y), clip.zw);
    }
    out.position = clip;
    out.world_position = world_position;
    out.world_normal = world_normal;
@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var base = outline.base_color;
@if(VERTEX_UVS_A)
    base *= textureSample(texture, texture_sampler, in.uv);
    if (base.a < 0.5) {
        discard;
    }
    var color = to_srgb(base.rgb) * outline.outline_color.rgb;
    // Hazed like what it's round (see the toon shader), by its depth.
    let depth = -dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), in.world_position);
    let fog_distance = outline.fog_color.w;
    if (fog_distance > 0.0) {
        color = mix(color, outline.fog_color.rgb, (1.0 - exp(-depth / fog_distance)) * step(depth, outline.params.x));
    }
    return vec4(to_linear(color), 1.0);
}
