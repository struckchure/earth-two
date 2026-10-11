package game

import (
	"math"
	"runtime"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// An authored free-issue depot for the local combat prototype. Purchases will
// replace the stock collection operation when authoritative trading exists.
var weaponsDepot = rl.Vector2{X: 9829, Y: -1764}

func depotPosition(x, y, z float32) rl.Vector3 {
	return rl.Vector3{X: weaponsDepot.X + x, Y: groundHeight(weaponsDepot.X, weaponsDepot.Y) + y, Z: weaponsDepot.Y + z}
}

func spawnWeaponsDepot(cmd *illusion.Commands, kit *illusion.Res[world.Kit], meshes *illusion.Res[asset.Assets[render.Mesh]], materials *illusion.Res[asset.Assets[render.StandardMaterial]], textures *illusion.Res[asset.Assets[render.Texture]], scopes *illusion.Res[scopeVisuals]) {
	for _, piece := range depotLayout() {
		if err := kit.Get().Place(cmd, piece); err != nil {
			panic(err)
		}
	}

	steel := materials.Get().Add(render.StandardMaterial{BaseColor: rl.NewColor(51, 55, 48, 255)})
	box := func(x, y, z, w, h, d float32, collision bool) {
		at := depotPosition(x, y, z)
		cmd.Spawn(illusion.C(render.Mesh3d{Mesh: meshes.Get().Add(render.Mesh{Mesh: rl.GenMeshCube(w, h, d)})}), illusion.C(render.MeshMaterial3d{Material: steel}), illusion.C(transform.FromTranslation(at)))
		if collision {
			cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(w, h, d)), illusion.C(transform.FromTranslation(at)))
		}
	}
	for _, x := range []float32{-3.3, 3.3} {
		box(x, 1.6, -1.1, .12, 3.2, .12, true)
	}
	box(0, 3.12, -1.1, 6.7, .16, .18, false)
	box(0, 2.66, -1.1, 4.4, .85, .10, false)
	sign := materials.Get().Add(render.StandardMaterial{BaseColor: rl.White, Texture: textures.Get().Add(depotSignTexture("WEAPONS DEPOT", "FIELD ISSUE / NO CHARGE")), Unlit: true})
	cmd.Spawn(illusion.C(render.Mesh3d{Mesh: meshes.Get().Add(render.Mesh{Mesh: rl.GenMeshPlane(4.35, .8, 1, 1)})}), illusion.C(render.MeshMaterial3d{Material: sign}), illusion.C(transform.FromTranslation(depotPosition(0, 2.66, -1.16)).WithRotation(rl.QuaternionFromEuler(math.Pi/2, math.Pi, 0))))
	for i, label := range []string{"WEAPONS", "AMMO"} {
		x := []float32{-1.65, 1.65}[i]
		mat := materials.Get().Add(render.StandardMaterial{BaseColor: rl.White, Texture: textures.Get().Add(depotSignTexture(label, "")), Unlit: true})
		cmd.Spawn(illusion.C(render.Mesh3d{Mesh: meshes.Get().Add(render.Mesh{Mesh: rl.GenMeshPlane(1.1, .23, 1, 1)})}), illusion.C(render.MeshMaterial3d{Material: mat}), illusion.C(transform.FromTranslation(depotPosition(x, .52, -1.915)).WithRotation(rl.QuaternionFromEuler(math.Pi/2, math.Pi, 0))))
	}
	spawnCombatPickup(cmd, kit.Get(), depotPosition(-1.65, .78, -1.55), combatPickup{DepotStock: true, Weapon: weaponState{Owned: true, Equipped: true, Magazine: 12, Reserve: 24}})
	spawnCombatPickup(cmd, kit.Get(), depotPosition(1.65, .78, -1.55), combatPickup{DepotStock: true, Rounds: 36})
	bench := cmd.Spawn(illusion.C(combatPickup{ScopeBench: true, DepotStock: true}), illusion.C(transform.FromTranslation(depotPosition(0, .8, -1.55))))
	spawnScopeParts(bench, scopes.Get(), sniperRifle, false)
	mat := materials.Get().Add(render.StandardMaterial{BaseColor: rl.White, Texture: textures.Get().Add(depotSignTexture("SCOPES", "FIT / REMOVE")), Unlit: true})
	cmd.Spawn(illusion.C(render.Mesh3d{Mesh: meshes.Get().Add(render.Mesh{Mesh: rl.GenMeshPlane(1.1, .23, 1, 1)})}), illusion.C(render.MeshMaterial3d{Material: mat}), illusion.C(transform.FromTranslation(depotPosition(0, .52, -1.915)).WithRotation(rl.QuaternionFromEuler(math.Pi/2, math.Pi, 0))))

	for i, kind := range []weaponKind{assaultRifle, sniperRifle, shotgun} {
		spec := weaponSpecs[kind]
		spawnCombatPickup(cmd, kit.Get(), depotPosition(-2.5+float32(i)*2.5, .78, 1.6), combatPickup{DepotStock: true, Weapon: weaponState{Kind: kind, Owned: true, Equipped: true, Magazine: spec.Magazine, Reserve: spec.Magazine * 2}})
	}

}

