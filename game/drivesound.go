package game

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// The vehicle the player drives: its motor, pitched by its revs and louder
// with the throttle; its tyres on the ground and their skids; its door; and
// what it hits. And the dust its wheels throw up off sand and soil, and
// sparks off metal it hits hard. Parked vehicles are quiet.

// engineSound is each kind of vehicle's motor, its own: the bike's
// single buzzing high, the trike's quad engine, the buggy's synthetic
// growl, the rover's electric drive humming low, the hauler's diesel. Each
// is pitched idle to red line from pitch to pitch*rev, and its volume rises
// with the throttle from idle.
var engineSound = map[string]struct {
	loop             string
	pitch, rev       float32
	idle, throttling float32
}{
	"bike":  {"engine_bike", .9, 2.1, .4, .55},
	"trike": {"engine_trike", .85, 1.9, .45, .5},
	"buggy": {"engine_buggy", .8, 1.8, .45, .5},
	"rover": {"engine_rover", .6, 1.5, .5, .35},
	"truck": {"engine_truck", .75, 1.45, .6, .4},
}

// engineLoops are all the motors' loops, so the ones not driven are kept
// quiet.
var engineLoops = []string{"engine_bike", "engine_trike", "engine_buggy", "engine_rover", "engine_truck"}

// driveMemory is how the driving was last frame.
type driveMemory struct {
	vehicle ecs.Entity
	gear    int
	// shift is how much longer the pitch dips for a gear change, skid how
	// long until another skid can sound, and crash another crash.
	shift, skid, crash float32
	// dust is the share of a puff each wheel has built up, to emit them
	// steadily however fast frames come.
	dust  []float32
	loops loops
}

// cars is the vehicles, as one parameter.
type cars struct {
	q       illusion.Query5[vehicle.Drivable, physics.VehicleInput, physics.VehicleState, transform.Transform, physics.Velocity]
	terrain illusion.Query1[terrainBody]
	hits    illusion.EventReader[physics.CollisionStarted]
}

func (c *cars) InitParam(w *ecs.World) {
	c.q.InitParam(w)
	c.terrain.InitParam(w)
	c.hits.InitParam(w)
}

// The skid: a sideways slide faster than skidSlip m/s, or the hand brake
// at speed. A crash is a hit at crashSpeed m/s or more; sparks fly off
// metal at sparkSpeed.
const (
	skidSlip   = 3.5
	crashSpeed = 2.5
	sparkSpeed = 7
	// wheelDust is how many puffs a wheel throws up each second, on loose
	// ground at dustSpeed m/s and faster.
	wheelDust = 18
	dustSpeed = 10
)

