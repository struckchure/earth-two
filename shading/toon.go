package shading

import "github.com/struckchure/illusion/render"

// The toon shader: light and shadow in hard bands, shadows tinted rather
// than darkened, the sun's shadows cut as hard, lamps' light in pools with
// hard edges, glowing what glows, and a rim of light on the lit side of
// rounded things.

const toonFragment = render.LightingGLSL + `
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

out vec4 finalColor;

void main() {
    vec4 base = texture(texture0, fragTexCoord) * colDiffuse * fragColor;
    // Cut out what's less than half opaque, as the default shader does.
    if (base.a < 0.5) {
        discard;
    }
    if (unlit > 0.5) {
        finalColor = base;
        return;
    }
    vec3 n = normalize(fragNormal);
    float facing = dot(n, -lightDir);
    // Two bands: out of the shadow, and into the full light at midBand.
    float lit = 0.5 * smoothstep(-softness, softness, facing)
        + 0.5 * smoothstep(midBand - softness, midBand + softness, facing);
    // In the shadow of something nearer the sun: as hard an edge as the
    // bands'.
    lit *= smoothstep(0.25, 0.75, sunShadow(fragPosition, n, -lightDir));
    vec3 light = mix(ambient * shadowColor, ambient + lightColor, lit);

    // The lamps: each a pool of light in two bands, its edge and brighter
    // nearer in; fainter in the sun, which outshines them.
    vec3 lamps = vec3(0.0);
    for (int i = 0; i < 8; i++) {
        if (float(i) >= pointCount) {
            break;
        }
        float a = pointLight(i, fragPosition, n);
        lamps += pointColor[i] * (0.5 * smoothstep(0.02, 0.06, a) + 0.5 * smoothstep(0.3, 0.34, a));
    }
    light += lamps * (1.0 - 0.75 * lit);
    // What glows is lit at least as brightly as it glows.
    light = max(light, emissive);

    // The rim, on what's lit. Flat things (the ground, walls) have none:
    // theirs would be a band across the whole face.
    float edge = pow(1.0 - max(dot(n, normalize(viewPos - fragPosition)), 0.0), rimPower);
    float rounded = step(0.0001, length(fwidth(n)));
    float rim = smoothstep(rimThreshold, rimThreshold + softness, edge) * lit * rounded;

    finalColor = vec4(base.rgb * (light + rimColor * rim), base.a);
}
`