func depotLayout() []world.Placement {
	var pieces []world.Placement
	place := func(name string, x, z float32, turn int) {
		at := depotPosition(x, 0, z)
		pieces = append(pieces, world.Placement{Piece: name, At: [3]float32{at.X, at.Y, at.Z}, Turns: turn})
	}
	for _, x := range []float32{-2, 2} {
		for _, z := range []float32{-2, 2} {
			place("pad_tile", x, z, 0)
		}
	}
	for _, x := range []float32{-1.65, 1.65} {
		place("tarp_awning", x, 0, 2)
		place("weapon_rack", x, 0, 2)
		place("folding_table", x, -1.55, 0)
	}
	for _, x := range []float32{-2.5, 0, 2.5} {
		place("folding_table", x, 1.6, 0)
	}
	place("folding_table", 0, -1.55, 0)
	place("crate", -3.6, 1.3, 0)
	place("crate_tall", 3.6, 1.3, 0)
	place("work_lamp", -3.7, -1.8, 2)
	place("work_lamp", 3.7, -1.8, 2)
	place("hazard_barrier", 6, 3.5, 0)
	place("hazard_barrier", 8, 3.5, 0)

	return pieces
}

// A small block-letter industrial sign, generated identically on desktop and
// web. It needs no extra downloaded texture or platform-specific font API.
func depotSignTexture(title, subtitle string) render.Texture {
	const w, h = 1024, 192
	pixels := make([]byte, w*h*4)
	ink := rl.NewColor(234, 216, 174, 255)
	paint := func(x, y, width, height int, c rl.Color) {
		for row := max(0, y); row < min(h, y+height); row++ {
			for col := max(0, x); col < min(w, x+width); col++ {
				copy(pixels[(row*w+col)*4:], []byte{c.R, c.G, c.B, c.A})
			}
		}
	}
	paint(0, 0, w, h, rl.NewColor(31, 39, 35, 255))
	paint(0, 0, w, 6, ink)
	paint(0, h-6, w, 6, ink)
	glyphs := map[rune][7]string{
		'A': {"01110", "10001", "10001", "11111", "10001", "10001", "10001"},
		'C': {"01111", "10000", "10000", "10000", "10000", "10000", "01111"},
		'D': {"11110", "10001", "10001", "10001", "10001", "10001", "11110"},
		'E': {"11111", "10000", "10000", "11110", "10000", "10000", "11111"},
		'F': {"11111", "10000", "10000", "11110", "10000", "10000", "10000"},
		'G': {"01111", "10000", "10000", "10111", "10001", "10001", "01111"},
		'H': {"10001", "10001", "10001", "11111", "10001", "10001", "10001"},
		'I': {"11111", "00100", "00100", "00100", "00100", "00100", "11111"},
		'L': {"10000", "10000", "10000", "10000", "10000", "10000", "11111"},
		'M': {"10001", "11011", "10101", "10101", "10001", "10001", "10001"},
		'N': {"10001", "11001", "11001", "10101", "10011", "10011", "10001"},
		'O': {"01110", "10001", "10001", "10001", "10001", "10001", "01110"},
		'P': {"11110", "10001", "10001", "11110", "10000", "10000", "10000"},
		'R': {"11110", "10001", "10001", "11110", "10100", "10010", "10001"},
		'S': {"01111", "10000", "10000", "01110", "00001", "00001", "11110"},
		'T': {"11111", "00100", "00100", "00100", "00100", "00100", "00100"},
		'U': {"10001", "10001", "10001", "10001", "10001", "10001", "01110"},
		'V': {"10001", "10001", "10001", "10001", "10001", "01010", "00100"},
		'W': {"10001", "10001", "10001", "10101", "10101", "11011", "10001"},
		'/': {"00001", "00001", "00010", "00100", "01000", "10000", "10000"},
	}
	text := func(label string, y, scale int) {
		x := (w - (len(label)*6-1)*scale) / 2
		for _, letter := range label {
			for row, line := range glyphs[letter] {
				for col, bit := range line {
					if bit == '1' {
						paint(x+col*scale, y+row*scale, scale, scale, ink)
					}
				}
			}
			x += 6 * scale
		}
	}
	text(title, 24, 11)
	text(subtitle, 128, 5)
	img := rl.NewImage(pixels, w, h, 1, rl.UncompressedR8g8b8a8)
	texture := rl.LoadTextureFromImage(img)
	runtime.KeepAlive(pixels)
	rl.SetTextureFilter(texture, rl.FilterBilinear)
	return render.Texture{Texture2D: texture}
}
