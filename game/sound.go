package game

import (
	"fmt"
	"math"
	"math/rand/v2"
	"os"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/asset"
	"github.com/struckchure/illusion/audio"
	"github.com/struckchure/illusion/render"
)

// What Earth Two sounds like (docs/look-and-feel.md): the Hull's air
// handlers, wind and grit outside, the market's chatter; feet on grating,
// sand and paving; engines; the Exchange's bell; paperwork for the menus.
// The sounds are in assets/sounds, fetched and cut by tools/sounds (where
// each comes from is in its CREDITS.txt).
//
// Nothing sends events for any of it: the cue systems (cues.go,
// ambience.go, drivesound.go) watch what the game already keeps, how a
// body's animation, a vehicle or the menus are, and play what changed. So
// does effects.go, for the dust and the sparks.

// soundBank is a resource: every sound, by name, each with its variants,
// and the loops that are streamed.
type soundBank struct {
	hits  map[string][]asset.Handle[audio.Sound]
	loops map[string]asset.Handle[audio.Music]
}

// The loops, streamed: ambience beds, machines, engines and tyres.
var loopNames = []string{
	"wind", "storm", "hull_hum", "dome_air", "market", "fountain", "generator", "fans", "tyres",
	"engine_bike", "engine_trike", "engine_buggy", "engine_rover", "engine_truck",
}

// The one-shots, each with variants name_0, name_1, ... (or just name).
var hitNames = []string{
	"step_sand", "step_soil", "step_paving", "step_rug", "step_grating", "land_loose", "land_hard",
	"slide", "whoosh", "cloth", "thump", "grab", "creak", "clank", "bell",
	"door", "crash_metal", "crash_ground", "skid",
	"ui_move", "ui_stamp", "ui_back", "ui_open", "ui_page", "ui_mark", "ui_arrive", "ui_deny",
}

// loadSounds fills the bank from assets/sounds. A sound that's missing, or
// can't load (no audio device), is left out, and playing it does nothing.
func loadSounds(hits *asset.Loader[audio.Sound], tracks *asset.Loader[audio.Music]) *soundBank {
	b := &soundBank{hits: map[string][]asset.Handle[audio.Sound]{}, loops: map[string]asset.Handle[audio.Music]{}}
	for _, name := range loopNames {
		for _, ext := range []string{".wav", ".ogg"} {
			if h, err := tracks.Load("sounds/" + name + ext); err == nil {
				b.loops[name] = h
				break
			}
		}
	}
	load := func(path string) bool {
		for _, ext := range []string{".ogg", ".wav"} {
			if h, err := hits.Load(path + ext); err == nil {
				name := path[len("sounds/"):]
				if i := lastUnderscoreDigit(name); i > 0 {
					name = name[:i]
				}
				b.hits[name] = append(b.hits[name], h)
				return true
			}
		}
		return false
	}
	for _, name := range hitNames {
		if load("sounds/" + name) {
			continue
		}
		for i := 0; load(fmt.Sprintf("sounds/%s_%d", name, i)); i++ {
		}
	}
	return b
}

// lastUnderscoreDigit is where a variant's "_N" starts in name, or -1.
func lastUnderscoreDigit(name string) int {
	i := len(name) - 1
	for i >= 0 && name[i] >= '0' && name[i] <= '9' {
		i--
	}
	if i < 0 || i == len(name)-1 || name[i] != '_' {
		return -1
	}
	return i
}

// listener is where the sounds are heard from: the camera.
type listener struct {
	at, right rl.Vector3
	set       bool
}

func listenerOf(v *render.View3D) listener {
	c := v.Camera
	ahead := rl.Vector3Normalize(rl.Vector3Subtract(c.Target, c.Position))
	right := rl.Vector3Normalize(rl.Vector3CrossProduct(ahead, c.Up))
	return listener{at: c.Position, right: right, set: v.Active}
}

// Sounds in the world fall off with distance: full within near, gone by
// far, as the square of how far between (which reads as natural without
// being a cliff), and pan to the side they're on, never fully: both ears
// hear everything.
const maxPan = .7

// spatial is how loud (0 to 1) and where (pan) a sound at at is heard, near
// and far apart.
func (l listener) spatial(at rl.Vector3, near, far float32) (volume, pan float32) {
	if !l.set {
		return 1, 0
	}
	to := rl.Vector3Subtract(at, l.at)
	d := rl.Vector3Length(to)
	if d >= far {
		return 0, 0
	}
	k := 1 - smoothstep(near, far, d)
	volume = k * k
	if d > .01 {
		// From -1, the left, to 1, the right; centred close up, where
		// which side is a matter of inches.
		pan = maxPan * rl.Vector3DotProduct(rl.Vector3Scale(to, 1/d), l.right) * smoothstep(0, near, d)
	}
	return volume, pan
}

// logCues prints each cue as it plays, and each loop as it starts and
// stops, when EARTH_TWO_CUES is set: for hearing what the game thinks it's
// playing.
var logCues = os.Getenv("EARTH_TWO_CUES") != ""

// The mix: how loud each kind of sound is, before the master volume.
const (
	mixAmbience = .55
	mixEffects  = .8
	mixUI       = .5
	// While a menu's up the world's ambience is turned down to this share,
	// and its effects stop.
	menuDuck = .4
)

