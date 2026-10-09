// game/effects.go: soft all the way to the centre, with the same alpha cutoff.
import bevy_pbr::render::forward_io::VertexOutput;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let r = length(in.uv * 2.0 - 1.0);
    let a = in.color.a * pow(1.0 - smoothstep(0.0, 1.0, r), 1.6);
    if a < 0.01 { discard; }
    // Vertex RGB is already converted from Go's sRGB bytes to Bevy linear.
    // No additional lighting, fog, exposure or tone mapping is applied here.
    return vec4(in.color.rgb, a);
}
