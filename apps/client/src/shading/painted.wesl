// The toon shader (the Go shading/toon.go, with illusion's render/shader.go
// lighting and shading/lamps.go): light and shadow in hard bands (the
// ground smoothly), see-through what's less than opaque (dithered), shaded
// faces tinted, cast shadows darkened to black with hard edges, an ambient
// light from the sky above and the ground below, zones lit their own way
// (inside the Hull, under the dome), lamps' light in soft-edged pools,
// glowing what glows, a rim of light on the lit side of rounded things, the
// brightest colours rolled off rather than clipped, and haze with distance.
//
// The Go shader worked on the textures' 8-bit sRGB values as they came, so
// this one encodes the renderer's linear colours to sRGB before the maths
// and decodes its result after; the camera is left untonemapped.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_types,
    mesh_view_bindings::{view, lights, clustered_lights},
    mesh_view_types,
    clustered_forward as clustering,
    shadows,
    forward_io::{VertexOutput, FragmentOutput},
};

struct Painted {
    shadow_color: vec4<f32>, // rgb, w = softness: how wide the bands' edges are
    rim_color: vec4<f32>, // rgb, w = rim power
    fog_color: vec4<f32>, // rgb, w = how far the haze goes a share 1 - 1/e of the way; 0 for none
    ground_fill: vec4<f32>, // the light off the ground, for what faces down; w = 1 to light from sky and ground, 0 for ambient alone
    ambient: vec4<f32>, // rgb, w = exposure
    sun_color: vec4<f32>, // rgb, w = knee: past here colours roll off towards white; 0 for none
    sun_dir: vec4<f32>, // xyz = the way the sun shines, w = midBand: where the half-lit band ends
    params: vec4<f32>, // x = rim threshold, y = fog end (past here, none: the sky's out there), z = veil (in a storm, this much dust over the sky), w = zone count
    flags: vec4<u32>, // x = 1 for the ground, lit smoothly; y = spot count
    zone_min: array<vec4<f32>, 4>, // xyz, w = blend
    zone_max: array<vec4<f32>, 4>,
    zone_ambient: array<vec4<f32>, 4>,
    zone_sun: array<vec4<f32>, 4>,
    spot_pos: array<vec4<f32>, 8>, // position and range
    spot_dir: array<vec4<f32>, 8>,
    spot_color: array<vec4<f32>, 8>,
    spot_cone: array<vec4<f32>, 8>, // cosines of outer and inner half angles
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> painted: Painted;

const SMOOTH: u32 = 1u;

// to_srgb and to_linear are the sRGB transfer curve and its inverse.
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

// roll_off brings c, past knee, back under 1 by its brightest channel, so it
// keeps its hue rather than clipping to yellow or white.
fn roll_off(c: vec3<f32>) -> vec3<f32> {
    let knee = painted.sun_color.w;
    let m = max(max(c.r, c.g), c.b);
    if (knee <= 0.0 || m <= knee) {
        return c;
    }
    let s = 1.0 - knee;
    return c * (knee + s * (1.0 - exp(-(m - knee) / s))) / m;
}

// hash2 and grain_noise are value noise on the ground's plane, for the
// sand's grain and ripples up close. The hash is of whole lattice points,
// so it holds kilometres out, where a float one breaks into streaks.
fn hash2(i: vec2<i32>) -> f32 {
    let q = bitcast<vec2<u32>>(i);
    var h = (q.x * 1597334677u) ^ (q.y * 3812015801u);
    h ^= h >> 16u;
    h *= 2246822519u;
    h ^= h >> 13u;
    return f32(h & 16777215u) / 16777215.0;
}

fn grain_noise(p: vec2<f32>) -> f32 {
    let c = floor(p);
    let f = p - c;
    let i = vec2<i32>(c);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash2(i), hash2(i + vec2<i32>(1, 0)), u.x),
        mix(hash2(i + vec2<i32>(0, 1)), hash2(i + vec2<i32>(1, 1)), u.x), u.y);
}

// bayer4 is a 4×4 ordered-dither threshold for the pixel at p, 1/32 to
// 31/32: steadier as the camera moves than noise.
fn bayer4(p: vec2<f32>) -> f32 {
    var m = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    let i = vec2<i32>(floor(p) % 4.0);
    return (m[i.x + 4 * i.y] + 0.5) / 16.0;
}

