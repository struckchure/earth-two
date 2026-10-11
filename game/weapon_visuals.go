package game

import (
	rl "github.com/gen2brain/raylib-go/raylib"

	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"math"
)

type scopePart struct {
	Mesh      asset.Handle[render.Mesh]
	Material  asset.Handle[render.StandardMaterial]
	Transform transform.Transform
}
type scopeVisuals struct{ Parts []scopePart }
type pickupScope struct{}
type heldScope struct {
	Visible bool
	Kind    weaponKind
	Part    int
}

func setupScopeVisuals(v *illusion.Res[scopeVisuals], meshes *illusion.Res[asset.Assets[render.Mesh]], materials *illusion.Res[asset.Assets[render.StandardMaterial]]) {
	metal := materials.Get().Add(render.StandardMaterial{BaseColor: rl.NewColor(48, 53, 54, 255)})
	rubber := materials.Get().Add(render.StandardMaterial{BaseColor: rl.NewColor(20, 23, 24, 255)})
	glass := materials.Get().Add(render.StandardMaterial{BaseColor: rl.NewColor(32, 72, 80, 255)})
	add := func(mesh rl.Mesh, at rl.Vector3, rotation rl.Quaternion, material asset.Handle[render.StandardMaterial]) {
		v.Get().Parts = append(v.Get().Parts, scopePart{meshes.Get().Add(render.Mesh{Mesh: mesh}), material, transform.FromTranslation(at).WithRotation(rotation)})
	}
	axial := rl.QuaternionFromEuler(0, 0, -math.Pi/2)
	tube := func(x, length, radius float32, material asset.Handle[render.StandardMaterial]) {
		add(rl.GenMeshCylinder(radius, length, 20), rl.Vector3{X: x, Z: .048}, axial, material)
	}
	// A compact 3x optic: ocular housing, narrow tube and stepped objective bell.
	tube(-.125, .042, .026, rubber)
	tube(-.083, .155, .018, metal)
	tube(.072, .016, .023, metal)
	tube(.088, .016, .028, metal)
	tube(.104, .038, .032, metal)
	tube(.142, .009, .034, rubber)
	// Recessed, muted glass avoids the old glowing blue end cap.
	tube(.151, .001, .027, glass)
	tube(-.126, .001, .020, glass)
	for _, x := range []float32{-.064, .047} {
		tube(x, .014, .023, rubber)
		add(rl.GenMeshCube(.022, .036, .030), rl.Vector3{X: x + .007, Z: .018}, rl.QuaternionIdentity(), metal)
	}
	add(rl.GenMeshCube(.16, .034, .009), rl.Vector3{X: .001, Z: .0045}, rl.QuaternionIdentity(), rubber)
	// Elevation and windage turrets sit at the centre of the tube.
	add(rl.GenMeshCylinder(.014, .022, 16), rl.Vector3{X: .008, Z: .061}, rl.QuaternionFromEuler(math.Pi/2, 0, 0), metal)
	add(rl.GenMeshCylinder(.017, .008, 16), rl.Vector3{X: .008, Z: .083}, rl.QuaternionFromEuler(math.Pi/2, 0, 0), rubber)
	add(rl.GenMeshCylinder(.012, .025, 16), rl.Vector3{X: .008, Y: .012, Z: .048}, rl.QuaternionIdentity(), metal)
	add(rl.GenMeshCylinder(.015, .007, 16), rl.Vector3{X: .008, Y: .037, Z: .048}, rl.QuaternionIdentity(), rubber)
}

// The source models use X along the barrel, Z up and Y across the receiver.
// Match each receiver height and centreline instead of floating one mount over all guns.
func scopeMount(kind weaponKind) rl.Vector3 {
	switch kind {
	case assaultRifle:
		return rl.Vector3{X: -.12, Y: .037, Z: .085}
	case shotgun:
		return rl.Vector3{X: -.12, Y: .0325, Z: .059}
	default:
		return rl.Vector3{X: -.12, Y: .02865, Z: .065}
	}
}

// Each mesh has its own grip origin; keeping the old pistol offset on long
// guns placed the firing hand in front of the trigger instead of on the grip.
func weaponGrip(kind weaponKind) transform.Transform {
	at := transform.FromXYZ(.003480, .103933, -.009267)
	at.Rotation = rl.Quaternion{X: -.149315, Y: -.387756, Z: -.659298, W: -.626638}
	shift := [...]rl.Vector3{{}, {X: .265, Z: .02}, {X: .23, Z: .015}, {X: .215, Z: .015}}[kind]
	at.Translation = rl.Vector3Add(at.Translation, rl.Vector3RotateByQuaternion(shift, at.Rotation))
	return at
}

func spawnScopeParts(parent illusion.EntityCommands, v *scopeVisuals, kind weaponKind, held bool) {
	for i, part := range v.Parts {
		tr := part.Transform
		tr.Translation = rl.Vector3Add(tr.Translation, scopeMount(kind))
		components := []illusion.Component{illusion.C(tr), illusion.C(render.MeshMaterial3d{Material: part.Material})}
		if held {
			components = append(components, illusion.C(heldScope{Part: i, Kind: kind}))
		} else {
			components = append(components, illusion.C(render.Mesh3d{Mesh: part.Mesh}))
		}
		parent.WithChild(components...)
	}
}
