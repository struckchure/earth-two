package game

import (
	"math"
	"strconv"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

const (
	worldPopulation = 4200
	maxTestNPCs     = worldPopulation - 1 // the local player also occupies a place
	// npcBatch is how many residents are placed a frame.
	npcBatch = 256
)

// testNPCs belongs to the test plugin: a local crowd of residents, for
// trying character rendering and physics, not the persistent world's.
// Moving to another area rebuilds the crowd there, inside streamed
// terrain. live is how many have been placed.
type testNPCs struct {
	enabled          bool
	population, live int
	editing          bool
	digits           string
	anchor           rl.Vector3
	site             int
}

func npcSwitch() testSwitch {
	return testSwitch{
		Name: "NPCs", Key: rl.KeyF8, Settings: []string{"Off", "On"},
		Get: func(w *ecs.World) int {
			if n := ecs.GetResource[testNPCs](w); n != nil && n.enabled {
				return 1
			}
			return 0
		},
		Set: func(w *ecs.World, i int) {
			if n := ecs.GetResource[testNPCs](w); n != nil {
				n.enabled = i == 1
			}
		},
	}
}

// F9 selects the current count for replacement. Consume the edit's keys
// before character/menu input, so Enter doesn't jump and Esc doesn't pause.
func editNPCPopulation(k *input.Keys, n *testNPCs, panel *testSwitches) bool {
	if k.JustPressed(rl.KeyF9) {
		n.editing, n.digits, panel.shown = true, "", true
		k.Clear()
		return true
	}
	if !n.editing {
		return false
	}
	switch {
	case k.JustPressed(testPanelKey):
		n.editing, panel.shown = false, false
	case k.JustPressed(rl.KeyEscape):
		n.editing = false
	case k.AnyJustPressed(rl.KeyEnter, rl.KeyKpEnter):
		if n.digits != "" {
			count, _ := strconv.Atoi(n.digits)
			n.population = min(maxTestNPCs, max(0, count))
		}
		n.editing = false
	default:
		if k.JustPressed(rl.KeyBackspace) && len(n.digits) > 0 {
			n.digits = n.digits[:len(n.digits)-1]
		}
		for digit := range 10 {
			if len(n.digits) < 5 && k.AnyJustPressed(input.Key(rl.KeyZero+digit), input.Key(rl.KeyKp0+digit)) {
				n.digits += strconv.Itoa(digit)
			}
		}
	}
	k.Clear()
	return true
}

func syncTestNPCs(
	state *illusion.Res[testNPCs],
	res *illusion.Res[residents],
	roster *illusion.Res[character.Roster],
	wardrobe *illusion.Res[character.Wardrobe],
	players *illusion.Query1Where[transform.Transform, illusion.With[character.Player]],
	p *physics.Physics,
	mu *illusion.Res[menu],
) {
	n, rs := state.Get(), res.Get()
	want := min(maxTestNPCs, max(0, n.population))
	if !n.enabled {
		want = 0
	}
	playerEntity, player, havePlayer := players.Single()
	if havePlayer && n.live > 0 && flatDistance(player.Translation, n.anchor) > 128 {
		want = 0 // rebuilt around the player below, next frame
	}
	if want == 0 {
		if n.live > 0 {
			rs.clear()
		}
		n.live, n.site = 0, 0
		return
	}
	if n.live > want {
		rs.truncate(want)
		n.live = want
	}
	if !havePlayer || mu.Get().screen() != playing {
		return
	}
	r, ready := roster.TryGet()
	w, dressed := wardrobe.TryGet()
	if !ready || !dressed || len(r.Skins) == 0 {
		return
	}
	if n.live == 0 {
		n.anchor, n.site = player.Translation, 0
	}
	for added, attempts := 0, 0; n.live < want && added < npcBatch && attempts < npcBatch*16; attempts++ {
		at := npcSite(n.anchor, n.site)
		n.site++
		// Keep the whole crowd within colliding terrain. If nearby ground
		// is full, the panel's live count shows how many could be placed.
		if flatDistance(at, n.anchor) > 120 {
			break
		}
		feet := rl.Vector3{X: at.X, Y: walkHeight(at.X, at.Z), Z: at.Z}
		// When testing an upper deck, try that level first.
		if hit, ok := p.CastRay(rl.Vector3{X: at.X, Y: player.Translation.Y + 1, Z: at.Z}, rl.Vector3{Y: -1}, 5); ok && hit.Normal.Y > .7 && hit.Point.Y > feet.Y {
			feet.Y = hit.Point.Y
		}
		center := rl.Vector3Add(feet, rl.Vector3{Y: .95})
		if p.OverlapCapsuleExcluding(center, .35, 1.8, playerEntity) {
			continue
		}
		body := n.live % len(r.Skins)
		rs.add(resident{feet: feet, skin: body, outfit: npcOutfit(w, body, n.live), home: feet, target: feet, left: float32(n.live % 7)})
		n.live++
		added++
	}
}

// A golden-angle spiral leaves room between people, and a clear space
// around the player. The same index has the same position on each toggle.
func npcSite(anchor rl.Vector3, site int) rl.Vector3 {
	a := float64(site) * 2.399963229728653
	r := 8 + 1.4*math.Sqrt(float64(site))
	return rl.Vector3{X: anchor.X + float32(r*math.Cos(a)), Y: anchor.Y, Z: anchor.Z + float32(r*math.Sin(a))}
}

func npcOutfit(w *character.Wardrobe, body, index int) character.Outfit {
	o := character.Outfit{Body: body}
	if body >= len(w.Bodies) {
		return o
	}
	bw := &w.Bodies[body]
	if len(bw.Tones) > 0 {
		o.Tone = index % len(bw.Tones)
	}
	for _, slot := range []character.Slot{character.Hair, character.Top, character.Bottom, character.Shoes} {
		if len(bw.Items[slot]) > 0 {
			o.Put(slot, index%len(bw.Items[slot]))
		}
	}
	return o
}

func flatDistance(a, b rl.Vector3) float32 {
	return float32(math.Hypot(float64(a.X-b.X), float64(a.Z-b.Z)))
}
