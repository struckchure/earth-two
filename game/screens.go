package game

import (
	"fmt"
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
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
	{[]string{"Mouse"}, "Look around"},
	{[]string{"M / J"}, "Map / debt and contract journal"},
	{[]string{"Shift"}, "Run"},
	{[]string{"Space"}, "Jump / vault / mantle / wall kick"},
	{[]string{"Ctrl"}, "Slide while running"},
	{[]string{"C"}, "Crouch (hold)"},
	{[]string{"R"}, "Roll"},
	{[]string{"W / S"}, "Climb ladder (move toward to attach)"},
	{[]string{"E"}, "Interact"},
	{[]string{"H"}, "Vehicle headlamps on / off"},
	{[]string{"F"}, "Punch"},
	{[]string{"Q"}, "Pick up"},
	{[]string{"T / G"}, "Talk / dance"},
	{[]string{"Esc"}, "Menu"},
}

// drawMenus draws the screen in front of the game.
func drawMenus(
	m *illusion.Res[menu],
	fonts *illusion.Res[uiFonts],
	win *illusion.Res[window.Window],
	players *illusion.Query1Where[character.Outfit, illusion.With[character.Player]],
	wardrobe *illusion.Res[character.Wardrobe],
	jobs *illusion.Res[contracts],
) {
	mu, ww := m.Get(), win.Get()
	s := mu.screen()
	if s == playing {
		return
	}
	width, height := float32(ww.Width), float32(ww.Height)
	p := newPainter(fonts.Get(), ww)
	l := layoutFor(s, width, height, p.s)
	focus := mu.top().focus

	switch s {
	case title:
		shade(min(width, l.panel.X+l.panel.Width+p.px(260)), height, 230)
		p.text("Earth Two", l.heading, 66, black, colText)
		menuButtons(p, l, s, focus)
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
	case contractOffer:
		if _, ok := jobs.TryGet(); ok {
			rl.DrawRectangle(0, 0, int32(width), int32(height), rl.NewColor(0, 0, 0, 120))
			drawContractOffer(p, l, focus)
		}
	case contractJournal:
		if c, ok := jobs.TryGet(); ok {
			p.s = l.sc
			rl.DrawRectangle(0, 0, int32(width), int32(height), rl.NewColor(0, 0, 0, 140))
			drawContractJournal(p, l, c, focus, mu.top().receipt)
		}
	}
}

func menuButtons(p painter, l layout, s screen, focus int) {
	for i, it := range items(s) {
		p.button(l.buttons[i], it.label, i == focus)
	}
}

// hud draws contextual actions and traversal hints, and driving, the speed.
// Navigation bindings are listed in the Controls screen.
func hud(win *illusion.Res[window.Window], fonts *illusion.Res[uiFonts], m *illusion.Res[menu], players *illusion.Query1Where[character.Traversal, illusion.With[character.Player]],
	prompt *illusion.Res[vehicle.Prompt], driving *illusion.Res[vehicle.Driving]) {
	ww := win.Get()
	p := newPainter(fonts.Get(), ww)
	width, height := float32(ww.Width), float32(ww.Height)

	if m.Get().screen() != playing {
		return
	}
	players.Each(func(_ ecs.Entity, s *character.Traversal) {
		if s.Hint != "" {
			p.text(s.Hint, rl.Vector2{X: p.px(20), Y: height - p.px(80)}, 15, semibold, colOnLight)
		}
	})
	drawDriving(p, prompt.Get(), driving.Get(), width, height)
	if driving.Get().Active() {
		lamps := "Headlamps off"
		if driving.Get().Headlamps {
			lamps = "Headlamps on"
		}
		x, y := p.px(130), height-p.px(38)
		x += p.keycap("H", rl.Vector2{X: x, Y: y}, 13) + p.px(8)
		p.text(lamps, rl.Vector2{X: x, Y: y + p.px(4)}, 13, semibold, colText)
	}
}

