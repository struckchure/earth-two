//go:build !js

package game

import rl "github.com/gen2brain/raylib-go/raylib"

// The GLSL version the particles' shader is written for, on the desktop.
const (
	effectsVertexHeader   = "#version 330\n"
	effectsFragmentHeader = "#version 330\n"
)

// The GPU needs only the live prefix of the indexed particle mesh. Keep
// the allocation at its full budget, but skip the collapsed unused quads.
func particleDrawMesh(mesh rl.Mesh, count int) rl.Mesh {
	mesh.VertexCount = int32(4 * count)
	mesh.TriangleCount = int32(2 * count)
	return mesh
}