func driveCues(
	e *ears,
	driving *illusion.Res[vehicle.Driving],
	vs *cars,
	fx *illusion.Res[effects],
	mem *illusion.Local[driveMemory],
) {
	m, d, bank, scape, dt, fxs := mem.Get(), driving.Get(), e.bank.Get(), e.scape.Get(), e.dt(), fx.Get()
	v := e.voice()
	playing := e.menu.Get().screen() == playing
	m.shift = max(0, m.shift-dt)
	m.skid = max(0, m.skid-dt)
	m.crash = max(0, m.crash-dt)
	quiet := func() {
		for _, s := range append(engineLoops, "tyres") {
			m.loops.set(bank, &e.au, s, 0, 0, 0, dt)
		}
	}

	// Getting in and out: a door shut on a car, the seat taken on a bike.
	if d.Vehicle != m.vehicle {
		e := d.Vehicle
		if e.IsZero() {
			e = m.vehicle
		}
		if dr, _, _, tr, _, ok := vs.q.Get(e); ok {
			if dr.Handling.Seat == character.Drive {
				v.at("door", tr.Translation, .9, 4, 30, 1)
			} else {
				v.at("cloth", tr.Translation, .7, 4, 30, 1)
			}
		}
		m.vehicle, m.gear, m.dust = d.Vehicle, d.Gear, nil
	}
	if !d.Active() {
		quiet()
		vs.hits.Clear()
		return
	}
	dr, in, st, tr, vel, ok := vs.q.Get(d.Vehicle)
	if !ok {
		quiet()
		return
	}
	h := dr.Handling
	at := tr.Translation
	speed := abs(st.Speed)

	// The motor: its revs between idle and the red line set its pitch, and
	// a gear change dips it for a moment.
	if st.Gear != m.gear {
		if st.Gear != 0 && m.gear != 0 {
			m.shift = .18
		}
		m.gear = st.Gear
	}
	snd, ok := engineSound[dr.Spec.Handling]
	if !ok {
		snd = engineSound["buggy"]
	}
	rev := clamp01((st.RPM - h.MinRPM) / max(1, h.MaxRPM-h.MinRPM))
	pitch := snd.pitch * (1 + (snd.rev-1)*rev) * (1 - .12*m.shift/.18)
	throttle := clamp01(abs(in.Forward))
	engine := (snd.idle + snd.throttling*throttle + .1*rev) * mixEffects
	if !playing {
		engine = 0
	}
	for _, s := range engineLoops {
		vol := float32(0)
		if s == snd.loop {
			vol = engine
		}
		m.loops.set(bank, &e.au, s, vol, pitch, 0, dt)
	}

	// The tyres: their roll, as loud as the speed and the share of the
	// wheels on the ground; softer on paving.
	touching := float32(st.Touching) / float32(max(1, len(st.Wheels)))
	under := scape.surfaceAt(at, false)
	roll := clamp01(speed/18) * touching * .8
	if !under.loose() {
		roll *= .5
	}
	if !playing {
		roll = 0
	}
	m.loops.set(bank, &e.au, "tyres", roll*mixEffects, .8+.5*clamp01(speed/25), 0, dt)

	// Skids: sliding sideways, or the hand brake on at speed.
	right := rl.Vector3RotateByQuaternion(rl.Vector3{X: 1}, tr.Rotation)
	slip := abs(rl.Vector3DotProduct(vel.Linear, right))
	if m.skid == 0 && touching > .4 && (slip > skidSlip || (in.HandBrake > .5 && speed > 6)) {
		v.at("skid", at, .5+.4*clamp01(slip/10), 3, 40, 1)
		m.skid = 1.2
	}

	// Dust off each wheel on the ground, on sand and soil, as fast as the
	// vehicle's going or its wheels are spinning.
	if len(m.dust) != len(st.Wheels) {
		m.dust = make([]float32, len(st.Wheels))
	}
	for i, w := range st.Wheels {
		if !w.Contact || i >= len(dr.Spec.Wheels) {
			continue
		}
		spec := dr.Spec.Wheels[i]
		radius := orOne(spec.Radius)
		pos := rl.Vector3Add(at, rl.Vector3RotateByQuaternion(w.Transform.Translation, tr.Rotation))
		ground := rl.Vector3Add(pos, rl.Vector3{Y: -radius * .8})
		if !scape.surfaceAt(ground, false).loose() {
			continue
		}
		spin := abs(w.Spin * radius)
		k := clamp01(max(speed, spin) / dustSpeed)
		if k < .1 {
			continue
		}
		m.dust[i] += wheelDust * k * dt
		for m.dust[i] >= 1 {
			m.dust[i]--
			fxs.wheel(ground, vel.Linear, k, lightAt(ground))
		}
	}

	// Crashes: what it hit, as hard as it was going into it.
	for hit := range vs.hits.Read() {
		other, ok := hit.Involves(d.Vehicle)
		if !ok || m.crash > 0 {
			continue
		}
		into := abs(rl.Vector3DotProduct(vel.Linear, hit.Normal))
		if into < crashSpeed {
			continue
		}
		m.crash = .3
		k := clamp01((into - crashSpeed) / 12)
		if _, ground := vs.terrain.Get(other); ground {
			v.at("crash_ground", hit.Point, .5+.5*k, 4, 60, 1)
			fxs.ring(hit.Point, 10+int(20*k), .8+k, lightAt(hit.Point))
			continue
		}
		v.at("crash_metal", hit.Point, .5+.5*k, 4, 60, 1-.15*k)
		if into > sparkSpeed {
			fxs.sparks(hit.Point, hit.Normal, 12+int(24*k))
		}
	}
}