func drawFrameRate(p painter, height float32) rl.Rectangle {
	fps := fmt.Sprintf("%d FPS", rl.GetFPS())
	r := frameRateRect(p, height)
	rl.DrawRectangleRec(r, rl.NewColor(16, 13, 11, 150))
	p.textIn(fps, r, 13, semibold, colMuted, centre)
	return r
}

func frameRateRect(p painter, height float32) rl.Rectangle {
	width := p.measure(fmt.Sprintf("%d FPS", rl.GetFPS()), 13, semibold).X
	return rl.Rectangle{X: p.px(20), Y: height - p.px(38), Width: width + p.px(18), Height: p.px(24)}
}

// Draw last so a menu reaching the lower edge cannot cover the counter.
func frameRate(win *illusion.Res[window.Window], fonts *illusion.Res[uiFonts]) {
	w := win.Get()
	drawFrameRate(newPainter(fonts.Get(), w), float32(w.Height))
}

// Camera shots, relative to the player's capsule centre: where the camera
// sits, and what it looks at.
var (
	front = shot{offset: rl.Vector3{Y: 0.25, Z: 3.1}, look: rl.Vector3{Y: 0.05}}
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
	players *illusion.Query2Where[transform.Transform, character.MotionSamples, illusion.With[character.Player]],
	cameras *illusion.Query2[transform.Transform, render.Camera3d],
	m *illusion.Res[menu],
	clk *clock,
	win *illusion.Res[window.Window],
	aim *illusion.Local[aiming],
	p *physics.Physics,
	o *illusion.Res[orbit],
) {
	t, fixed := &clk.time, &clk.fixed
	e, player, samples, ok := players.Single()
	if !ok {
		return
	}
	mu, ww := m.Get(), win.Get()
	s := mu.screen()
	sh := o.Get().shot()
	lag := float32(playLag)
	switch {
	case s == dressing:
		sh, lag = front, cameraLag
	case mu.onTitle():
		sh, lag = orbiting(mu.orbit), cameraLag
	}
	x := float32(0)
	if s != playing && s != contractOffer && s != contractJournal && ww.Width > 0 {
		l := layoutFor(s, float32(ww.Width), float32(ww.Height), uiScale(ww))
		x = min((l.panel.X+l.panel.Width)/float32(ww.Width), 0.6)
	}
	k := float32(1 - math.Exp(float64(-lag*t.Get().DeltaSecs())))
	position := samples.Position(player.Translation, fixed.Get().Overstep())
	cameras.Each(func(_ ecs.Entity, tr *transform.Transform, cam *render.Camera3d) {
		fovy := cam.Fovy
		if fovy == 0 {
			fovy = 45
		}
		aspect := float32(ww.Width) / max(float32(ww.Height), 1)
		eye, target := frame(rl.Vector3Add(position, sh.offset), rl.Vector3Add(position, sh.look), x, fovy, aspect)
		// Ease what it looks at too, so it turns smoothly between shots.
		a := aim.Get()
		if !a.set {
			a.at, a.set = target, true
		}
		a.at = rl.Vector3Lerp(a.at, target, k)
		look := a.at
		tr.Translation = rl.Vector3Lerp(tr.Translation, eye, k)
		if s == playing || s == mapping || s == contractOffer || s == contractJournal {
			// Pulled in short of a wall in the way; easing back out, once
			// it's clear, as it eases anywhere.
			look = clearAbove(p, position, a.at, o.Get().avoid(e))
			tr.Translation = springArm(p, look, tr.Translation, o.Get().avoid(e))
			// Never skimming the ground: on a slope, a dune between it and
			// the player would hide their legs.
			if floor := groundHeight(tr.Translation.X, tr.Translation.Z) + groundLevel + cameraClearance; tr.Translation.Y < floor {
				tr.Translation.Y = floor
			}
		}
		tr.LookAt(look, transform.Up)
	})
}
