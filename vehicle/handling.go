package vehicle

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion/physics"
)

// Handling is how a kind of vehicle drives. Engines are geared for about
// half a g at the wheels in first: Torque × Gears[0] × Final over a wheel's
// radius, against Mass. Its suspension is set up so the
// wheels hang where the model has them when the vehicle stands at rest.
type Handling struct {
	Mass      float32 // kg, all up
	WheelMass float32 // kg, each, for its inertia
	Torque    float32 // the engine's most, Nm
	MinRPM    float32
	MaxRPM    float32
	ShiftUp   float32 // rpm
	ShiftDown float32 // rpm
	Gears     []float32
	Reverse   float32
	Final     float32 // the differentials' ratio
	Bump      float32 // m the suspension can rise from rest
	Frequency float32 // the suspension's spring, Hz
	Damping   float32 // 0..1
	Steer     float32 // radians, front wheels at full lock
	RearSteer float32 // radians, steering wheels behind the middle: negative turns them against the front
	SteerFade float32 // m/s by which steering is down to half
	Brake     float32 // Nm a wheel
	HandBrake float32 // Nm a wheel that has one
	// AntiRoll is an anti-roll bar's stiffness on each axle (N/m), 0 for
	// none, which none of them has: Jolt's bars, as stiff as would keep
	// these bodies flat, pump them into rocking side to side on uneven
	// ground until a wheel's off it.
	AntiRoll  float32
	Grip      float32
	PitchRoll float32 // radians it can tip before it's held; 0 no limit
	Lean      *physics.Lean
	Caster    float32 // radians the front fork leans back (a bike's)
	Seat      character.Anim
	Top       float32 // m/s it's limited to; 0 none
}

// Handlings are the kinds of vehicle, by the name a Spec gives.
var Handlings = map[string]Handling{
	// A light four-by-four with long travel: lively, and forgiving.
	"buggy": {
		Mass: 950, WheelMass: 25, Torque: 300, MinRPM: 900, MaxRPM: 6500, ShiftUp: 5200, ShiftDown: 2400,
		Gears: []float32{2.8, 1.9, 1.4, 1.05, 0.82}, Reverse: 3.0, Final: 3.2,
		Bump: 0.24, Frequency: 1.35, Damping: 0.45,
		Steer: 0.6, SteerFade: 26, Brake: 1600, HandBrake: 3500, Grip: 1.15,
		PitchRoll: 70 * rl.Deg2rad, Seat: character.Drive, Top: 38,
	},
	// Two steering wheels in front, one driven behind; narrow, so it's kept
	// from rolling over in turns.
	"trike": {
		Mass: 380, WheelMass: 14, Torque: 120, MinRPM: 900, MaxRPM: 7000, ShiftUp: 6000, ShiftDown: 2600,
		Gears: []float32{2.6, 1.8, 1.35, 1.05, 0.85}, Reverse: 2.6, Final: 2.6,
		Bump: 0.16, Frequency: 1.6, Damping: 0.5,
		Steer: 0.5, SteerFade: 9, Brake: 700, HandBrake: 1500, Grip: 0.95,
		PitchRoll: 40 * rl.Deg2rad, Seat: character.Ride, Top: 30,
	},
	// Two wheels: it leans into turns and holds itself up (Jolt's
	// motorcycle controller).
	"bike": {
		Mass: 230, WheelMass: 10, Torque: 70, MinRPM: 1000, MaxRPM: 8000, ShiftUp: 7000, ShiftDown: 3000,
		Gears: []float32{2.27, 1.63, 1.3, 1.09, 0.96, 0.88}, Reverse: 2.0, Final: 2.6,
		Bump: 0.14, Frequency: 1.8, Damping: 0.6,
		Steer: 0.5, SteerFade: 14, Brake: 500, HandBrake: 700, Grip: 1.2,
		Lean: &physics.Lean{MaxAngle: 45 * rl.Deg2rad}, Caster: 30 * rl.Deg2rad, Seat: character.Ride, Top: 36,
	},
	// A pressurised six-wheeler: heavy and low-geared, its back wheels
	// steering against the front ones to turn tighter.
	"rover": {
		Mass: 3600, WheelMass: 45, Torque: 900, MinRPM: 600, MaxRPM: 3600, ShiftUp: 2900, ShiftDown: 1400,
		Gears: []float32{3.6, 2.3, 1.5, 1.0}, Reverse: 3.8, Final: 2.7,
		Bump: 0.26, Frequency: 1.15, Damping: 0.5,
		Steer: 0.5, RearSteer: -0.25, SteerFade: 14, Brake: 5000, HandBrake: 8000, Grip: 1.0,
		PitchRoll: 50 * rl.Deg2rad, Seat: character.Drive, Top: 22,
	},
	// The haulers: a cab-over truck on six or eight driven wheels.
	"truck": {
		Mass: 9500, WheelMass: 90, Torque: 1800, MinRPM: 600, MaxRPM: 3000, ShiftUp: 2400, ShiftDown: 1200,
		Gears: []float32{4.0, 2.7, 1.8, 1.3, 1.0}, Reverse: 4.2, Final: 2.4,
		Bump: 0.24, Frequency: 1.2, Damping: 0.5,
		Steer: 0.45, SteerFade: 14, Brake: 12000, HandBrake: 18000, Grip: 1.0,
		PitchRoll: 45 * rl.Deg2rad, Seat: character.Drive, Top: 24,
	},
}

const suspensionMin = 0.05 // m of suspension left fully raised

const gravity = 9.81

// sag is how far a spring of frequency f sinks under the weight it carries.
func sag(f float32) float32 {
	w := 2 * math.Pi * float64(f)
	return float32(gravity / (w * w))
}

