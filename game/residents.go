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
	"github.com/struckchure/illusion/transform"
)

// Residents are the world's people as data: where each stands, how they
// look, and what they're about. Only those near the player are characters
// (Roster.Spawn: a body, clothes, physics, animation), the nearest
// maxEmbodied within embodyRadius; a character goes back to being data
// when its resident's further off than releaseRadius, or more are nearer.
// A character costs every frame, seen or not, and a resident as data next
// to nothing: so a crowd of thousands costs about what the few hundred
// nearest do.
//
// As data, a resident goes about the same business as their character
// would (resident.think), moved a slice of the crowd at a time.
const (
	embodyRadius  = peopleSight + 10
	releaseRadius = embodyRadius + 10
	maxEmbodied   = 400
	// embodySlack more than maxEmbodied may stay characters, so the
	// nearest few at the edge don't come and go.
	embodySlack = 20
	// embodyBatch is how many characters are made a frame.
	embodyBatch = 16
	// residentSlices: residents who are data are moved one slice a frame,
	// so each every that many frames, by all the time between.
	residentSlices = 8
	residentWalk   = 1.2 // metres a second, as data
)

// resident is one of the world's people.
type resident struct {
	// feet is where they stand: their character's, while they have one.
	feet   rl.Vector3
	skin   int
	outfit character.Outfit
	health character.Health
	// They mill about home: each while, they pick a spot near it (target)
	// and walk there.
	home, target rl.Vector3
	left         float32
	step         int
}

// residents is a resource: the world's people.
type residents struct {
	list []resident
	// near is how many are characters now.
	near int
	// generation counts clears, so a character from before one isn't
	// taken for the resident put in its place.
	generation int
	// Scratch, kept between frames.
	bodies []ecs.Entity
	order  []rankedResident
	slice  int
}

type rankedResident struct {
	index int
	far   float32
}

// residentOf marks the character standing for residents.list[index], of
// residents.generation.
type residentOf struct{ index, generation int }

// add makes a resident of r.
func (rs *residents) add(r resident) { rs.list = append(rs.list, r) }

// clear sends every resident away. Their characters go at the next
// embodyResidents.
func (rs *residents) clear() {
	rs.list = rs.list[:0]
	rs.generation++
}

// truncate leaves the first n residents. Their characters go with the
// rest at the next embodyResidents.
func (rs *residents) truncate(n int) { rs.list = rs.list[:min(n, len(rs.list))] }

// think moves the resident's business on by dt seconds: when they've
// waited long enough, they pick another spot near home, which ok (if not
// nil) may turn down.
func (r *resident) think(index int, dt float32, ok func(rl.Vector3) bool) {
	r.left -= dt
	if r.left > 0 {
		return
	}
	r.step++
	r.left = 4 + float32(index%5)
	a := float64(index+r.step*3) * 2.399963229728653
	target := rl.Vector3{X: r.home.X + float32(math.Cos(a))*1.5, Y: r.home.Y, Z: r.home.Z + float32(math.Sin(a))*1.5}
	if ok == nil || ok(target) {
		r.target = target
	}
}

