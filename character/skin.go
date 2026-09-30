package character

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// changeSkin puts the player in the roster's next skin when Tab is pressed.
func changeSkin(
	keys *illusion.Res[input.Keys],
	bodies *illusion.Query4Where[State, render.Model3d, render.AnimationPlayer, transform.Transform, illusion.With[Body]],
	players *illusion.Query0Where[illusion.With[Player]],
	hier *illusion.Hierarchy,
	roster *illusion.Res[Roster],
) {
	if !keys.Get().JustPressed(rl.KeyTab) {
		return
	}
	r := roster.Get()
	if len(r.Skins) < 2 {
		return
	}
	bodies.Each(func(e ecs.Entity, st *State, m *render.Model3d, p *render.AnimationPlayer, tr *transform.Transform) {
		if parent, ok := hier.Parent(e); ok && players.Contains(parent) {
			r.Wear(st, m, p, tr, (st.skin+1)%len(r.Skins))
		}
	})
}

// Wear dresses a body in skin, starting it idle.
func (r *Roster) Wear(st *State, m *render.Model3d, p *render.AnimationPlayer, tr *transform.Transform, skin int) {
	s := &r.Skins[skin]
	st.skin, st.Current = skin, Idle
	m.Model = s.Model
	*p = render.AnimationPlayer{Animations: s.Anims}
	p.Play(s.clip(Idle).Name)
	tr.Scale = rl.Vector3{X: s.Scale, Y: s.Scale, Z: s.Scale}
}