// loads shares the weight of mass, centred at com, among wheels: as evenly
// as balances it, front to back and side to side.
func loads(wheels []WheelSpec, mass float32, com rl.Vector3) []float32 {
	n := float32(len(wheels))
	var mx, mz float32
	for _, w := range wheels {
		mx += w.At[0] / n
		mz += w.At[2] / n
	}
	var vx, vz float32
	for _, w := range wheels {
		vx += (w.At[0] - mx) * (w.At[0] - mx)
		vz += (w.At[2] - mz) * (w.At[2] - mz)
	}
	weight := mass * gravity
	var a, b float32
	if vz > 1e-4 {
		a = weight * (com.Z - mz) / vz
	}
	if vx > 1e-4 {
		b = weight * (com.X - mx) / vx
	}
	out := make([]float32, len(wheels))
	for i, w := range wheels {
		// Never less than a tenth of an even share: a wheel the weight
		// barely reaches still has a spring.
		out[i] = max(weight/n/10, weight/n+a*(w.At[2]-mz)+b*(w.At[0]-mx))
	}
	return out
}

// build is the physics.Vehicle for spec handled as h, its weight centred at
// com.
func build(spec *Spec, h Handling, com rl.Vector3) physics.Vehicle {
	v := physics.Vehicle{
		Engine: physics.Engine{MaxTorque: h.Torque, MinRPM: h.MinRPM, MaxRPM: h.MaxRPM},
		Transmission: physics.Transmission{Gears: h.Gears, Reverse: h.Reverse,
			ShiftUpRPM: h.ShiftUp, ShiftDownRPM: h.ShiftDown},
		MaxPitchRoll: h.PitchRoll,
		Lean:         h.Lean,
	}
	// The middle, front to back: wheels ahead of it steer as the front, and
	// behind it as the rear.
	var mid float32
	for _, w := range spec.Wheels {
		mid += w.At[2]
	}
	if len(spec.Wheels) > 0 {
		mid /= float32(len(spec.Wheels))
	}
	rest := suspensionMin + h.Bump
	omega := 2 * math.Pi * float64(h.Frequency)
	load := loads(spec.Wheels, h.Mass, com)
	for i, w := range spec.Wheels {
		// Each spring is as stiff as makes its wheel's load sink it by sag:
		// so at rest, every wheel's centre is where the model has it.
		stiffness := load[i] * float32(omega*omega) / gravity
		damping := 2 * h.Damping * float32(math.Sqrt(float64(stiffness*load[i]/gravity)))
		down := rl.Vector3{Y: -1}
		front := w.At[2] > mid
		if h.Caster != 0 && front {
			// The fork leans back: the wheel goes down and forward along it.
			down = rl.Vector3{Y: -float32(math.Cos(float64(h.Caster))), Z: float32(math.Sin(float64(h.Caster)))}
		}
		wheel := physics.Wheel{
			// Attached so that at rest, the spring sunk by its sag, the
			// wheel's centre is where the model has it.
			Position:      rl.Vector3Subtract(vec(w.At), rl.Vector3Scale(down, rest)),
			Radius:        w.Radius,
			Width:         w.Width,
			SuspensionMin: suspensionMin,
			SuspensionMax: rest + sag(h.Frequency),
			Stiffness:     stiffness,
			DampingRate:   damping,
			Brake:         h.Brake,
			Inertia:       0.5 * h.WheelMass * w.Radius * w.Radius,
			Grip:          h.Grip,
			SuspensionDir: down,
			SteeringAxis:  rl.Vector3Negate(down),
			ModelRight:    modelRight(w.Left),
		}
		if w.Steer {
			wheel.MaxSteer = h.Steer
			if !front {
				wheel.MaxSteer = h.RearSteer
			}
		}
		if w.HandBrake {
			wheel.HandBrake = h.HandBrake
		}
		v.Wheels = append(v.Wheels, wheel)
	}
	for _, axle := range axles(spec.Wheels) {
		left, right := axle[0], axle[1]
		if anyDriven(spec.Wheels, left, right) {
			v.Differentials = append(v.Differentials, physics.Differential{Left: left, Right: right, Ratio: h.Final})
		}
		if h.AntiRoll > 0 && left >= 0 && right >= 0 {
			v.AntiRollBars = append(v.AntiRollBars, physics.AntiRollBar{Left: left, Right: right, Stiffness: h.AntiRoll})
		}
	}
	return v
}

// modelRight is the axis of a wheel's model that faces the vehicle's right:
// the models are of left wheels, whose outer face (+X) faces left.
func modelRight(left bool) rl.Vector3 {
	if left {
		return rl.Vector3{X: -1}
	}
	return rl.Vector3{X: 1}
}

// axles pairs the wheels side by side: a left and a right wheel index each,
// -1 where a side has none (a bike's, or a trike's back wheel).
func axles(wheels []WheelSpec) [][2]int {
	var out [][2]int
	used := make([]bool, len(wheels))
	for i, w := range wheels {
		if used[i] {
			continue
		}
		used[i] = true
		pair := [2]int{-1, -1}
		side := func(w WheelSpec) int {
			if w.Left {
				return 0
			}
			return 1
		}
		pair[side(w)] = i
		for j := i + 1; j < len(wheels); j++ {
			o := wheels[j]
			if !used[j] && o.Left != w.Left && math.Abs(float64(o.At[2]-w.At[2])) < 0.25 {
				used[j] = true
				pair[side(o)] = j
				break
			}
		}
		out = append(out, pair)
	}
	return out
}

func anyDriven(wheels []WheelSpec, is ...int) bool {
	for _, i := range is {
		if i >= 0 && wheels[i].Drive {
			return true
		}
	}
	return false
}
