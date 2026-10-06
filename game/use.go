package game

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// What the player can use in the world besides vehicles: sit on benches,
// stools, chairs and bunks, and kneel to work on machinery (the Repair Run
// contract's work, docs/contracts.md). Walk up to one and the HUD offers it
// (E); a vehicle in reach is offered first. Moving, or E again, gets up.

// useKind is what a spot's for.
type useKind uint8

const (
	useSit useKind = iota
	useRepair
)

// useSpot is a place on a piece to use it, in the piece's own frame (the
// game's: Y up, the piece's front toward +Z): At is where the user's feet
// go (sitting, the seat's front edge on the floor; repairing, where they
// kneel), Facing the way they face (radians from +Z toward +X).
type useSpot struct {
	kind   useKind
	at     rl.Vector3
	facing float32
}

// seatHeight is how high the sitting clips sit (Sitting_Enter and its
// loop): a seat higher or lower raises or lowers the sitter to it.
const seatHeight = 0.45

// sitAt is a seat at x across a piece and z out from it, its top height
// high, facing the piece's front.
func sitAt(x, z, height float32) useSpot {
	return useSpot{useSit, rl.Vector3{X: x, Y: height - seatHeight, Z: z}, 0}
}

// repairAt is where to kneel to work on a machine whose front is front
// metres out from its origin, x across it.
func repairAt(x, front float32) useSpot {
	return useSpot{useRepair, rl.Vector3{X: x, Z: front + 0.45}, math.Pi}
}

// useSpots are the pieces that can be used, by name (see tools/world for
// their shapes and sizes).
var useSpots = map[string][]useSpot{
	"stone_bench":   {sitAt(-0.55, 0.05, 0.45), sitAt(0, 0.05, 0.45), sitAt(0.55, 0.05, 0.45)},
	"waiting_bench": {sitAt(-0.55, 0.08, 0.48), sitAt(0, 0.08, 0.48), sitAt(0.55, 0.08, 0.48)},
	"stool":         {sitAt(0, 0.05, 0.46)},
	"office_chair":  {sitAt(0, 0.08, 0.5)},
	"cot":           {sitAt(-0.45, 0.2, 0.45), sitAt(0.45, 0.2, 0.45)},
	"bunk_bed":      {sitAt(-0.5, 0.3, 0.45), sitAt(0.4, 0.3, 0.45)},

	"air_scrubber":    {repairAt(0, 0.57)},
	"air_fan":         {repairAt(0, 0.6)},
	"generator":       {repairAt(0, 0.56)},
	"pump_unit":       {repairAt(0, 0.59)},
	"valve_station":   {repairAt(0, 0.33)},
	"junction_box":    {repairAt(0, 0.29)},
	"electronics_box": {repairAt(0, 0.27)},
	"control_console": {repairAt(0, 0.45)},
	"power_conduit":   {repairAt(0, 0.28)},
	"wind_turbine":    {repairAt(0, 0.4)},
}

// useReach is how close (m) the player's feet must be to a spot to use it.
const useReach = 1.4

// The sitting clips' lengths: sitting down, then the loop, then getting up.
const (
	sitDownTime = 1.3
	getUpTime   = 1.03
)

// placedSpot is a use spot where its piece was placed: At in the world,
// Facing turned with it.
type placedSpot struct {
	piece  string
	kind   useKind
	at     rl.Vector3
	facing float32
}

// uses is a resource: every use spot in the world, on a grid of cells
// useCell metres square to find those near the player.
type uses struct {
	cells map[[2]int][]placedSpot
	// sitting is the seat the player's in, if any, and how long since they
	// sat (getting up counts down from getUpTime).
	sitting   *placedSpot
	since     float32
	gettingUp float32
	// face is the way to turn the player, who's just knelt to a machine.
	face *float32
}

const useCell = 8

func cellOf(x, z float32) [2]int {
	return [2]int{int(math.Floor(float64(x / useCell))), int(math.Floor(float64(z / useCell)))}
}

