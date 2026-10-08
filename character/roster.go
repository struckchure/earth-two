package character

import (
	"math/rand/v2"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// Body proportions. Models are scaled to stand bodyHeight tall, with their
// feet at the bottom of the capsule.
const (
	bodyHeight    = 1.75
	capsuleHeight = 1.8
	capsuleRadius = 0.3
)

// Clip is the animation an Anim plays.
type Clip struct {
	Name string
	// Start is how many seconds into the clip to begin. Jumps start at
	// take-off, since the physics jump happens at once.
	Start float32
	// Land, for jumps, is when the feet touch down. The clip plays slower or
	// faster so take-off to Land lasts as long as the jump, and holds at Land
	// while the character is still falling.
	Land float32
	// Hold, for jumps, keeps the clip at Start for as long as the character
	// is airborne: a pose, for a model without a jump clip.
	Hold bool
	// Loop, for jumps, loops the clip for as long as the character is
	// airborne: an in-air cycle.
	Loop bool
	// Step, for stairs, is the share of the clip's cycle at which a foot
	// is planted on a step at the height of the floor the stairs stand on.
	Step float32
	// Backward plays the clip backwards: coming down stairs with the clip
	// for climbing them, for a body with no clip of its own for that.
	Backward bool
}

// Model describes a character model file: a skinned glTF holding the mesh
// and its clips, facing +Z with its feet at the origin. Run files from
// FBX exporters through tools/bindpose first.
type Model struct {
	Path  string // relative to the asset root
	Clips map[Anim]Clip
	// Scale sizes the model; 0 fits it to bodyHeight. Models made in metres
	// can keep their own height with 1.
	Scale float32
}

// Skin is a loaded Model.
type Skin struct {
	Model asset.Handle[render.Model]
	Anims asset.Handle[render.Animations]
	Clips map[Anim]Clip
	// Scale brings the model to bodyHeight.
	Scale float32
}

// Has reports whether the skin has a clip for a.
func (s *Skin) Has(a Anim) bool { return s.Clips[a].Name != "" }

// clip is the clip for a, falling back from RunJump to Jump and from
// anything else missing to Idle.
func (s *Skin) clip(a Anim) Clip {
	if c, ok := s.Clips[a]; ok && c.Name != "" {
		return c
	}
	if a == RunJump {
		return s.clip(Jump)
	}
	switch a {
	case WallKickRight:
		return s.clip(WallKick)
	case PunchRight:
		return s.clip(Punch)
	case Fall:
		return s.clip(Jump)
	case WallFallRight:
		return s.clip(WallFall)
	case WallFall:
		return s.clip(Jump)
	}
	return s.Clips[Idle]
}

// Roster is a resource with the characters that can be spawned.
type Roster struct {
	Skins []Skin
}

// Spawn adds a character wearing skin (wrapped to the roster's size),
// standing with its feet at feet. extra components go on the root, next to
// Default's Character; insert your own Character afterwards to change it.
func (r *Roster) Spawn(cmd *illusion.Commands, skin int, feet rl.Vector3, extra ...illusion.Component) illusion.EntityCommands {
	n := len(r.Skins)
	skin = (skin%n + n) % n
	s := &r.Skins[skin]
	player := render.AnimationPlayer{Animations: s.Anims}
	player.Play(s.clip(Idle).Name)
	player.Seek(rand.Float32() * 2) // so crowds don't breathe in step

	center := rl.Vector3Add(feet, rl.Vector3{Y: capsuleHeight / 2})
	root := append([]illusion.Component{
		illusion.C(Default()),
		illusion.C(Health{}),
		illusion.C(MotionSamples{previous: center, current: center, height: capsuleHeight}),
		illusion.C(Intent{}),
		illusion.C(Traversal{}),
		illusion.C(DefaultTraversal()),
		illusion.C(physics.CharacterController{Radius: capsuleRadius, Height: capsuleHeight, StepHeight: 0.3}),
		illusion.C(transform.FromTranslation(center)),
	}, extra...)
	return cmd.Spawn(root...).WithChild(
		illusion.C(Body{}),
		illusion.C(State{skin: skin}),
		illusion.C(render.Model3d{Model: s.Model}),
		illusion.C(player),
		illusion.C(transform.FromXYZ(0, -capsuleHeight/2, 0).WithScale(s.Scale)),
	)
}