// sun_shadow is how much of the sun reaches pos, a point on a surface
// facing n, from 0 (none) to 1 (all): the renderer's shadow map, when the
// sun casts shadows.
fn sun_shadow(pos: vec4<f32>, n: vec3<f32>, view_z: f32, frag_coord: vec2<f32>) -> f32 {
    if (lights.n_directional_lights == 0u) {
        return 1.0;
    }
    if ((lights.directional_lights[0].flags & mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) == 0u) {
        return 1.0;
    }
    return shadows::fetch_directional_shadow(0u, pos, n, view_z, frag_coord);
}

// world_lamps is the lamps' light at pos: each a pool of light, brighter
// nearer in, its edge soft. The lamps are the renderer's point lights,
// every one, found by the cluster the fragment's in.
fn world_lamps(pos: vec3<f32>, n: vec3<f32>, view_z: f32, frag_coord: vec2<f32>) -> vec3<f32> {
    let is_orthographic = view.clip_from_view[3].w == 1.0;
    let cluster_index = clustering::view_fragment_cluster_index(frag_coord, view_z, is_orthographic);
    let ranges = clustering::unpack_clusterable_object_index_ranges(cluster_index);
    var lamps = vec3(0.0);
    for (var i = ranges.first_point_light_index_offset; i < ranges.first_spot_light_index_offset; i = i + 1u) {
        let id = clustering::get_clusterable_object_id(i);
        let range = clustered_lights.data[id].range;
        let to = clustered_lights.data[id].position_radius.xyz - pos;
        let d = length(to);
        if (d >= range) {
            continue;
        }
        let fall = 1.0 - d / range;
        let a = fall * fall * max(dot(n, to / max(d, 0.0001)), 0.0);
        lamps += clustered_lights.data[id].color_inverse_square_range.rgb
            * (0.45 * smoothstep(0.0, 0.12, a) + 0.55 * smoothstep(0.15, 0.5, a));
    }
    return lamps;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var out: FragmentOutput;
    let pbr_input = pbr_input_from_standard_material(in, is_front);
    let linear = pbr_input.material.base_color;
    var base = vec4(to_srgb(linear.rgb), linear.a);
    // What's less than opaque is drawn as that share of its pixels, in a
    // fine screen-space pattern: clear glass and the soft edges of hair
    // cards, without sorting what's drawn. The shadow map still cuts at a
    // half, so glass casts none. The largest threshold is 31/32. Above it
    // no pixel can be discarded, so opaque surfaces skip the lookup.
    if (base.a < 0.96875 && base.a < bayer4(in.position.xy)) {
        discard;
    }
    let fog_color = painted.fog_color.rgb;
    let fog_distance = painted.fog_color.w;
    let fog_end = painted.params.y;
    let veil = painted.params.z;
    if ((pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) != 0u) {
        // The sky and the sun (all that's unlit), behind the storm's dust.
        out.color = vec4(to_linear(mix(base.rgb, fog_color, veil)), 1.0);
        return out;
    }
    let softness = painted.shadow_color.w;
    let mid_band = painted.sun_dir.w;
    let light_dir = painted.sun_dir.xyz;
    let pos = in.world_position.xyz;
    let n = normalize(pbr_input.world_normal);
    let facing = dot(n, -light_dir);
    // The ground's material is marked: lit smoothly, its slopes shading
    // off gently. Everything else in two bands: out of the shadow, and
    // into the full light at midBand.
    let smooth_lit = (painted.flags.x & SMOOTH) != 0u;
    if (smooth_lit) {
        // Up close, the sand: ripples the wind's laid across it, bent by
        // the noise, and its grain; gone with distance, where they'd
        // shimmer. Each fades out as it gets finer than a few pixels,
        // rather than aliasing into streaks.
        let g = pos.xz;
        let phase = dot(g, vec2(0.83, 0.55)) * 9.0 + grain_noise(g * 0.45) * 7.0;
        let ripple = sin(phase) * (1.0 - smoothstep(0.12, 0.45, fwidth(phase)));
        let px = length(fwidth(g));
        var grain = 0.0;
        if (px < 0.06) {
            grain = (grain_noise(g * 9.0) - 0.5) * (1.0 - smoothstep(0.02, 0.06, px));
            if (px < 0.02) {
                grain += 0.5 * (grain_noise(g * 23.0) - 0.5) * (1.0 - smoothstep(0.008, 0.02, px));
            }
        }
        let flat = smoothstep(0.85, 0.98, n.y);
        base = vec4(base.rgb * (1.0 + 0.05 * ripple * flat + 0.12 * grain), base.a);
    }
    var lit = 0.0;
    if (smooth_lit) {
        lit = smoothstep(-0.25, 0.45, facing);
    } else {
        lit = 0.5 * smoothstep(-softness, softness, facing)
            + 0.5 * smoothstep(mid_band - softness, mid_band + softness, facing);
    }
    let view_z = dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), in.world_position);
    // In the shadow of something nearer the sun: as hard an edge as the
    // bands'.
    let visibility = smoothstep(0.25, 0.75, sun_shadow(in.world_position, n, view_z, in.position.xy));
    lit *= visibility;

    // The fill: from the sky above and the ground below. Then the zones,
    // each its own fill and its own share of the sun.
    let ambient = painted.ambient.rgb;
    var fill = ambient;
    if (painted.ground_fill.w > 0.5) {
        fill = mix(painted.ground_fill.rgb, ambient, 0.5 + 0.5 * n.y);
    }
    let light_color = painted.sun_color.rgb;
    var sun = light_color;
    let zone_count = painted.params.w;
    for (var i = 0; i < 4; i = i + 1) {
        if (f32(i) >= zone_count) {
            break;
        }
        let lo = painted.zone_min[i].xyz;
        let blend = vec3(painted.zone_min[i].w);
        let hi = painted.zone_max[i].xyz;
        let in3 = smoothstep(lo, lo + blend, pos) * (1.0 - smoothstep(hi - blend, hi, pos));
        let w = in3.x * in3.y * in3.z;
        fill = mix(fill, painted.zone_ambient[i].rgb * (0.8 + 0.2 * n.y), w);
        sun = mix(sun, light_color * painted.zone_sun[i].rgb, w);
    }
    // Cast shadows darken the fill too, rather than painting the ground
    // with the sky's blue or the shaded faces' violet. Lamps and emissive
    // surfaces still add their own light below.
    var light = mix(fill * painted.shadow_color.rgb * visibility, fill + sun, lit);

    // The lamps: fainter in the sun, which outshines them.
    light += world_lamps(pos, n, view_z, in.position.xy) * (1.0 - 0.75 * lit);
    // Headlamps: forward cones with soft edges and distance falloff. They
    // illuminate even a cast shadow, independently of the sun.
    let spot_count = painted.flags.y;
    for (var i = 0u; i < 8u; i = i + 1u) {
        if (i >= spot_count) {
            break;
        }
        let from_light = pos - painted.spot_pos[i].xyz;
        let d = length(from_light);
        let direction = from_light / max(d, 0.0001);
        let cone = smoothstep(painted.spot_cone[i].x, painted.spot_cone[i].y, dot(direction, painted.spot_dir[i].xyz));
        let fall = max(1.0 - d / painted.spot_pos[i].w, 0.0);
        light += painted.spot_color[i].rgb * cone * fall * fall * max(dot(n, -direction), 0.0);
    }
    // What glows is lit at least as brightly as it glows.
    light = max(light, to_srgb(pbr_input.material.emissive.rgb));

    // The rim, on what's lit. Flat things (the ground, walls) have none:
    // theirs would be a band across the whole face.
    var rim = 0.0;
    let rim_color = painted.rim_color.rgb;
    if (any(rim_color > vec3(0.0))) {
        let edge = pow(1.0 - max(dot(n, normalize(view.world_position - pos)), 0.0), painted.rim_color.w);
        let rounded = step(0.0001, length(fwidth(n)));
        rim = smoothstep(painted.params.x, painted.params.x + softness, edge) * lit * rounded;
    }

    var color = roll_off(base.rgb * (light + rim_color * rim) * painted.ambient.w);
    if (fog_distance > 0.0) {
        let d = length(view.world_position - pos);
        var haze = veil;
        if (d < fog_end) {
            haze = 1.0 - exp(-d / fog_distance);
        }
        color = mix(color, fog_color, haze);
    }
    out.color = vec4(to_linear(color), 1.0);
    return out;
}