// embodyResidents gives the nearest residents characters and takes them
// from those further off, and keeps each resident where their character
// is.
func embodyResidents(
	cmd *illusion.Commands,
	res *illusion.Res[residents],
	roster *illusion.Res[character.Roster],
	characters *illusion.Query3[residentOf, transform.Transform, character.Health],
	controllers *illusion.Query1[physics.CharacterController],
	players *illusion.Query1Where[transform.Transform, illusion.With[character.Player]],
	mu *illusion.Res[menu],
) {
	rs := res.Get()
	// Who has a character, and where it's got to.
	rs.bodies = slices.Grow(rs.bodies[:0], len(rs.list))[:len(rs.list)]
	clear(rs.bodies)
	characters.Each(func(e ecs.Entity, of *residentOf, tr *transform.Transform, health *character.Health) {
		if of.generation != rs.generation || of.index >= len(rs.list) || !rs.bodies[of.index].IsZero() {
			cmd.Despawn(e) // their resident's gone
			return
		}
		rs.bodies[of.index] = e
		height := float32(.3)
		if cc, ok := controllers.Get(e); ok {
			height = cc.Height / 2
		}
		rs.list[of.index].feet = rl.Vector3Subtract(tr.Translation, rl.Vector3{Y: height})
		rs.list[of.index].health = *health
	})
	_, player, ok := players.Single()
	r, ready := roster.TryGet()
	if !ok || !ready || len(r.Skins) == 0 || mu.Get().screen() != playing {
		rs.near = 0
		for _, e := range rs.bodies {
			if !e.IsZero() {
				rs.near++
			}
		}
		return
	}
	// The nearest first.
	rs.order = rs.order[:0]
	for i := range rs.list {
		far := flatDistance(rs.list[i].feet, player.Translation)
		if far < releaseRadius {
			rs.order = append(rs.order, rankedResident{i, far})
		} else if e := rs.bodies[i]; !e.IsZero() {
			cmd.Despawn(e)
			rs.bodies[i] = ecs.Entity{}
		}
	}
	slices.SortFunc(rs.order, func(a, b rankedResident) int { return cmp.Compare(a.far, b.far) })
	made := 0
	rs.near = 0
	for rank, o := range rs.order {
		e := rs.bodies[o.index]
		keep := !e.IsZero() && rank < maxEmbodied+embodySlack
		want := rank < maxEmbodied && o.far < embodyRadius
		switch {
		case keep || (!e.IsZero() && want):
			rs.near++
		case !e.IsZero():
			cmd.Despawn(e)
			rs.bodies[o.index] = ecs.Entity{}
		case want && made < embodyBatch:
			who := &rs.list[o.index]
			actor := r.Spawn(cmd, who.skin, rl.Vector3Add(who.feet, rl.Vector3{Y: .05}), illusion.C(who.outfit), illusion.C(residentOf{o.index, rs.generation}))
			if who.health.State != character.Healthy {
				// A streamed corpse returns prone. Starting an upright pose at
				// torso height would initialize its ragdoll legs below the floor.
				at := transform.FromTranslation(rl.Vector3Add(who.feet, rl.Vector3{Y: .3})).
					WithRotation(rl.QuaternionFromAxisAngle(rl.Vector3{X: 1}, -math.Pi/2))
				character.KnockDown(actor, at, who.health, rl.Vector3{})
			}
			made++
			rs.near++
		}
	}
}

// moveResidents goes about the business of residents who are data, a
// slice of them a frame.
func moveResidents(res *illusion.Res[residents], clock *illusion.Res[illusion.Time], mu *illusion.Res[menu]) {
	rs := res.Get()
	if mu.Get().screen() != playing || len(rs.list) == 0 {
		return
	}
	dt := clock.Get().DeltaSecs() * residentSlices
	rs.slice = (rs.slice + 1) % residentSlices
	for i := rs.slice; i < len(rs.list); i += residentSlices {
		if i < len(rs.bodies) && !rs.bodies[i].IsZero() {
			continue // their character's doing it
		}
		r := &rs.list[i]
		if r.health.State != character.Healthy {
			continue
		}
		r.think(i, dt, nil)
		to := rl.Vector3Subtract(r.target, r.feet)
		to.Y = 0
		if d := rl.Vector3Length(to); d > .25 {
			r.feet = rl.Vector3Add(r.feet, rl.Vector3Scale(to, min(1, residentWalk*dt/d)))
		}
	}
}

// steerResidents walks each resident's character about their business.
// Physics handles contacts; a spot taken up is passed over.
func steerResidents(
	q *illusion.Query3[residentOf, character.Intent, transform.Transform],
	res *illusion.Res[residents],
	mu *illusion.Res[menu],
	clock *illusion.Res[illusion.Time],
	p *physics.Physics,
) {
	rs := res.Get()
	q.Each(func(e ecs.Entity, of *residentOf, intent *character.Intent, tr *transform.Transform) {
		*intent = character.Intent{}
		if mu.Get().screen() != playing || of.generation != rs.generation || of.index >= len(rs.list) {
			return
		}
		r := &rs.list[of.index]
		if r.health.State != character.Healthy {
			return
		}
		r.think(of.index, clock.Get().DeltaSecs(), func(at rl.Vector3) bool {
			return !p.OverlapCapsuleExcluding(rl.Vector3Add(at, rl.Vector3{Y: .95}), .35, 1.8, e)
		})
		dir := rl.Vector3Subtract(r.target, tr.Translation)
		dir.Y = 0
		if rl.Vector3Length(dir) > .25 {
			intent.Move = rl.Vector3Normalize(dir)
		}
	})
}

// residentsSet is the Update set that keeps the residents, before
// characters take input. Order what adds or sends away residents before it.
const residentsSet illusion.SystemSet = "game.residents"

// residentsPlugin keeps the world's people: characters near the player,
// data elsewhere.
type residentsPlugin struct{}

func (residentsPlugin) Build(app *illusion.App) {
	app.InsertResource(illusion.R(&residents{}))
	app.ConfigureSets(illusion.Update, residentsSet.Before(character.Input))
	app.AddSystems(illusion.Update,
		illusion.Chain(illusion.Fn7(embodyResidents), illusion.Fn3(moveResidents), illusion.Fn5(steerResidents)).InSet(residentsSet),
	)
}