// newUses finds the use spots on placed pieces, stood on the ground as
// spawnOnTerrain stands them.
func newUses(k *world.Kit, placed []world.Placement) *uses {
	u := &uses{cells: map[[2]int][]placedSpot{}}
	for _, p := range placed {
		spots, ok := useSpots[p.Piece]
		if !ok {
			continue
		}
		p.At[1] += standOn(k, p)
		turn := rl.QuaternionFromAxisAngle(transform.Up, float32(p.Turns)*math.Pi/2)
		origin := rl.Vector3{X: p.At[0], Y: p.At[1], Z: p.At[2]}
		for _, s := range spots {
			at := rl.Vector3Add(origin, rl.Vector3RotateByQuaternion(s.at, turn))
			ps := placedSpot{p.Piece, s.kind, at, s.facing + float32(p.Turns)*math.Pi/2}
			c := cellOf(at.X, at.Z)
			u.cells[c] = append(u.cells[c], ps)
		}
	}
	return u
}

// near returns the spot nearest feet within useReach, if any.
func (u *uses) near(feet rl.Vector3) (placedSpot, bool) {
	var best placedSpot
	bestDist := float32(useReach)
	found := false
	c := cellOf(feet.X, feet.Z)
	for dx := -1; dx <= 1; dx++ {
		for dz := -1; dz <= 1; dz++ {
			for _, s := range u.cells[[2]int{c[0] + dx, c[1] + dz}] {
				// The floor under a seat, or where a repairer kneels.
				floor := s.at
				if s.kind == useSit {
					floor = rl.Vector3Add(s.at, rl.Vector3RotateByQuaternion(rl.Vector3{Z: 0.45}, yawTurn(s.facing)))
				}
				if dy := floor.Y - feet.Y; dy < -0.6 || dy > 0.9 {
					continue
				}
				if d := float32(math.Hypot(float64(floor.X-feet.X), float64(floor.Z-feet.Z))); d < bestDist {
					best, bestDist, found = s, d, true
				}
			}
		}
	}
	return best, found
}

func yawTurn(yaw float32) rl.Quaternion { return rl.QuaternionFromAxisAngle(transform.Up, yaw) }

// standUpAt is where someone getting up from seat s stands: in front of it.
func standUpAt(s placedSpot) rl.Vector3 {
	return rl.Vector3Add(s.at, rl.Vector3RotateByQuaternion(rl.Vector3{Z: 0.55}, yawTurn(s.facing)))
}

// vehicleNear is how close (m) a parked vehicle has to be for E to be
// left to it (vehicle.Reach is to its seat, which can be well off its
// middle).
const vehicleNear = vehicle.Reach + 2

// byVehicle reports whether a vehicle nobody's driving is near feet.
func byVehicle(cars *illusion.Query2[vehicle.Drivable, transform.Transform], feet rl.Vector3) bool {
	near := false
	cars.Each(func(_ ecs.Entity, d *vehicle.Drivable, tr *transform.Transform) {
		if d.Driver.IsZero() && rl.Vector3Distance(tr.Translation, feet) < vehicleNear {
			near = true
		}
	})
	return near
}

