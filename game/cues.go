package game

import (
	"math"
	"math/rand/v2"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/audio"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// ears is what every cue system needs, as one parameter: the sounds, the
// world's soundscape, the audio, the camera that hears it all, the menus
// (which turn the world down) and the clock.
type ears struct {
	bank  illusion.Res[soundBank]
	scape illusion.Res[soundscape]
	au    audio.Audio
	view  illusion.Res[render.View3D]
	menu  illusion.Res[menu]
	clock illusion.Res[illusion.Time]
}

func (e *ears) InitParam(w *ecs.World) {
	e.bank.InitParam(w)
	e.scape.InitParam(w)
	e.au.InitParam(w)
	e.view.InitParam(w)
	e.menu.InitParam(w)
	e.clock.InitParam(w)
}

// voice is how the world's one-shots are played this frame.
func (e *ears) voice() voice {
	duck := float32(1)
	if e.menu.Get().screen() != playing {
		duck = menuDuck
	}
	return voice{bank: e.bank.Get(), au: &e.au, ear: listenerOf(e.view.Get()), duck: duck}
}

func (e *ears) dt() float32 { return e.clock.Get().DeltaSecs() }

// Earshot is how far off a person's sounds are heard: their steps and what
// they do.
const (
	earNear = 3
	earFar  = 28
)

// bodyMemory is what a character was doing last frame, to hear what's
// changed.
type bodyMemory struct {
	anim     character.Anim
	grounded bool
	mode     character.Anim
	// fall is the fastest it's been falling since it left the ground (m/s,
	// down), for how hard it lands.
	fall float32
	// clank counts down to the next knock of a tool, working on a machine.
	clank float32
}

// someone is a character the cues hear: its root, and where its feet are.
type someone struct {
	root ecs.Entity
	tr   *transform.Transform
	cc   *physics.CharacterController
	tv   *character.Traversal
	feet rl.Vector3
}

// crowd is the characters and their bodies, as one parameter.
type crowd struct {
	roots   illusion.Query3Where[transform.Transform, physics.CharacterController, character.Traversal, illusion.With[character.Character]]
	bodies  illusion.Query1Where[character.State, illusion.With[character.Body]]
	fam     illusion.Res[character.Family]
	terrain illusion.Query1[terrainBody]
	phys    physics.Physics
}

func (p *crowd) InitParam(w *ecs.World) {
	p.roots.InitParam(w)
	p.bodies.InitParam(w)
	p.fam.InitParam(w)
	p.terrain.InitParam(w)
	p.phys.InitParam(w)
}

// each calls fn for every character with a body.
func (p *crowd) each(fn func(s someone, body ecs.Entity, st *character.State)) {
	p.roots.Each(func(root ecs.Entity, tr *transform.Transform, cc *physics.CharacterController, tv *character.Traversal) {
		feet := rl.Vector3Subtract(tr.Translation, rl.Vector3{Y: cc.Height / 2})
		p.fam.Get().EachChild(root, func(c ecs.Entity) {
			if st, ok := p.bodies.Get(c); ok {
				fn(someone{root, tr, cc, tv, feet}, c, st)
			}
		})
	})
}

// underfoot is what s stands on.
func (p *crowd) underfoot(scape *soundscape, s someone) footing {
	hit, ok := p.phys.CastRayExcluding(rl.Vector3Add(s.feet, rl.Vector3{Y: .3}), rl.Vector3{Y: -1}, .8, s.root)
	onKit := false
	if ok {
		_, ground := p.terrain.Get(hit.Entity)
		onKit = !ground
	}
	return scape.surfaceAt(s.feet, onKit)
}

// bodyCues plays what characters do with their whole body: leaving the
// ground and landing, rolls and slides, wall kicks, climbing, punches,
// sitting, working on a machine. And the dust a landing, a roll or a
// slide kicks up.
func bodyCues(e *ears, ps *crowd, fx *illusion.Res[effects], used *illusion.Res[uses], mem *illusion.Local[map[ecs.Entity]*bodyMemory]) {
	if !e.view.Get().Active {
		return
	}
	// The Exchange's floor bell, rung (use.go): heard right across the
	// floor and out of the doors.
	if u, ok := used.TryGet(); ok && u.rung != nil {
		if scape := e.scape.Get(); scape.hasBell {
			e.voice().at("bell", scape.bell, 1, 10, 120, 1)
		}
		u.rung = nil
	}
	if *mem.Get() == nil {
		*mem.Get() = map[ecs.Entity]*bodyMemory{}
	}
	memory := *mem.Get()
	v, scape, dt, fxs := e.voice(), e.scape.Get(), e.dt(), fx.Get()
	seen := map[ecs.Entity]bool{}
	ps.each(func(s someone, body ecs.Entity, st *character.State) {
		seen[body] = true
		m := memory[body]
		if m == nil {
			m = &bodyMemory{anim: st.Current, grounded: s.cc.Grounded, mode: s.tv.Mode}
			memory[body] = m
		}
		cur := st.Current
		at := rl.Vector3Add(s.feet, rl.Vector3{Y: 1})
		play := func(name string, volume, pitch float32) { v.at(name, at, volume, earNear, earFar, pitch) }
		if !s.cc.Grounded && s.cc.Velocity.Y < 0 {
			m.fall = max(m.fall, -s.cc.Velocity.Y)
		}

		// Leaving the ground: a jump's take-off (a fall off an edge is
		// quiet).
		if m.grounded && !s.cc.Grounded && s.cc.Velocity.Y > 1 && !s.cc.Controlled {
			play("cloth", .5, 1)
		}
		// Landing, from a jump or a fall: as hard as it was falling. (A
		// step down a kerb isn't a landing.)
		if !m.grounded && s.cc.Grounded && !s.cc.Controlled && m.fall > 2.5 {
			under := ps.underfoot(scape, s)
			k := clamp01((m.fall - 2.5) / 6)
			name, pitch := under.step()
			play(name, .6+.4*k, pitch*.9)
			if under.loose() {
				play("land_loose", .3+.6*k, 1)
				fxs.ring(s.feet, 8+int(16*k), .6+.8*k, lightAt(at))
			} else {
				play("land_hard", .25+.6*k, 1)
			}
		}
		if s.cc.Grounded {
			m.fall = 0
		}

		// What the body's started doing.
		if cur != m.anim {
			switch cur {
			case character.Roll:
				play("whoosh", .5, .8)
				play("cloth", .6, .9)
			case character.Slide:
				play("slide", .8, 1)
			case character.WallKick, character.WallKickRight:
				play("thump", .7, 1)
				play("cloth", .5, 1.1)
			case character.Vault, character.Mantle, character.LadderEnter:
				play("grab", .7, 1)
				play("cloth", .4, 1)
			case character.Punch, character.PunchRight:
				play("whoosh", .45, 1.25)
			case character.Interact, character.PickUp:
				play("cloth", .4, 1.1)
			case character.SitDown:
				play("creak", .5, 1)
			case character.SitUp:
				play("creak", .35, .9)
			case character.Fix:
				m.clank = .4
			}
		}
		// A slide or a roll on loose ground trails dust.
		if (cur == character.Slide || cur == character.Roll) && s.cc.Grounded {
			if under := ps.underfoot(scape, s); under.loose() {
				fxs.trail(s.feet, s.cc.Velocity, dt, lightAt(at))
			}
		}
		// Working on a machine: a tool knocking now and then.
		if cur == character.Fix {
			if m.clank -= dt; m.clank <= 0 {
				play("clank", .5, 1)
				m.clank = .7 + 1.1*rand.Float32()
			}
		}
		m.anim, m.grounded, m.mode = cur, s.cc.Grounded, s.tv.Mode
	})
	for b := range memory {
		if !seen[b] {
			delete(memory, b)
		}
	}
}

// foot marks an empty entity on a body's foot bone, to hear its steps by.
type foot struct {
	// h is how high the foot was over the character's soles last frame,
	// low the lowest it's been lately (where it plants), and since how long
	// since it last stepped. lifted is whether it's come up off the ground
	// since.
	h, low, since float32
	lifted, ready bool
}

// The feet: a step is the foot coming down to within stepClear of where it
// plants, having been lifted at least stepLift above it. Where it plants is the
// lowest it's been, rising slowly (lowRise m/s) so a change of ground or a
// clip that holds it higher is learned again. stepGap is the least time
// between one foot's steps.
const (
	stepClear = .035
	stepLift  = .06
	lowRise   = .05
	stepGap   = .22
)

// The foot bones, in the MakeHuman skeleton (see character/contacts.go).
var footBones = []string{"foot_l", "foot_r"}

// footCues gives every body a pair of feet to hear, and plays a step each
// time one plants: what's underfoot, as loud as the gait. A step on sand
// or soil puffs a little dust.
func footCues(
	e *ears,
	ps *crowd,
	feet *illusion.Query2[foot, transform.GlobalTransform],
	fx *illusion.Res[effects],
	cmd *illusion.Commands,
	given *illusion.Local[map[ecs.Entity]bool],
) {
	if *given.Get() == nil {
		*given.Get() = map[ecs.Entity]bool{}
	}
	shod := *given.Get()
	v, scape, dt, fxs := e.voice(), e.scape.Get(), e.dt(), fx.Get()
	active := e.view.Get().Active
	ps.each(func(s someone, body ecs.Entity, st *character.State) {
		if !shod[body] {
			shod[body] = true
			for _, bone := range footBones {
				cmd.Spawn(illusion.C(foot{}), illusion.C(transform.Identity()), illusion.C(render.BoneAttachment{Bone: bone})).ChildOf(body)
			}
			return
		}
		walking := s.cc.Grounded && !s.cc.Controlled && stepping(st.Current)
		speed := float32(math.Hypot(float64(s.cc.Velocity.X), float64(s.cc.Velocity.Z)))
		ps.fam.Get().EachChild(body, func(c ecs.Entity) {
			f, g, ok := feet.Get(c)
			if !ok {
				return
			}
			if f.track(g.Translation().Y-s.feet.Y, dt) && walking && speed > .3 && active {
				under := ps.underfoot(scape, s)
				name, pitch := under.step()
				volume := .35 + .5*clamp01((speed-1)/4)
				if s.cc.Height < crouchedBelow {
					volume *= .5 // crouched, treading softly
				}
				at := g.Translation()
				v.at(name, at, volume, earNear, earFar, pitch)
				if under.loose() && rectDistance(domeWalls, at.X, at.Z) > 0 {
					fxs.puff(at, s.cc.Velocity, speed, lightAt(at))
				}
			}
		})
	})
}

// track follows the foot to h over the soles, dt on, and reports whether
// it's just stepped: come down to where it plants, having been lifted,
// stepGap or more since its last step.
func (f *foot) track(h, dt float32) bool {
	f.since += dt
	if !f.ready {
		f.h, f.low, f.ready = h, h, true
		return false
	}
	f.low = min(h, f.low+lowRise*dt)
	if h > f.low+stepLift {
		f.lifted = true
	}
	f.h = h
	if !f.lifted || h > f.low+stepClear {
		return false
	}
	f.lifted = false
	if f.since < stepGap {
		return false
	}
	f.since = 0
	return true
}

// crouchedBelow is the capsule's height (m) under which a character is
// crouched: crouching shortens it from its full 1.8.
const crouchedBelow = 1.5

// stepping reports whether a body doing a is on its feet and could be
// walking: not in the air, acting, climbing, getting over something, sat
// or holding a pose.
func stepping(a character.Anim) bool {
	if a.Airborne() || a.OneShot() || a.Held() {
		return false
	}
	switch a {
	case character.Slide, character.Roll, character.LadderClimb, character.LadderEnter, character.LadderExit,
		character.Vault, character.Mantle, character.WallKick, character.WallKickRight,
		character.WallFall, character.WallFallRight, character.WallLand,
		character.Drive, character.Ride, character.SitDown, character.Sitting, character.SitUp:
		return false
	}
	return true
}

// uiMemory is how the menus and the map were last frame.
type uiMemory struct {
	depth  int
	top    screen
	focus  int
	outfit character.Outfit
	marked bool
	dest   rl.Vector2
	note   string
	set    bool
}

// uiCues plays the menus and the map as paperwork: a tick as the focus
// moves, a stamp as a choice is made, a page turned to open one, a book
// closed going back; a pencil marking the map, a chime arriving where it
// was marked. And a blip for a note the HUD can't do what was asked.
func uiCues(
	e *ears,
	outfits *illusion.Query1Where[character.Outfit, illusion.With[character.Player]],
	wm *illusion.Res[worldMap],
	prompt *illusion.Res[vehicle.Prompt],
	mem *illusion.Local[uiMemory],
) {
	m, mu, v := mem.Get(), e.menu.Get(), e.voice()
	var outfit character.Outfit
	outfits.Each(func(_ ecs.Entity, o *character.Outfit) { outfit = *o })
	depth, top, focus := len(mu.stack), mu.screen(), 0
	if depth > 0 {
		focus = mu.stack[depth-1].focus
	}
	var marked bool
	var dest rl.Vector2
	if w, ok := wm.TryGet(); ok {
		marked, dest = w.marked, w.dest
	}
	note := prompt.Get().Noting()
	if !m.set {
		*m = uiMemory{depth, top, focus, outfit, marked, dest, note, true}
		return
	}
	switch {
	case depth > m.depth && top == mapping:
		v.ui("ui_open", .8)
	case depth > m.depth:
		v.ui("ui_page", .7)
	case depth < m.depth && depth == 0:
		// Into play, or back to it.
		v.ui("ui_stamp", .9)
	case depth < m.depth:
		v.ui("ui_back", .6)
	case top != m.top:
		v.ui("ui_stamp", .9)
	case focus != m.focus:
		v.ui("ui_move", .6)
	}
	if outfit != m.outfit && top == dressing {
		v.ui("cloth", .9)
	}
	switch {
	case marked && (!m.marked || dest != m.dest):
		v.ui("ui_mark", .8)
	case !marked && m.marked && top == mapping:
		v.ui("ui_mark", .6)
	case !marked && m.marked:
		v.ui("ui_arrive", .8)
	}
	if note != "" && note != m.note {
		v.ui("ui_deny", .7)
	}
	*m = uiMemory{depth, top, focus, outfit, marked, dest, note, true}
}
