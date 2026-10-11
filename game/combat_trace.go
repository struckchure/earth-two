package game

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion/transform"
)

// Presentation only: damage still uses the obstruction-tested hitscan. Keep
// each round alive independently so automatic fire doesn't replace old trails.
const maxBulletTraces = 64

type bulletTrace struct {
	From, To rl.Vector3
	Age      float32
	Kind     weaponKind
}

type muzzleFlash struct {
	From, Direction rl.Vector3
	Age             float32
	Kind            weaponKind
}

func tracerStyle(kind weaponKind) (speed, length, opacity float32) {
	switch kind {
	case assaultRifle:
		return 820, 1.8, 180
	case sniperRifle:
		return 900, 2.4, 190
	case shotgun:
		return 380, .28, 65
	default:
		return 320, .7, 145
	}
}

func (t bulletTrace) lifetime() float32 {
	speed, _, _ := tracerStyle(t.Kind)
	return rl.Vector3Distance(t.From, t.To)/speed + .025
}

// A short shutter exposure makes the first frame visible even at high FPS.
// Both endpoints are clamped to the hit: no trail continues through a wall.
func (t bulletTrace) segment() (tail, head rl.Vector3, alpha float32) {
	speed, length, opacity := tracerStyle(t.Kind)
	distance := rl.Vector3Distance(t.From, t.To)
	if distance < .001 {
		return t.To, t.To, 0
	}
	direction := rl.Vector3Scale(rl.Vector3Subtract(t.To, t.From), 1/distance)
	travel := speed * (t.Age + 1.0/120)
	front := min(distance, travel)
	back := min(distance, max(float32(0), travel-length))
	alpha = opacity * max(float32(0), 1-t.Age/t.lifetime())
	return rl.Vector3Add(t.From, rl.Vector3Scale(direction, back)), rl.Vector3Add(t.From, rl.Vector3Scale(direction, front)), alpha
}

func (s *combatState) addTrace(from, to rl.Vector3, kind weaponKind) {
	if len(s.Traces) >= maxBulletTraces {
		copy(s.Traces, s.Traces[1:])
		s.Traces = s.Traces[:len(s.Traces)-1]
	}
	s.Traces = append(s.Traces, bulletTrace{From: from, To: to, Kind: kind})
}

func (s *combatState) advanceTraces(dt float32) {
	s.Flash.Age += max(float32(0), dt)
	live := s.Traces[:0]
	for _, trace := range s.Traces {
		trace.Age += max(float32(0), dt)
		if trace.Age < trace.lifetime() {
			live = append(live, trace)
		}
	}
	s.Traces = live
}

func (s *combatState) drawTraces() {
	for _, trace := range s.Traces {
		tail, head, alpha := trace.segment()
		if alpha <= 0 || rl.Vector3DistanceSqr(tail, head) < .000001 {
			continue
		}
		// Three fading portions, not a full-length beam or lingering laser.
		for i := 0; i < 3; i++ {
			a := rl.Vector3Lerp(tail, head, float32(i)/3)
			b := rl.Vector3Lerp(tail, head, float32(i+1)/3)
			rl.DrawLine3D(a, b, rl.NewColor(255, 224, 170, uint8(alpha*float32(i+1)/3)))
		}
	}
	flash := s.Flash
	if flash.Age >= .022 || rl.Vector3LengthSqr(flash.Direction) < .5 {
		return
	}
	fade := 1 - flash.Age/.022
	length := float32(.12)
	if flash.Kind == shotgun || flash.Kind == sniperRifle {
		length = .19
	}
	end := rl.Vector3Add(flash.From, rl.Vector3Scale(flash.Direction, length))
	rl.DrawLine3D(flash.From, end, rl.NewColor(255, 235, 190, uint8(230*fade)))
	// Small irregular radial tongues replace the old large glowing sphere.
	side := rl.Vector3CrossProduct(flash.Direction, rl.Vector3{Y: 1})
	if rl.Vector3LengthSqr(side) < .001 {
		side = rl.Vector3{X: 1}
	}
	side = rl.Vector3Normalize(side)
	up := rl.Vector3CrossProduct(side, flash.Direction)
	for i := 0; i < 3; i++ {
		angle := float64(i) * 2 * math.Pi / 3
		off := rl.Vector3Add(rl.Vector3Scale(side, float32(math.Cos(angle))*.027), rl.Vector3Scale(up, float32(math.Sin(angle))*.027))
		rl.DrawLine3D(flash.From, rl.Vector3Add(rl.Vector3Lerp(flash.From, end, .6), off), rl.NewColor(255, 175, 75, uint8(160*fade)))
	}
}

// The bone attachment's world matrix tracks the animated grip, including ADS
// and crouch. Use the preceding rendered pose at the instant the shot is fired.
func (a *combatActors) muzzle(kind weaponKind, fallback rl.Vector3) rl.Vector3 {
	out := fallback
	a.weaponWorld.Each(func(_ ecs.Entity, held *heldPistol, world *transform.GlobalTransform) {
		if !held.Visible || held.Kind != kind {
			return
		}
		local := [...]rl.Vector3{{X: .10, Y: .02, Z: .06}, {X: .43, Y: .037, Z: .055}, {X: .53, Y: .02865, Z: .04}, {X: .48, Y: .0325, Z: .045}}[kind]
		at := rl.Vector3Transform(local, world.Matrix)
		// A just-spawned/switched attachment may not have propagated yet.
		if rl.Vector3DistanceSqr(at, fallback) < 4 {
			out = at
		}
	})
	return out
}