// useThings seats the player, kneels them to a machine or gets them up,
// when they press E (or move, to get up). It runs after the keys are read
// and before characters act, so E isn't also the generic Interact;
// offerUse puts what it would do on the HUD.
func useThings(
	players *illusion.Query4Where[character.Intent, transform.Transform, physics.CharacterController, character.Traversal, illusion.With[character.Player]],
	seated *illusion.Query2Where[character.Seated, character.Intent, illusion.With[character.Player]],
	cars *illusion.Query2[vehicle.Drivable, transform.Transform],
	used *illusion.Res[uses],
	controls *illusion.Res[character.Controls],
	driving *illusion.Res[vehicle.Driving],
	t *illusion.Res[illusion.Time],
	cmd *illusion.Commands,
) {
	u := used.Get()
	if !controls.Get().Enabled || driving.Get().Active() {
		return
	}

	// In a seat of ours: sitting down, sat, getting up.
	if u.sitting != nil {
		var player ecs.Entity
		var s *character.Seated
		var in *character.Intent
		seated.Each(func(p ecs.Entity, st *character.Seated, i *character.Intent) { player, s, in = p, st, i })
		if s == nil || !s.Seat.IsZero() {
			u.sitting = nil // seated in a vehicle now, or not at all
			return
		}
		u.since += t.Get().DeltaSecs()
		switch {
		case u.gettingUp > 0:
			u.gettingUp -= t.Get().DeltaSecs()
			s.Anim = character.SitUp
			if u.gettingUp <= 0 {
				character.Stand(cmd, player, standUpAt(*u.sitting), u.sitting.facing)
				u.sitting = nil
			}
		case u.since < sitDownTime:
			s.Anim = character.SitDown
		default:
			s.Anim = character.Sitting
			if in.Act == character.Interact || in.Move != (rl.Vector3{}) {
				u.gettingUp = getUpTime
			}
		}
		in.Act = character.Idle
		return
	}

	player, in, tr, cc, tv, ok := players.Single()
	if !ok || in.Act != character.Interact || !cc.Grounded || tv.Mode != character.Idle {
		return
	}
	feet := rl.Vector3Subtract(tr.Translation, rl.Vector3{Y: cc.Height / 2})
	if byVehicle(cars, feet) {
		return
	}
	s, ok := u.near(feet)
	if !ok {
		return
	}
	in.Act = character.Idle // using it, not interacting
	switch s.kind {
	case useSit:
		spot := s
		u.sitting, u.since, u.gettingUp = &spot, 0, 0
		character.Sit(cmd, player, character.Seated{
			Pose: transform.FromTranslation(s.at).WithRotation(yawTurn(s.facing)),
			Anim: character.SitDown,
		})
	case useRepair:
		// Kneeling to the machine until they move; offerUse turns them to it.
		in.Hold = character.Fix
		face := s.facing
		u.face = &face
	}
}

// offerUse puts on the HUD what E would use, after the vehicles have made
// their offer (which comes first), and turns someone who's just knelt to a
// machine to face it.
func offerUse(
	players *illusion.Query3Where[character.Intent, transform.Transform, physics.CharacterController, illusion.With[character.Player]],
	bodies *illusion.Query1Where[transform.Transform, illusion.With[character.Body]],
	hier *illusion.Hierarchy,
	cars *illusion.Query2[vehicle.Drivable, transform.Transform],
	used *illusion.Res[uses],
	controls *illusion.Res[character.Controls],
	prompt *illusion.Res[vehicle.Prompt],
	driving *illusion.Res[vehicle.Driving],
) {
	u, pr := used.Get(), prompt.Get()
	player, in, tr, cc, ok := players.Single()
	if ok && u.face != nil {
		hier.EachChild(player, func(c ecs.Entity) {
			if body, ok := bodies.Get(c); ok {
				body.Rotation = yawTurn(*u.face)
			}
		})
		u.face = nil
	}
	if pr.Key != "" || !controls.Get().Enabled || driving.Get().Active() {
		return
	}
	if u.sitting != nil {
		if u.gettingUp <= 0 && u.since >= sitDownTime {
			pr.Key, pr.Text = "E", "Get up"
		}
		return
	}
	if !ok || !cc.Grounded {
		return
	}
	if in.Hold == character.Fix {
		pr.Key, pr.Text = "Move", "Stop working"
		return
	}
	feet := rl.Vector3Subtract(tr.Translation, rl.Vector3{Y: cc.Height / 2})
	if byVehicle(cars, feet) {
		return
	}
	if s, ok := u.near(feet); ok {
		pr.Key, pr.Text = "E", map[useKind]string{useSit: "Sit down", useRepair: "Work on it"}[s.kind]
	}
}
