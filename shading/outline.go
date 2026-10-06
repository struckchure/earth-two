package shading

// The outline shader, for a render.Pass with CullFront: the model's back
// faces pushed out along their normals, an inverted hull, in a dark tint of
// the surface's own colour.

const outlineVertex = outlineVertexStart + "vertexNormal" + outlineVertexEnd

// A box's normals are its faces', which would pull its hull apart at the
// edges, so its outline goes out from its centre through each corner.
const boxOutlineVertex = outlineVertexStart + "sign(vertexPosition)" + outlineVertexEnd

const outlineVertexStart = `
in vec3 vertexPosition;
in vec2 vertexTexCoord;
in vec3 vertexNormal;

uniform mat4 mvp;
uniform vec2 pixel; // the outline's width, in clip space
uniform float reach; // how far away it starts thinning with distance

out vec2 fragTexCoord;
out float fragDepth;

void main() {
    fragTexCoord = vertexTexCoord;
    vec4 clip = mvp * vec4(vertexPosition, 1.0);
    fragDepth = clip.w;
    // Outwards on screen, so the line is as wide wherever it is.
    vec2 along = (mvp * vec4(`

const outlineVertexEnd = `, 0.0)).xy / pixel;
    if (length(along) > 0.00001) {
        clip.xy += normalize(along) * pixel * min(clip.w, reach);
    }
    gl_Position = clip;
}
`

const outlineFragment = `
in vec2 fragTexCoord;
in float fragDepth;

uniform sampler2D texture0;
uniform vec4 colDiffuse;
uniform vec3 outlineColor;
uniform vec3 fogColor;
uniform float fogDistance;
uniform float fogEnd;

out vec4 finalColor;

void main() {
    vec4 base = texture(texture0, fragTexCoord) * colDiffuse;
    if (base.a < 0.5) {
        discard;
    }
    finalColor = vec4(base.rgb * outlineColor, 1.0);
    // Hazed like what it's round (see the toon shader).
    if (fogDistance > 0.0) {
        finalColor.rgb = mix(finalColor.rgb, fogColor, (1.0 - exp(-fragDepth / fogDistance)) * step(fragDepth, fogEnd));
    }
}
`
