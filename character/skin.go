package character

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// Wear dresses a body in skin, starting it idle.
func (r *Roster) Wear(st *State, m *render.Model3d, p *render.AnimationPlayer, tr *transform.Transform, skin int) {
	s := &r.Skins[skin]
	st.skin, st.Current = skin, Idle
	m.Model = s.Model
	*p = render.AnimationPlayer{Animations: s.Anims}
	p.Play(s.clip(Idle).Name)
	tr.Scale = rl.Vector3{X: s.Scale, Y: s.Scale, Z: s.Scale}
}
