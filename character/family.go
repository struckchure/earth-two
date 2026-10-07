package character

import (
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
)

// Family is a resource: each entity's children, as they were at the start
// of the frame. Systems that look up a character's body, or what it wears,
// every frame for every character, look here rather than walking
// illusion.Hierarchy: finding one parent's children there is a query over
// every parent's table, so with a crowd of N it's N of those for each of N
// characters. This is one pass over all of them a frame.
//
// It doesn't see what's spawned during the frame until the next; what's
// despawned during it, it leaves out as it hands it over.
type Family struct {
	world *ecs.World
	kids  map[ecs.Entity][]ecs.Entity
}

// EachChild calls fn for each of parent's children still alive.
func (f *Family) EachChild(parent ecs.Entity, fn func(child ecs.Entity)) {
	for _, c := range f.kids[parent] {
		if f.world == nil || f.world.Alive(c) {
			fn(c)
		}
	}
}

// EachDescendant calls fn for every descendant of parent still alive,
// parents before their children.
func (f *Family) EachDescendant(parent ecs.Entity, fn func(e ecs.Entity)) {
	f.EachChild(parent, func(c ecs.Entity) {
		fn(c)
		f.EachDescendant(c, fn)
	})
}

// familyIndex is indexFamily's memory: the filter over every child, and
// the map to their parents.
type familyIndex struct {
	children *ecs.Filter0
	parents  *ecs.Map[illusion.ChildOf]
}

// indexFamily builds the Family afresh: every child, under its parent.
func indexFamily(w *illusion.World, fam *illusion.Res[Family], state *illusion.Local[familyIndex]) {
	s, f := state.Get(), fam.Get()
	if s.children == nil {
		s.children = ecs.NewFilter0(w.World).With(ecs.C[illusion.ChildOf]())
		s.parents = ecs.NewMap[illusion.ChildOf](w.World)
	}
	f.world = w.World
	if f.kids == nil {
		f.kids = map[ecs.Entity][]ecs.Entity{}
	}
	for k, v := range f.kids {
		f.kids[k] = v[:0]
	}
	q := s.children.Query()
	for q.Next() {
		e := q.Entity()
		p := s.parents.GetRelation(e)
		f.kids[p] = append(f.kids[p], e)
	}
	for k, v := range f.kids {
		if len(v) == 0 {
			delete(f.kids, k)
		}
	}
}
