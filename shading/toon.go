package shading

// The toon shader: light and shadow in hard bands (the ground smoothly),
// see-through what's less than opaque (dithered),
// shadows tinted rather than darkened, the sun's shadows cut as hard, an
// ambient light from the sky above and the ground below, zones lit their own
// way (inside the Hull, under the dome), lamps' light in soft-edged pools,
// glowing what glows, a rim of light on the lit side of rounded things, the
// brightest colours rolled off rather than clipped, and haze with distance.

const toonFragment = lightingGLSL + `
in vec3 fragPosition;
in vec2 fragTexCoord;
in vec4 fragColor;
in vec3 fragNormal;

uniform sampler2D texture0;
uniform vec4 colDiffuse;
uniform vec3 lightDir;
uniform vec3 lightColor;
uniform vec3 ambient;
uniform vec3 viewPos;
uniform float unlit;

uniform vec3 shadowColor;
uniform float softness;
uniform float midBand;
uniform vec3 rimColor;
uniform float rimPower;
uniform float rimThreshold;
uniform vec3 fogColor;
uniform float fogDistance; // how far the haze goes a share 1 - 1/e of the way; 0 for none
uniform float fogEnd; // past here, none: the sky's out there
uniform float veil; // but in a storm, this much: the sky, the sun and the planets dimmed by dust
uniform vec3 groundFill; // the light off the ground, for what faces down
uniform float hemisphere; // 1 to light from sky and ground, 0 for ambient alone
uniform float exposure;
uniform float knee; // past here colours roll off towards white; 0 for none
uniform float zoneCount;
uniform vec3 zoneMin[4];
uniform vec3 zoneMax[4];
uniform float zoneBlend[4];
uniform vec3 zoneAmbient[4];
uniform vec3 zoneSun[4];

out vec4 finalColor;

// rollOff brings c, past knee, back under 1 by its brightest channel, so it
// keeps its hue rather than clipping to yellow or white.
vec3 rollOff(vec3 c) {
    float m = max(max(c.r, c.g), c.b);
    if (knee <= 0.0 || m <= knee) {
        return c;
    }
    float s = 1.0 - knee;
    return c * (knee + s * (1.0 - exp(-(m - knee) / s))) / m;
}

// hash2 and grainNoise are value noise on the ground's plane, for the
// sand's grain and ripples up close. The hash is of whole lattice points,
// so it holds kilometres out, where a float one breaks into streaks.
float hash2(ivec2 i) {
    uvec2 q = uvec2(i);
    uint h = q.x * 1597334677u ^ q.y * 3812015801u;
    h ^= h >> 16u;
    h *= 2246822519u;
    h ^= h >> 13u;
    return float(h & 16777215u) / 16777215.0;
}

float grainNoise(vec2 p) {
    vec2 c = floor(p);
    vec2 f = p - c;
    ivec2 i = ivec2(c);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash2(i), hash2(i + ivec2(1, 0)), u.x),
        mix(hash2(i + ivec2(0, 1)), hash2(i + ivec2(1, 1)), u.x), u.y);
}

// bayer4 is a 4×4 ordered-dither threshold for the pixel at p, 1/32 to
// 31/32: steadier as the camera moves than noise.
float bayer4(vec2 p) {
    const float m[16] = float[16](0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    ivec2 i = ivec2(mod(floor(p), 4.0));
    return (m[i.x + 4 * i.y] + 0.5) / 16.0;
}

void main() {
    vec4 base = texture(texture0, fragTexCoord) * colDiffuse * fragColor;
    // What's less than opaque is drawn as that share of its pixels, in a
    // fine screen-space pattern: clear glass (a third, so the dome's seen
    // through) and the soft edges of hair cards, without sorting what's
    // drawn. The shadow map still cuts at a half, so glass casts none.
    // The largest threshold is 31/32. Above it no pixel can be discarded,
    // so opaque surfaces skip the pattern lookup with identical coverage.
    if (base.a < 0.96875 && base.a < bayer4(gl_FragCoord.xy)) {
        discard;
    }
    if (unlit > 0.5) {
        // The sky and the sun (all that's unlit), behind the storm's dust.
        finalColor = vec4(mix(base.rgb, fogColor, veil), 1.0);
        return;
    }
    vec3 n = normalize(fragNormal);
    float facing = dot(n, -lightDir);
    // The ground's material is marked (shading.Smooth): lit smoothly, its
    // slopes shading off gently. Everything else in two bands: out of the
    // shadow, and into the full light at midBand.
    bool smoothLit = colDiffuse.a < 0.999;
    if (smoothLit) {
        // Up close, the sand: ripples the wind's laid across it, bent by
        // the noise, and its grain; gone with distance, where they'd
        // shimmer.
        // Each fades out as it gets finer than a few pixels, rather than
        // aliasing into streaks.
        vec2 g = fragPosition.xz;
        float phase = dot(g, vec2(0.83, 0.55)) * 9.0 + grainNoise(g * 0.45) * 7.0;
        float ripple = sin(phase) * (1.0 - smoothstep(0.12, 0.45, fwidth(phase)));
        float px = length(fwidth(g));
        float grain = 0.0;
        if (px < 0.06) {
            grain = (grainNoise(g * 9.0) - 0.5) * (1.0 - smoothstep(0.02, 0.06, px));
            if (px < 0.02) {
                grain += 0.5 * (grainNoise(g * 23.0) - 0.5) * (1.0 - smoothstep(0.008, 0.02, px));
            }
        }
        float flat_ = smoothstep(0.85, 0.98, n.y);
        base.rgb *= 1.0 + 0.05 * ripple * flat_ + 0.12 * grain;
    }
    float lit = smoothLit ? smoothstep(-0.25, 0.45, facing)
        : 0.5 * smoothstep(-softness, softness, facing)
        + 0.5 * smoothstep(midBand - softness, midBand + softness, facing);
    // In the shadow of something nearer the sun: as hard an edge as the
    // bands'.
    lit *= smoothstep(0.25, 0.75, sunShadow(fragPosition, n, -lightDir));

    // The fill: from the sky above and the ground below. Then the zones,
    // each its own fill and its own share of the sun.
    vec3 fill = hemisphere > 0.5 ? mix(groundFill, ambient, 0.5 + 0.5 * n.y) : ambient;
    vec3 sun = lightColor;
    for (int i = 0; i < 4; i++) {
        if (float(i) >= zoneCount) {
            break;
        }
        vec3 in3 = smoothstep(zoneMin[i], zoneMin[i] + zoneBlend[i], fragPosition)
            * (1.0 - smoothstep(zoneMax[i] - zoneBlend[i], zoneMax[i], fragPosition));
        float w = in3.x * in3.y * in3.z;
        fill = mix(fill, zoneAmbient[i] * (0.8 + 0.2 * n.y), w);
        sun = mix(sun, lightColor * zoneSun[i], w);
    }
    vec3 light = mix(fill * shadowColor, fill + sun, lit);

    // The lamps: each a pool of light, brighter nearer in, its edge soft;
    // fainter in the sun, which outshines them.
    vec3 lamps = vec3(0.0);
    for (int i = 0; i < 8; i++) {
        if (float(i) >= pointCount) {
            break;
        }
        float a = pointLight(i, fragPosition, n);
        lamps += pointColor[i] * (0.45 * smoothstep(0.0, 0.12, a) + 0.55 * smoothstep(0.15, 0.5, a));
    }
    light += lamps * (1.0 - 0.75 * lit);
    // What glows is lit at least as brightly as it glows.
    light = max(light, emissive);

    // The rim, on what's lit. Flat things (the ground, walls) have none:
    // theirs would be a band across the whole face.
    float rim = 0.0;
    if (any(greaterThan(rimColor, vec3(0.0)))) {
        float edge = pow(1.0 - max(dot(n, normalize(viewPos - fragPosition)), 0.0), rimPower);
        float rounded = step(0.0001, length(fwidth(n)));
        rim = smoothstep(rimThreshold, rimThreshold + softness, edge) * lit * rounded;
    }

    finalColor = vec4(rollOff(base.rgb * (light + rimColor * rim) * exposure), 1.0);
    if (fogDistance > 0.0) {
        float d = length(viewPos - fragPosition);
        finalColor.rgb = mix(finalColor.rgb, fogColor, d < fogEnd ? 1.0 - exp(-d / fogDistance) : veil);
    }
}
`
