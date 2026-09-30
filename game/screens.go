package game

import (
	"fmt"
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

// bindings are the controls, as the Controls screen lists them.
var bindings = []struct {
	keys []string
	does string
}{
	{[]string{"WASD", "Arrows"}, "Walk"},
	{[]string{"Shift"}, "Run"},
	{[]string{"Space"}, "Jump"},
	{[]string{"E"}, "Interact"},
	{[]string{"F"}, "Punch"},
	{[]string{"Q"}, "Pick up"},
	{[]string{"C"}, "Wardrobe"},
	{[]string{"Esc"}, "Menu"},
}

// drawMenus draws the screen in front of the game.
func drawMenus(
	m *illusion.Res[menu],
	fonts *illusion.Res[uiFonts],
	win *illusion.Res[window.Window],
	players *illusion.Query1Where[character.Outfit, illusion.With[character.Player]],
	wardrobe *illusion.Res[character.Wardrobe],
) {
	mu, ww := m.Get(), win.Get()
	s := mu.screen()
	if s == playing {
		return
	}
	width, height := float32(ww.Width), float32(ww.Height)
	p := painter{fonts: fonts.Get(), s: uiScale(ww)}
	l := layoutFor(s, width, height, p.s)
	focus := mu.top().focus

	switch s {
	case title:
		shade(min(width, l.panel.X+l.panel.Width+p.px(260)), height, 230)
		p.text("Earth Two", l.heading, 66, black, colText)
		menuButtons(p, l, s, focus)
		x := l.heading.X
		y := height - p.px(56)
		x += p.keycap("↑", rl.Vector2{X: x, Y: y}, 14) + p.px(6)
		x += p.keycap("↓", rl.Vector2{X: x, Y: y}, 14) + p.px(10)
		p.text("Choose", rl.Vector2{X: x, Y: y + p.px(4)}, 15, regular, colMuted)
		x += p.measure("Choose", 15, regular).X + p.px(24)
		x += p.keycap("Enter", rl.Vector2{X: x, Y: y}, 14) + p.px(10)
		p.text("Select", rl.Vector2{X: x, Y: y + p.px(4)}, 15, regular, colMuted)
		// Fade in from black when the game starts.
		if mu.orbit < 1 {
			rl.DrawRectangle(0, 0, int32(width), int32(height), rl.NewColor(0, 0, 0, uint8(255*(1-mu.orbit))))
		}
	case paused:
		rl.DrawRectangle(0, 0, int32(width), int32(height), rl.NewColor(0, 0, 0, 90))
		p.panel(l.panel)
		p.text("Paused", l.heading, 30, black, colText)
		menuButtons(p, l, s, focus)
	case controlsHelp:
		rl.DrawRectangle(0, 0, int32(width), int32(height), rl.NewColor(0, 0, 0, 90))
		p.panel(l.panel)
		p.text("Controls", l.heading, 30, black, colText)
		for i, b := range bindings {
			r := l.rows[i]
			x := r.X
			for j, k := range b.keys {
				if j > 0 {
					p.textIn("or", rl.Rectangle{X: x, Y: r.Y, Width: p.px(24), Height: r.Height}, 14, regular, colMuted, centre)
					x += p.px(24)
				}
				x += p.keycap(k, rl.Vector2{X: x, Y: r.Y + (r.Height-p.px(14*1.7))/2}, 14) + p.px(4)
			}
			p.textIn(b.does, rl.Rectangle{X: r.X + p.px(190), Y: r.Y, Width: r.Width - p.px(190), Height: r.Height}, 18, regular, colText, left)
		}
		menuButtons(p, l, s, focus)
	case dressing:
		if _, o, ok := players.Single(); ok {
			drawWardrobe(p, l, focus, wardrobe.Get(), *o)
		}
	}
}

func menuButtons(p painter, l layout, s screen, focus int) {
	for i, it := range items(s) {
		p.button(l.buttons[i], it.label, i == focus)
	}
}

// hud draws the frame rate, and in play the keys for the menus.
func hud(win *illusion.Res[window.Window], fonts *illusion.Res[uiFonts], m *illusion.Res[menu]) {
	ww := win.Get()
	p := painter{fonts: fonts.Get(), s: uiScale(ww)}
	width, height := float32(ww.Width), float32(ww.Height)

	fps := fmt.Sprintf("%d fps", rl.GetFPS())
	fm := p.measure(fps, 13, semibold)
	pill := rl.Rectangle{X: width - fm.X - p.px(32), Y: p.px(14), Width: fm.X + p.px(18), Height: p.px(24)}
	rl.DrawRectangleRec(pill, rl.NewColor(16, 13, 11, 150))
	p.textIn(fps, pill, 13, semibold, colMuted, centre)

	if m.Get().screen() != playing {
		return
	}
	x, y := p.px(20), height-p.px(48)
	for _, h := range []struct{ key, does string }{{"Esc", "Menu"}, {"C", "Wardrobe"}} {
		x += p.keycap(h.key, rl.Vector2{X: x, Y: y}, 14) + p.px(8)
		p.text(h.does, rl.Vector2{X: x + 1, Y: y + p.px(4) + 1}, 15, semibold, rl.NewColor(0, 0, 0, 110))
		p.text(h.does, rl.Vector2{X: x, Y: y + p.px(4)}, 15, semibold, colText)
		x += p.measure(h.does, 15, semibold).X + p.px(22)
	}
}

// Camera shots, relative to the player's capsule centre: where the camera
// sits, and what it looks at.
var (
	behind = shot{offset: cameraOffset, look: rl.Vector3{Y: 1}}
	front  = shot{offset: rl.Vector3{Y: 0.25, Z: 3.1}, look: rl.Vector3{Y: 0.05}}
)

type shot struct{ offset, look rl.Vector3 }

// aiming is where the camera is looking, eased towards its shot's target.
type aiming struct {
	at  rl.Vector3
	set bool
}

// orbiting is the title screen's shot: the camera swinging slowly to and
// fro in front of the player, t seconds in.
func orbiting(t float32) shot {
	a := 0.6 * math.Sin(float64(t)*0.2)
	return shot{
		offset: rl.Vector3{X: float32(3.4 * math.Sin(a)), Y: 0.5, Z: float32(3.4 * math.Cos(a))},
		look:   rl.Vector3{Y: 0.15},
	}
}

// frame moves a camera at eye looking at target sideways, so that target
// appears at x across the screen (-1 left, 0 centre, 1 right).
func frame(eye, target rl.Vector3, x, fovy, aspect float32) (rl.Vector3, rl.Vector3) {
	forward := rl.Vector3Subtract(target, eye)
	right := rl.Vector3Normalize(rl.Vector3CrossProduct(forward, transform.Up))
	halfWidth := rl.Vector3Length(forward) * float32(math.Tan(float64(fovy)*math.Pi/360)) * aspect
	shift := rl.Vector3Scale(right, -x*halfWidth)
	return rl.Vector3Add(eye, shift), rl.Vector3Add(target, shift)
}

// follow eases the camera to the shot for the screen in front: behind the
// player in play, and round to their front on the title screen and in the
// wardrobe, with the player to the right of the menu.
func follow(
	players *illusion.Query1Where[transform.Transform, illusion.With[character.Player]],
	cameras *illusion.Query2[transform.Transform, render.Camera3d],
	m *illusion.Res[menu],
	t *illusion.Res[illusion.Time],
	win *illusion.Res[window.Window],
	aim *illusion.Local[aiming],
) {
	_, player, ok := players.Single()
	if !ok {
		return
	}
	mu, ww := m.Get(), win.Get()
	s := mu.screen()
	sh := behind
	switch {
	case s == dressing:
		sh = front
	case mu.onTitle():
		sh = orbiting(mu.orbit)
	}
	x := float32(0)
	if s != playing && ww.Width > 0 {
		l := layoutFor(s, float32(ww.Width), float32(ww.Height), uiScale(ww))
		x = min((l.panel.X+l.panel.Width)/float32(ww.Width), 0.6)
	}
	k := min(1, cameraLag*t.Get().DeltaSecs())
	cameras.Each(func(_ ecs.Entity, tr *transform.Transform, cam *render.Camera3d) {
		fovy := cam.Fovy
		if fovy == 0 {
			fovy = 45
		}
		aspect := float32(ww.Width) / max(float32(ww.Height), 1)
		eye, target := frame(rl.Vector3Add(player.Translation, sh.offset), rl.Vector3Add(player.Translation, sh.look), x, fovy, aspect)
		// Ease what it looks at too, so it turns smoothly between shots.
		a := aim.Get()
		if !a.set {
			a.at, a.set = target, true
		}
		a.at = rl.Vector3Lerp(a.at, target, k)
		tr.Translation = rl.Vector3Lerp(tr.Translation, eye, k)
		tr.LookAt(a.at, transform.Up)
	})
}
