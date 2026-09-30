// Package character spawns and animates the people of Earth Two. Every
// character shares one body, one locomotion system and one animation state
// machine, and differs only in what writes its [Intent]: the keyboard for
// the player, and later, job and contract AI for everyone else.
//
// A character is a root entity with [Character], [Intent], a
// physics.CharacterController and a Transform at the capsule's center, and a
// child [Body] with the skinned model, its render.AnimationPlayer and a
// [State]. Spawn one with [Roster.Spawn].
package character

import rl "github.com/gen2brain/raylib-go/raylib"

// Anim is what a character's body is doing: a locomotion loop, a jump, or a
// one-shot action.
type Anim uint8

const (
	Idle Anim = iota
	Walk
	Run
	Jump    // airborne after standing still (or falling off something)
	RunJump // airborne after moving

	// One-shots: they play once, and moving cuts them short.
	Interact
	Punch
	PickUp
)

// Airborne reports whether a is a jump.
func (a Anim) Airborne() bool { return a == Jump || a == RunJump }

// OneShot reports whether a is an action that plays once.
func (a Anim) OneShot() bool { return a >= Interact }

func (a Anim) String() string {
	switch a {
	case Idle:
		return "idle"
	case Walk:
		return "walk"
	case Run:
		return "run"
	case Jump:
		return "jump"
	case RunJump:
		return "running jump"
	case Interact:
		return "interact"
	case Punch:
		return "punch"
	case PickUp:
		return "pick up"
	}
	return "unknown"
}

// Character is how a character moves. It goes on the root entity.
type Character struct {
	WalkSpeed, RunSpeed float32 // units per second
	JumpSpeed           float32 // upward speed at take-off
	TurnSpeed           float32 // how fast the body turns to face where it's going
}

// Default is an ordinary person. The speeds are the Mixamo walk and run's
// own, measured from how fast a planted foot slides under the body.
func Default() Character {
	return Character{WalkSpeed: 1.6, RunSpeed: 4.6, JumpSpeed: 4, TurnSpeed: 12}
}

// Intent is what a character wants to do. playerInput writes the player's;
// the character systems carry it out.
type Intent struct {
	// Move is the direction to go on the XZ plane, at most 1 long; zero
	// stands still.
	Move rl.Vector3
	Run  bool
	// Jump asks for a jump; it's cleared at the next physics step.
	Jump bool
	// Act asks for a one-shot action; it's cleared once started. Idle means
	// none. Moving characters, and skins without the clip, ignore it.
	Act Anim
}

// Body marks the child entity that draws a character.
type Body struct{}

// State is the body's current animation. It goes on the Body.
type State struct {
	Current Anim
	skin    int     // index into Roster.Skins
	air     float32 // seconds off the ground
}

// Player marks the character the keyboard controls.
type Player struct{}
