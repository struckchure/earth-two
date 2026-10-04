package shading

// The toon shader: light and shadow in hard bands, shadows tinted rather
// than darkened, and a rim of light on the lit side of rounded things.

const toonFragment = `
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
    vec3 light = mix(ambient * shadowColor, ambient + lightColor, lit);

    // The rim, on what's lit. Flat things (the ground, walls) have none:
    // theirs would be a band across the whole face.
    float edge = pow(1.0 - max(dot(n, normalize(viewPos - fragPosition)), 0.0), rimPower);
    float rounded = step(0.0001, length(fwidth(n)));
    float rim = smoothstep(rimThreshold, rimThreshold + softness, edge) * lit * rounded;

    finalColor = vec4(base.rgb * (light + rimColor * rim), base.a);
}
`
