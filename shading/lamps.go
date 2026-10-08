package shading

import (
	"fmt"
	"math"
	"sort"
	"strings"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion/render"
)

// Lamp is a fixed light from the world's authored layout. Unlike vehicle
// headlights, it never moves or switches off during play.
type Lamp struct {
	At    rl.Vector3
	Light render.PointLight
}

// Group distant settlements so a fragment skips their lights together.
// These bounds enclose each lamp's entire reach, including its height.
type lampGroup struct {
	min, max     rl.Vector3
	start, count int
}

func groupLamps(lamps []Lamp) ([]Lamp, []lampGroup) {
	type cell struct{ x, z int }
	cellOf := func(l Lamp) cell {
		return cell{int(math.Round(float64(l.At.X) / 1024)), int(math.Round(float64(l.At.Z) / 1024))}
	}
	byCell := map[cell][]Lamp{}
	var cells []cell
	for _, l := range lamps {
		if l.Light.Range <= 0 {
			continue
		}
		c := cellOf(l)
		if _, ok := byCell[c]; !ok {
			cells = append(cells, c)
		}
		byCell[c] = append(byCell[c], l)
	}
	sort.Slice(cells, func(i, j int) bool {
		if cells[i].x != cells[j].x {
			return cells[i].x < cells[j].x
		}
		return cells[i].z < cells[j].z
	})
	var ordered []Lamp
	var groups []lampGroup
	for _, c := range cells {
		lights := byCell[c]
		g := lampGroup{start: len(ordered), count: len(lights)}
		for i, l := range lights {
			r := l.Light.Range
			lo := rl.Vector3Subtract(l.At, rl.Vector3{X: r, Y: r, Z: r})
			hi := rl.Vector3Add(l.At, rl.Vector3{X: r, Y: r, Z: r})
			if i == 0 {
				g.min, g.max = lo, hi
			} else {
				g.min, g.max = rl.Vector3Min(g.min, lo), rl.Vector3Max(g.max, hi)
			}
		}
		ordered = append(ordered, lights...)
		groups = append(groups, g)
	}
	return ordered, groups
}

// Fixed lamps are shader constants, prepared before the renderer compiles
// its program. All lights remain available without extra per-draw uniform
// uploads or a camera-selected eight-light budget. The same source runs on
// desktop GL and WebGL 2; no textures or extra uniform slots are needed.
func lampGLSL(lamps []Lamp) string {
	lights, groups := groupLamps(lamps)
	if len(lights) == 0 {
		return "\nvec3 worldLamps(vec3 pos, vec3 n) { return vec3(0.0); }\n"
	}
	var b strings.Builder
	fmt.Fprintf(&b, "\nconst vec4 lampPos[%d] = vec4[%d](\n", len(lights), len(lights))
	for i, l := range lights {
		if i > 0 {
			b.WriteString(",\n")
		}
		fmt.Fprintf(&b, "vec4(%.6f, %.6f, %.6f, %.6f)", l.At.X, l.At.Y, l.At.Z, l.Light.Range)
	}
	fmt.Fprintf(&b, "\n);\nconst vec3 lampColor[%d] = vec3[%d](\n", len(lights), len(lights))
	for i, l := range lights {
		if i > 0 {
			b.WriteString(",\n")
		}
		k := l.Light.Intensity
		if k == 0 {
			k = 1
		} // render.PointLight's default
		c := scaled(l.Light.Color, k)
		fmt.Fprintf(&b, "vec3(%.6f, %.6f, %.6f)", c[0], c[1], c[2])
	}
	fmt.Fprintf(&b, "\n);\nconst vec4 lampGroupMin[%d] = vec4[%d](\n", len(groups), len(groups))
	for i, g := range groups {
		if i > 0 {
			b.WriteString(",\n")
		}
		fmt.Fprintf(&b, "vec4(%.6f, %.6f, %.6f, %.1f)", g.min.X, g.min.Y, g.min.Z, float32(g.start))
	}
	fmt.Fprintf(&b, "\n);\nconst vec4 lampGroupMax[%d] = vec4[%d](\n", len(groups), len(groups))
	for i, g := range groups {
		if i > 0 {
			b.WriteString(",\n")
		}
		fmt.Fprintf(&b, "vec4(%.6f, %.6f, %.6f, %.1f)", g.max.X, g.max.Y, g.max.Z, float32(g.count))
	}
	fmt.Fprintf(&b, `
);
vec3 worldLamps(vec3 pos, vec3 n) {
    vec3 lamps = vec3(0.0);
    for (int group = 0; group < %d; group++) {
        vec4 lo = lampGroupMin[group];
        vec4 hi = lampGroupMax[group];
        if (any(lessThan(pos, lo.xyz)) || any(greaterThan(pos, hi.xyz))) {
            continue;
        }
        for (int offset = 0; offset < %d; offset++) {
            if (offset >= int(hi.w)) { break; }
            int i = int(lo.w) + offset;
            vec3 to = lampPos[i].xyz - pos;
            float d = length(to);
            if (d >= lampPos[i].w) { continue; }
            float fall = 1.0 - d / lampPos[i].w;
            float a = fall * fall * max(dot(n, to / max(d, 0.0001)), 0.0);
            lamps += lampColor[i] * (0.45 * smoothstep(0.0, 0.12, a)
                + 0.55 * smoothstep(0.15, 0.5, a));
        }
    }
    return lamps;
}
`, len(groups), len(lights))
	return b.String()
}
