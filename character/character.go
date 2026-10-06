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

	// One-shots: they play once, the character pulling up for them. Only a
	// fall cuts one short; wanting to move skips its recovery.
	Interact
	Punch      // with the left hand
	PunchRight // its mirror image: punches alternate hands
	PickUp

	Slide
	Roll
	LadderClimb
	Vault
	Mantle
	WallKick
	WallKickRight // its mirror image, for a wall on the right
	WallFall      // falling after a wall kick, until landing
	WallFallRight
	WallLand // touching down after a wall kick
	Crouch
	StandUp
	LadderExit
	LadderEnter
	// Fall loops after a jump's clip reaches touchdown in the air: falling
	// from higher than it jumped, legs swinging, until it lands.
	Fall
	// StairsUp and StairsDown walk (or run) up and down stairs, a foot on
	// each step: their clips' cycles are set by how high the feet are, not
	// by time (see stairPhase).
	StairsUp
	StairsDown
	// Drive sits at a wheel, and Ride astride a bike or a trike: see Seated.
	Drive
	Ride
	// SitDown, Sitting and SitUp sit on a bench, a stool or a bunk and get
	// up again (see Seated: whatever seats the character plays them).
	SitDown
	Sitting
	SitUp
	// Held poses, played standing still for as long as Intent.Hold asks:
	// kneeling at a machine to work on it, talking, dancing.
	Fix
	Talk
	Dance
)

// Held reports whether a is a pose held for as long as Intent.Hold asks.
func (a Anim) Held() bool { return a == Fix || a == Talk || a == Dance }

// Airborne reports whether a is a jump, or the fall after one.
func (a Anim) Airborne() bool { return a == Jump || a == RunJump || a == Fall }

// OneShot reports whether a is an action that plays once.
func (a Anim) OneShot() bool { return a >= Interact && a <= PickUp }

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
	case Punch, PunchRight:
		return "punch"
	case PickUp:
		return "pick up"
	case Slide:
		return "slide"
	case Roll:
		return "roll"
	case LadderClimb:
		return "ladder climb"
	case Vault:
		return "vault"
	case Mantle:
		return "mantle"
	case WallKick, WallKickRight:
		return "wall kick"
	case WallLand:
		return "wall kick landing"
	case WallFall, WallFallRight:
		return "wall kick fall"
	case StandUp:
		return "stand up"
	case LadderExit:
		return "ladder exit"
	case LadderEnter:
		return "ladder entry"
	case Crouch:
		return "crouch"
	case Fall:
		return "fall"
	case StairsUp:
		return "stairs up"
	case StairsDown:
		return "stairs down"
	case Drive:
		return "drive"
	case Ride:
		return "ride"
	case SitDown:
		return "sit down"
	case Sitting:
		return "sitting"
	case SitUp:
		return "get up"
	case Fix:
		return "fix"
	case Talk:
		return "talk"
	case Dance:
		return "dance"
	}
	return "unknown"
}

// Character is how a character moves. It goes on the root entity.
type Character struct {
	WalkSpeed, RunSpeed float32 // units per second
	JumpSpeed           float32 // upward speed at take-off
	// Ease is how quickly its speed follows a change, walking to running
	// and back, starting and stopping: each second it closes the gap by
	// the share 1 - e^-Ease, so it eases in as it gets close. Accel caps
	// how hard that pushes, in units per second per second, so a start isn't
	// a jolt. It slows down twice as quickly as it speeds up.
	Ease, Accel float32
	// TurnSpeed is the fastest the body turns to face where it's going, in
	// radians per second, standing (the faster it goes, the slower it
	// turns), and TurnAccel how fast it gets up to that speed and back
	// down.
	TurnSpeed, TurnAccel float32
}

// Default is an ordinary person. The speeds are the Mixamo walk and run's
// own, measured from how fast a planted foot slides under the body. A walk
// turns about in two thirds of a second.
func Default() Character {
	return Character{WalkSpeed: 1.6, RunSpeed: 4.6, JumpSpeed: 4, Ease: 5, Accel: 8, TurnSpeed: 7, TurnAccel: 30}
}

// Intent is what a character wants to do. playerInput writes the player's;
// the character systems carry it out.
type Intent struct {
	// Move is the direction to go on the XZ plane, at most 1 long; zero
	// stands still. In the air it can only veer a character a little.
	Move rl.Vector3
	Run  bool
	// Jump asks for a jump; it's cleared at the next physics step, and
	// ignored while an action plays.
	Jump bool
	// Slide/Roll are edge-triggered requests consumed by traversal.
	Slide, Roll bool
	// Act asks for a one-shot action; Idle means none. It's cleared as soon
	// as it's read, so an ignored request isn't kept for later. Airborne
	// characters, characters already acting, and skins without the clip
	// ignore it. A moving character pulls up for an action, and plays it to
	// its end before another can start; Move waits until then, or until the
	// action is far enough along to skip its recovery and move on.
	Act Anim
	// Hold asks for a held pose (Fix, Talk, Dance) while it stands still;
	// Idle means none. It's kept until it's changed: moving, jumping,
	// sliding, rolling or acting drops it (see playerInput).
	Hold Anim
}

// Body marks the child entity that draws a character.
type Body struct{}

// State is the body's current animation. It goes on the Body.
type State struct {
	Current Anim
	skin    int     // index into Roster.Skins
	air     float32 // seconds off the ground
	turn    float32 // how fast the body is turning, in radians per second
	resume  float32 // seconds left counting as moving on after an action
	// stairs is which way it's going on stairs (1 up, -1 down, 0 not), and
	// stairsLeft how much longer it counts as on them since it last was,
	// so a moment off the slope at the top or bottom step doesn't flicker
	// to a walk and back.
	stairs     int8
	stairsLeft float32
	// rightPunch is whether the next punch is with the right hand.
	rightPunch bool
	// hidden is whether it's hidden, seated out of sight (see Seated).
	hidden bool

	// Off the ground (as the physics step last saw it), and the way it was
	// going and facing when it left: in the air it can only veer a little
	// from those (see airborne).
	aloft     bool
	launch    rl.Vector3
	launchYaw float32
}

// Player marks the character the keyboard controls.
type Player struct{}
