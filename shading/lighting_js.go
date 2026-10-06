package shading

import "github.com/struckchure/illusion/render"

// Keep the renderer's uniforms and four-texel interpolation, with one
// blended shadow tap instead of the desktop's four (4 reads rather than
// 16 per shaded pixel). The unused full filter is removed by the compiler.
const lightingGLSL = "#define sunShadow fullSunShadow\n" + render.LightingGLSL + `
#undef sunShadow
float sunShadow(vec3 pos, vec3 n, vec3 toLight) {
    if (shadowParams.x < 0.5) {
        return 1.0;
    }
    float facing = clamp(dot(n, toLight), 0.0, 1.0);
    vec3 p = pos + n * shadowParams.z * (1.0 + 2.0 * (1.0 - facing));
    vec4 ls = lightSpace * vec4(p, 1.0);
    vec3 c = ls.xyz / ls.w * 0.5 + 0.5;
    vec2 off = abs(c.xy * 2.0 - 1.0);
    float edge = max(off.x, off.y);
    if (edge >= 1.0 || c.z >= 1.0) {
        return 1.0;
    }
    float lit = shadowTap(c.xy, c.z - shadowParams.w);
    return mix(lit, 1.0, smoothstep(0.8, 1.0, edge));
}
`