// voice plays the bank's one-shots: a random variant, its pitch nudged so
// repeats don't sound the same.
type voice struct {
	bank *soundBank
	au   *audio.Audio
	ear  listener
	duck float32 // the world's share of its volume, 1 or menuDuck
}

// play plays name at volume (0 to 1) for the mix, panned by pan, its pitch
// times pitch (0 as 1) and jittered by jitter (±share).
func (v voice) play(name string, volume, pan, pitch, jitter float32) {
	hs := v.bank.hits[name]
	if len(hs) == 0 || volume <= .005 {
		return
	}
	if pitch == 0 {
		pitch = 1
	}
	pitch *= 1 + jitter*(2*rand.Float32()-1)
	if logCues {
		fmt.Fprintf(os.Stderr, "cue %s volume %.2f pan %.2f pitch %.2f\n", name, volume, pan, pitch)
	}
	v.au.PlayWith(hs[rand.IntN(len(hs))], audio.Playback{Volume: min(1, volume), Pitch: pitch, Pan: pan})
}

// at plays name as heard at a place in the world, near and far apart.
func (v voice) at(name string, where rl.Vector3, volume, near, far, pitch float32) {
	vol, pan := v.ear.spatial(where, near, far)
	v.play(name, volume*vol*mixEffects*v.duck, pan, pitch, .06)
}

// ui plays a menu's sound.
func (v voice) ui(name string, volume float32) { v.play(name, volume*mixUI, 0, 1, .03) }

// loopState is a streamed loop the mix eases: playing or not, and how loud
// and pitched it is now.
type loopState struct {
	playing      bool
	volume, gain float32
	pitch, pan   float32
}

// loops eases the bank's streamed loops toward what's asked of them each
// frame, starting them when they're first heard and stopping them once
// they've faded out.
type loops struct {
	state map[string]*loopState
}

// loopEase is how quickly a loop's volume follows: the share 1 - e^-k a
// second.
const loopEase = 3

// set eases loop name toward volume, pitch and pan over dt.
func (l *loops) set(b *soundBank, au *audio.Audio, name string, volume, pitch, pan, dt float32) {
	h, ok := b.loops[name]
	if !ok {
		return
	}
	if l.state == nil {
		l.state = map[string]*loopState{}
	}
	s := l.state[name]
	if s == nil {
		s = &loopState{pitch: 1}
		l.state[name] = s
	}
	k := float32(1 - math.Exp(-loopEase*float64(dt)))
	s.volume += (volume - s.volume) * k
	s.pitch += (orOne(pitch) - s.pitch) * min(1, 4*k)
	s.pan += (pan - s.pan) * k
	switch {
	case !s.playing && volume > .01:
		au.PlayMusic(h, true)
		s.playing = true
		if logCues {
			fmt.Fprintf(os.Stderr, "loop %s on\n", name)
		}
	case s.playing && volume <= .01 && s.volume < .005:
		if logCues {
			fmt.Fprintf(os.Stderr, "loop %s off\n", name)
		}
		au.StopMusic(h)
		s.playing = false
		s.volume = 0
		return
	}
	if s.playing {
		au.SetMusicVolume(h, min(1, s.volume))
		au.SetMusicPitch(h, s.pitch)
		au.SetMusicPan(h, s.pan)
	}
}

func orOne(v float32) float32 {
	if v == 0 {
		return 1
	}
	return v
}

// cuesPlugin is the sound cues and the effects: ambience, feet, bodies,
// vehicles, menus, and the dust and sparks that go with them.
type cuesPlugin struct{}

// effectsSet draws the particles over the world, still in 3D, once its
// meshes are drawn.
const effectsSet illusion.SystemSet = "game.effects"

func (cuesPlugin) Build(app *illusion.App) {
	app.InsertResource(illusion.R(&soundBank{}), illusion.R(&soundscape{}), illusion.R(newEffects()))
	app.ConfigureSets(illusion.Render, effectsSet.After(render.Draw3D).Before(render.End3D))
	app.AddSystems(illusion.Startup, illusion.Fn4(startSounds))
	app.AddSystems(illusion.Update,
		illusion.Chain(
			illusion.Fn4(ambience),
			illusion.Fn5(bodyCues),
			illusion.Fn6(footCues),
			illusion.Fn5(driveCues),
			illusion.Fn5(uiCues),
			illusion.Fn3(moveEffects),
		).After(character.Act),
	)
	app.AddSystems(illusion.Render, illusion.Fn2(drawEffects).InSet(effectsSet))
}

// startSounds loads the sounds and finds what in the world makes them.
func startSounds(bank *illusion.Res[soundBank], scape *illusion.Res[soundscape], kit *illusion.Res[world.Kit], loaders *soundLoaders) {
	*bank.Get() = *loadSounds(&loaders.hits, &loaders.tracks)
	if logCues {
		fmt.Fprintf(os.Stderr, "sounds: %d one-shots, %d loops\n", len(bank.Get().hits), len(bank.Get().loops))
	}
	placed, err := world.Layout(assetRoot(), "world/landfall.json")
	if err != nil {
		return
	}
	*scape.Get() = *newSoundscape(kit.Get(), placed)
}

// soundLoaders is the two loaders the bank loads with, as one parameter.
type soundLoaders struct {
	hits   asset.Loader[audio.Sound]
	tracks asset.Loader[audio.Music]
}

func (l *soundLoaders) InitParam(w *ecs.World) {
	l.hits.InitParam(w)
	l.tracks.InitParam(w)
}
