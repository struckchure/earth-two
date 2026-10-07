package character

import (
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// MotionSamples keeps the last two physics positions for visual interpolation.
// The root and controller keep their authoritative physics position.
type MotionSamples struct {
	previous, current rl.Vector3
	height            float32
	// span is how many fixed steps apart previous and current were (more
	// than one for a character stepped only every so often: see
	// physics.CharacterController.Every), and since how many have passed
	// since current.
	span, since int
}

// Position returns the position to draw at between physics steps. An external
// move or a teleport is shown immediately instead of streaking across the map.
func (s *MotionSamples) Position(current rl.Vector3, alpha float32) rl.Vector3 {
	if current != s.current || rl.Vector3DistanceSqr(s.previous, s.current) > 1 {
		return current
	}
	return rl.Vector3Lerp(s.previous, s.current, clamp((float32(s.since)+alpha)/float32(max(s.span, 1)), 0, 1))
}

// Speed is how fast the character really went over the ground when last
// stepped, physics steps being dt seconds: nothing, for one walking into a
// wall.
func (s *MotionSamples) Speed(dt float32) float32 {
	if dt <= 0 {
		return 0
	}
	return float32(math.Hypot(float64(s.current.X-s.previous.X), float64(s.current.Z-s.previous.Z))) / (dt * float32(max(s.span, 1)))
}

// remember takes the position after a fixed step, if physics stepped the
// character.
func (s *MotionSamples) remember(position rl.Vector3, height float32, stepped bool) {
	if !stepped {
		s.since++
		return
	}
	// Resizing moves the capsule center, not its feet. Put the older sample
	// into the new center convention before interpolating locomotion.
	if s.height > 0 {
		s.current.Y += (height - s.height) / 2
	}
	s.previous, s.current, s.height = s.current, position, height
	s.span, s.since = s.since+1, 0
}
func rememberMotion(q *illusion.Query3Where[transform.Transform, MotionSamples, physics.CharacterController, illusion.With[Character]]) {
	q.Each(func(_ ecs.Entity, tr *transform.Transform, s *MotionSamples, cc *physics.CharacterController) {
		s.remember(tr.Translation, cc.Height, cc.Stepped || cc.Every <= 1)
	})
}

func presentMotion(
	bodies *illusion.Query1Where[transform.Transform, illusion.With[Body]],
	roots *illusion.Query3[transform.Transform, MotionSamples, physics.CharacterController],
	hier *illusion.Hierarchy,
	fixed *illusion.Res[illusion.FixedTime],
) {
	alpha := fixed.Get().Overstep()
	bodies.Each(func(e ecs.Entity, tr *transform.Transform) {
		parent, ok := hier.Parent(e)
		if !ok {
			return
		}
		root, samples, cc, ok := roots.Get(parent)
		if !ok {
			return
		}
		offset := rl.Vector3Subtract(samples.Position(root.Translation, alpha), root.Translation)
		tr.Translation = rl.Vector3Add(rl.Vector3{Y: -cc.Height / 2}, offset)
	})
}
