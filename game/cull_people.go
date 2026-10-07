package game

import (
	"cmp"
	"math"
	"slices"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

// People are culled as the world's pieces are (cull.go), but whole: a body
// and everything it wears together, and in levels of detail, as a crowd
// needs. A person the engine draws is posed (skinned, on the CPU, once for
// each pass that draws them) every frame, and one that's hidden isn't.
//
//   - In view out to peopleSight, a person's drawn; out of view but within
//     peopleShadow, only into the shadow map (so shadows don't pop as the
//     camera turns); otherwise not at all.
//   - Only those within peopleCast cast shadows.
//   - Past peopleNear, a person is character.Distant (posed in step with
//     the rest of the crowd, so a mesh is skinned once for them all) and
//     has no outline (a second draw of each of their models).
//   - Physics steps a person less often the further off they are, and
//     less often still out of view (physics.CharacterController.Every):
//     a crowd's character controllers are most of its cost otherwise.
//   - Only the clothBudget nearest in view, within clothRange, have
//     clothes that move with physics (render.Cloth, a person's dearest
//     part by far); everyone else's move with their skeleton alone. In a
//     crowd, that's the few closest.
//
// The player is always drawn in full.
const (
	peopleSight  = 45
	peopleShadow = 20
	// peopleCast is how far off a person still casts a shadow: past it,
	// the shadow's small and the shadow pass would pose them all over
	// again.
	peopleCast = 12
	peopleNear = 10
	clothRange = 8
	// clothBudget is how many people's clothes move with physics at once.
	// One who has them keeps them until clothSlack more are nearer, or
	// they're clothSlack metres further off: starting cloth again builds
	// it afresh.
	clothBudget = 4
	clothSlack  = 2
	// How often physics steps a person in view past peopleNear, further
	// than peopleMid, and out of view, in fixed steps.
	peopleMid      = 25
	stepsNear      = 2
	stepsFar       = 3
	stepsOutOfView = 6
	// personRadius is a person's bounds about their middle.
	personRadius = 1.1
)

// crowdView is what cullPeople looks at: the camera, the player, the window,
// the outlines, and who's whose.
type crowdView struct {
	cameras illusion.Query2[transform.Transform, render.Camera3d]
	players illusion.Query1Where[transform.Transform, illusion.With[character.Player]]
	win     illusion.Res[window.Window]
	passes  illusion.Query1[render.Passes]
	cloth   illusion.Query1[render.Cloth]
	people  illusion.Query1[character.Body]
	moves   illusion.Query1[physics.CharacterController]
	hier    illusion.Hierarchy
	fam     illusion.Res[character.Family]
}

func (p *crowdView) InitParam(w *ecs.World) {
	p.cameras.InitParam(w)
	p.players.InitParam(w)
	p.win.InitParam(w)
	p.passes.InitParam(w)
	p.cloth.InitParam(w)
	p.people.InitParam(w)
	p.moves.InitParam(w)
	p.hier.InitParam(w)
	p.fam.InitParam(w)
}

// peopleCulling is cullPeople's memory: how it's told the engine to draw
// each person's models, which cast shadows, which bodies are distant, the
// outlines and cloth it's taken off, and who has cloth now.
type peopleCulling struct {
	culling
	casts   map[ecs.Entity]bool
	distant map[ecs.Entity]bool
	lines   map[ecs.Entity]render.Passes
	cloth   map[ecs.Entity]render.Cloth
	clothed map[ecs.Entity]bool
	near    []nearPerson
}

// nearPerson is a body in view within clothRange, and how far off.
type nearPerson struct {
	body ecs.Entity
	far  float32
}

// cullPeople shows, shadows or hides each person but the player, and sets
// how much detail they're drawn in.
func cullPeople(
	cmd *illusion.Commands,
	bodies *illusion.Query2Where[transform.GlobalTransform, character.Body, illusion.With[render.Model3d]],
	ps *crowdView,
	state *illusion.Local[peopleCulling],
) {
	_, eye, cam, ok := ps.cameras.Single()
	if !ok {
		return
	}
	player, _, _ := ps.players.Single()
	ww := ps.win.Get()
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
	st := state.Get()
	s := &st.culling
	if s.drawn == nil {
		s.drawn, st.casts = map[ecs.Entity]drawn{}, map[ecs.Entity]bool{}
		st.distant, st.lines = map[ecs.Entity]bool{}, map[ecs.Entity]render.Passes{}
		st.cloth, st.clothed = map[ecs.Entity]render.Cloth{}, map[ecs.Entity]bool{}
	}
	fam := ps.fam.Get()
	// Who gets cloth: the nearest in view, those who have it first among
	// equals (see clothSlack).
	st.near = st.near[:0]
	bodies.Each(func(e ecs.Entity, g *transform.GlobalTransform, _ *character.Body) {
		if root, _ := ps.hier.Parent(e); root == player {
			return
		}
		center := rl.Vector3Add(g.Translation(), rl.Vector3{Y: .9})
		far := rl.Vector3Distance(center, v.at)
		reach := float32(clothRange)
		if st.clothed[e] {
			reach += clothSlack
		}
		if far < reach && v.sees(center, personRadius, peopleSight) {
			st.near = append(st.near, nearPerson{e, far})
		}
	})
	slices.SortFunc(st.near, func(a, b nearPerson) int { return cmp.Compare(a.far, b.far) })
	clothed := map[ecs.Entity]bool{}
	for i, n := range st.near {
		if i < clothBudget || (st.clothed[n.body] && i < clothBudget+clothSlack) {
			clothed[n.body] = true
		}
	}
	st.clothed = clothed
	alive := map[ecs.Entity]bool{}
	bodies.Each(func(e ecs.Entity, g *transform.GlobalTransform, _ *character.Body) {
		root, _ := ps.hier.Parent(e)
		d, casts, distant, cloth := seen, true, false, true
		if root != player {
			center := rl.Vector3Add(g.Translation(), rl.Vector3{Y: .9})
			far := rl.Vector3Distance(center, v.at)
			switch {
			case v.sees(center, personRadius, peopleSight):
			case far < peopleShadow:
				d = shadowOnly
			default:
				d = unseen
			}
			casts, distant, cloth = far < peopleCast, far > peopleNear, clothed[e]
			if cc, ok := ps.moves.Get(root); ok {
				switch {
				case d != seen:
					cc.Every = stepsOutOfView
				case far > peopleMid:
					cc.Every = stepsFar
				case distant:
					cc.Every = stepsNear
				default:
					cc.Every = 1
				}
			}
		}
		if distant != st.distant[e] {
			if distant {
				cmd.Entity(e).Insert(illusion.C(character.Distant{}))
			} else {
				cmd.Entity(e).Remove(ecs.C[character.Distant]())
			}
		}
		st.distant[e] = distant
		apply := func(c ecs.Entity) {
			s.show(cmd, c, d)
			st.cast(cmd, c, casts)
			st.outline(cmd, c, !distant, &ps.passes)
			st.drape(cmd, c, cloth, &ps.cloth)
			alive[c] = true
		}
		apply(e)
		// And what they wear, with them.
		fam.EachDescendant(e, apply)
	})
	// Garments put on this frame aren't in the family yet: take their
	// cloth off before it's built (which is a few milliseconds each) if
	// whoever wears them isn't to have it.
	ps.cloth.Each(func(e ecs.Entity, _ *render.Cloth) {
		body, ok := ps.hier.Parent(e)
		if !ok || clothed[body] || ps.hier.Root(body) == player {
			return
		}
		if _, person := ps.people.Get(body); person {
			st.drape(cmd, e, false, &ps.cloth)
		}
	})
	// Forget the people who've gone.
	for e := range s.drawn {
		if !alive[e] {
			delete(s.drawn, e)
			delete(st.casts, e)
			delete(st.distant, e)
			delete(st.lines, e)
			delete(st.cloth, e)
		}
	}
}

// cast tells the engine whether e casts a shadow, if that's changed. A
// model casts one unless it's told not to, as everything does at first.
func (st *peopleCulling) cast(cmd *illusion.Commands, e ecs.Entity, casts bool) {
	was, known := st.casts[e]
	if !known {
		was = true
	}
	st.casts[e] = casts
	if casts == was {
		return
	}
	if casts {
		cmd.Entity(e).Remove(ecs.C[render.NotShadowCaster]())
	} else {
		cmd.Entity(e).Insert(illusion.C(render.NotShadowCaster{}))
	}
}

// outline takes e's outline off, keeping it, or gives it back.
func (st *peopleCulling) outline(cmd *illusion.Commands, e ecs.Entity, on bool, passes *illusion.Query1[render.Passes]) {
	kept, off := st.lines[e]
	switch {
	case on && off:
		cmd.Entity(e).Insert(illusion.C(kept))
		delete(st.lines, e)
	case !on && !off:
		if p, ok := passes.Get(e); ok {
			st.lines[e] = *p
			cmd.Entity(e).Remove(ecs.C[render.Passes]())
		}
	}
}

// drape takes e's cloth off, keeping how it moves, or gives it back: built
// afresh, from where e's skeleton has it now.
func (st *peopleCulling) drape(cmd *illusion.Commands, e ecs.Entity, on bool, cloth *illusion.Query1[render.Cloth]) {
	kept, off := st.cloth[e]
	switch {
	case on && off:
		cmd.Entity(e).Insert(illusion.C(kept))
		delete(st.cloth, e)
	case !on && !off:
		if c, ok := cloth.Get(e); ok {
			// Only what's said of it, not where it's got to.
			st.cloth[e] = render.Cloth{Meshes: c.Meshes, Colliders: c.Colliders, Gravity: c.Gravity,
				Damping: c.Damping, Stiffness: c.Stiffness, Bending: c.Bending, Thickness: c.Thickness}
			cmd.Entity(e).Remove(ecs.C[render.Cloth]())
		}
	}
}
