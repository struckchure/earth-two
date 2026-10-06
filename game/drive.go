package game

import (
	"fmt"
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

// Driving: the camera follows the vehicle, not the player in it, further
// back for a bigger one, and swings round behind it sooner than behind
// someone walking. The vehicles themselves are in the vehicle package.
const (
	// chaseDelay is how long after the mouse last moved the camera starts
	// to swing round behind the vehicle, and chaseRate how quickly.
	chaseDelay = .6
	chaseRate  = 3
	// chasePitch is how far it looks down at a vehicle.
	chasePitch = 12 * math.Pi / 180
	// chaseSpeed is how fast (m/s) a vehicle goes forward before the camera
	// follows it round: not reversing, or it would swing to its front.
	chaseSpeed = 1.5
)

// steerDriving points the camera behind the vehicle the player's driving,
// and has it follow at the vehicle's distance, clear of it.
func steerDriving(o *illusion.Res[orbit], m *illusion.Res[menu], driving *illusion.Res[vehicle.Driving], t *illusion.Res[illusion.Time]) {
	or, d := o.Get(), driving.Get()
	if !d.Active() {
		or.distance, or.exclude = 0, ecs.Entity{}
		return
	}
	or.distance, or.exclude = d.Camera, d.Vehicle
	if m.Get().screen() != playing {
		return
	}
	f := rl.Vector3RotateByQuaternion(rl.Vector3{Z: 1}, d.Pose.Rotation)
	or.chase(float32(math.Atan2(float64(f.X), float64(f.Z))), d.Speed > chaseSpeed, t.Get().DeltaSecs())
}

// chase swings the orbit, over dt seconds, round behind a vehicle facing
// the yaw facing, if it's going forward and the mouse has been left alone a
// moment.
func (o *orbit) chase(facing float32, moving bool, dt float32) {
	o.still += dt
	if !moving || o.still < chaseDelay {
		return
	}
	k := float32(1 - math.Exp(-chaseRate*float64(dt)))
	o.yaw = wrapYaw(o.yaw + wrapYaw(facing+math.Pi-o.yaw)*k)
	o.pitch += (chasePitch - o.pitch) * k
}

// avoid is what the camera's spring arm passes through: the vehicle being
// driven, or else the player.
func (o *orbit) avoid(player ecs.Entity) ecs.Entity {
	if !o.exclude.IsZero() {
		return o.exclude
	}
	return player
}

// coverGround tells the vehicles where there's ground to collide with: in
// the terrain's colliders round the player, a margin inside their edge. A
// vehicle left anywhere else is parked at once, before the ground under it
// goes.
func coverGround(tr *illusion.Res[terrain], ground *illusion.Res[vehicle.Ground]) {
	t, ok := tr.TryGet()
	if !ok {
		return
	}
	g := ground.Get()
	if g.Covered != nil {
		return
	}
	const margin = 24
	g.Covered = func(x, z float32) bool {
		lo := rl.Vector2{X: float32(t.ci-colliderRadius) * chunkSize, Y: float32(t.cj-colliderRadius) * chunkSize}
		hi := rl.Vector2{X: float32(t.ci+colliderRadius+1) * chunkSize, Y: float32(t.cj+colliderRadius+1) * chunkSize}
		return x > lo.X+margin && x < hi.X-margin && z > lo.Y+margin && z < hi.Y-margin
	}
}

// cullVehicles hides the vehicles' bodies and wheels the camera can't see,
// as cull does the world's pieces (which it doesn't do for them: they're
// children of the vehicles' physics bodies).
func cullVehicles(
	cmd *illusion.Commands,
	cameras *illusion.Query2[transform.Transform, render.Camera3d],
	shells *illusion.Query2Where[render.Model3d, transform.GlobalTransform, illusion.With[vehicle.Shell]],
	wheels *illusion.Query2Where[render.Model3d, transform.GlobalTransform, illusion.With[vehicle.WheelOf]],
	models *illusion.Res[asset.Assets[render.Model]],
	win *illusion.Res[window.Window],
	state *illusion.Local[culling],
) {
	_, eye, cam, ok := cameras.Single()
	if !ok {
		return
	}
	v := viewFrom(*eye, *cam, win.Get())
	s := state.Get()
	if s.bounds == nil {
		s.bounds, s.drawn = map[asset.Handle[render.Model]]sphere{}, map[ecs.Entity]drawn{}
	}
	store := models.Get()
	part := func(e ecs.Entity, m *render.Model3d, g *transform.GlobalTransform) {
		b, ok := s.bounds[m.Model]
		if !ok {
			model := store.Get(m.Model)
			if model == nil {
				return
			}
			box := rl.GetModelBoundingBox(model.Model)
			b = sphere{center: rl.Vector3Scale(rl.Vector3Add(box.Min, box.Max), .5), radius: rl.Vector3Distance(box.Min, box.Max) / 2}
			s.bounds[m.Model] = b
		}
		center := rl.Vector3Transform(b.center, g.Matrix)
		switch {
		case v.sees(center, b.radius, sight(b.radius)):
			s.show(cmd, e, seen)
		case rl.Vector3Distance(center, v.at)-b.radius < shadowReach:
			s.show(cmd, e, shadowOnly)
		default:
			s.show(cmd, e, unseen)
		}
	}
	shells.Each(part)
	wheels.Each(part)
}

// viewFrom is what a camera at eye sees in a window.
func viewFrom(eye transform.Transform, cam render.Camera3d, ww *window.Window) view {
	fovy := cam.Fovy
	if fovy == 0 {
		fovy = 45
	}
	v := view{
		at:     eye.Translation,
		ahead:  eye.Forward(),
		up:     rl.Vector3RotateByQuaternion(transform.Up, eye.Rotation),
		tanV:   float32(math.Tan(float64(fovy) * math.Pi / 360)),
		aspect: float32(ww.Width) / max(float32(ww.Height), 1),
	}
	v.right = rl.Vector3CrossProduct(v.ahead, v.up)
	return v
}

// drawDriving draws what the player can do with a vehicle (a key and what
// it does, above the menu keys), a passing note, and while driving, the
// speed: square, in the bottom right.
func drawDriving(p painter, pr *vehicle.Prompt, d *vehicle.Driving, width, height float32) {
	y := height - p.px(84)
	if pr.Key != "" {
		x := p.px(20)
		x += p.keycap(pr.Key, rl.Vector2{X: x, Y: y}, 14) + p.px(8)
		p.text(pr.Text, rl.Vector2{X: x + 1, Y: y + p.px(4) + 1}, 15, semibold, rl.NewColor(0, 0, 0, 110))
		p.text(pr.Text, rl.Vector2{X: x, Y: y + p.px(4)}, 15, semibold, colText)
		y -= p.px(30)
	}
	if note := pr.Noting(); note != "" {
		p.text(note, rl.Vector2{X: p.px(20) + 1, Y: y + p.px(4) + 1}, 15, semibold, rl.NewColor(0, 0, 0, 110))
		p.text(note, rl.Vector2{X: p.px(20), Y: y + p.px(4)}, 15, semibold, colAccent)
	}
	if !d.Active() {
		return
	}
	kmh := fmt.Sprintf("%.0f", math.Abs(float64(d.Speed))*3.6)
	// Coasting off the throttle the gearbox drops to neutral; that's N
	// only at a standstill.
	gear := ""
	switch {
	case d.Gear < 0:
		gear = "R"
	case d.Gear > 0:
		gear = fmt.Sprint(d.Gear)
	case math.Abs(float64(d.Speed)) < 0.5:
		gear = "N"
	}
	box := rl.Rectangle{X: width - p.px(150), Y: height - p.px(104), Width: p.px(130), Height: p.px(84)}
	p.panel(box)
	p.textIn(kmh, rl.Rectangle{X: box.X + p.px(14), Y: box.Y + p.px(8), Width: p.px(76), Height: p.px(46)}, 38, semibold, colText, left)
	p.textIn(gear, rl.Rectangle{X: box.X + box.Width - p.px(44), Y: box.Y + p.px(8), Width: p.px(30), Height: p.px(46)}, 22, semibold, colAccent, right)
	p.textIn("km/h · "+d.Name, rl.Rectangle{X: box.X + p.px(14), Y: box.Y + p.px(54), Width: box.Width - p.px(28), Height: p.px(20)}, 12, semibold, colMuted, left)
}
