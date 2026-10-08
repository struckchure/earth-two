//go:build !js

package game

import (
	"encoding/json"
	"fmt"
	"io"
	"math"
	"os"
	"os/exec"
	"path/filepath"
	"time"
	"unsafe"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

// TrailerShot extends a tour view with a camera move and a duration.
// Cameras stay near the player because world streaming follows the player.
type TrailerShot struct {
	View
	Seconds   float64     `json:"seconds"`
	EyeEnd    *[3]float32 `json:"eyeEnd,omitempty"`
	TargetEnd *[3]float32 `json:"targetEnd,omitempty"`
	YawEnd    *float32    `json:"yawEnd,omitempty"`
	Hour      *float32    `json:"hour,omitempty"`
	Paper     bool        `json:"paper,omitempty"`
}

// Trailer captures clean world footage, without the HUD, into one MP4 per
// shot. Paper shots also include the specified menu. FFmpeg must be on PATH.
func Trailer(shotsFile, out string, fps int) error {
	data, err := os.ReadFile(shotsFile)
	if err != nil {
		return err
	}
	var shots []TrailerShot
	if err := json.Unmarshal(data, &shots); err != nil {
		return err
	}
	if len(shots) == 0 || fps < 1 || fps > 120 {
		return fmt.Errorf("provide shots and FPS between 1 and 120")
	}
	for _, s := range shots {
		if s.Seconds <= 0 || s.Name == "" || filepath.Base(s.Name) != s.Name {
			return fmt.Errorf("invalid shot %q", s.Name)
		}
	}
	if _, err := exec.LookPath("ffmpeg"); err != nil {
		return err
	}
	if err := os.MkdirAll(out, 0755); err != nil {
		return err
	}
	const warmup = 150
	at, frame := 0, 0
	var encoder *exec.Cmd
	var pipe io.WriteCloser
	var captureErr error
	finish := func() {
		if pipe != nil {
			if err := pipe.Close(); captureErr == nil {
				captureErr = err
			}
			if err := encoder.Wait(); captureErr == nil {
				captureErr = err
			}
			pipe, encoder = nil, nil
		}
	}
	defer finish()
	app := buildConfigured(&menu{}, true)
	app.AddSystems(illusion.Update, illusion.Fn1(func(controls *illusion.Res[character.Controls]) {
		controls.Get().Enabled = false
	}).Before(character.Input))
	// Set the cinematic camera before streaming, visibility and sky placement.
	// Updating it in PostUpdate makes culling use the gameplay camera instead.
	app.AddSystems(illusion.Update, illusion.Fn8(func(
		mu *illusion.Res[menu],
		players *illusion.Query2Where[transform.Transform, physics.CharacterController, illusion.With[character.Player]],
		cameras *illusion.Query2[transform.Transform, render.Camera3d],
		o *illusion.Res[orbit],
		outfits *illusion.Query1Where[character.Outfit, illusion.With[character.Player]],
		wardrobe *illusion.Res[character.Wardrobe],
		day *illusion.Res[daylight],
		intents *illusion.Query1Where[character.Intent, illusion.With[character.Player]],
	) {
		if at >= len(shots) {
			return
		}
		s := shots[at]
		m := mu.Get()
		m.orbit, m.stack = 1, nil
		if sc, ok := tourScreens[s.Screen]; ok {
			m.stack = []page{{screen: sc}}
		}
		p := rl.Vector3{X: s.Player[0], Y: s.Player[1], Z: s.Player[2]}
		if s.Ground {
			p.Y = groundHeight(p.X, p.Z) + groundLevel + 1
		}
		if frame < 40 {
			players.Each(func(_ ecs.Entity, tr *transform.Transform, cc *physics.CharacterController) {
				tr.Translation, cc.Velocity = p, rl.Vector3{}
			})
		}
		if frame == 0 {
			if s.Hour != nil {
				day.Get().hour = *s.Hour
			}
			outfits.Each(func(_ ecs.Entity, outfit *character.Outfit) {
				w := wardrobe.Get()
				for i, look := range w.Bodies[outfit.Body].Looks {
					if look.Name == s.Look {
						*outfit = w.Wear(*outfit, i)
					}
				}
			})
		}
		if frame == 90 {
			intents.Each(func(_ ecs.Entity, in *character.Intent) {
				if hold, ok := tourHolds[s.Hold]; ok {
					in.Hold = hold
				}
			})
		}
		t := float32(math.Max(0, math.Min(1, float64(frame-warmup)/math.Max(1, math.Round(s.Seconds*float64(fps))-1))))
		// A gentle ease keeps the starts and ends of camera moves deliberate.
		t = t * t * (3 - 2*t)
		orb := o.Get()
		orb.yaw, orb.pitch, orb.still = s.Yaw, s.Pitch, 0
		if s.YawEnd != nil {
			orb.yaw += (*s.YawEnd - s.Yaw) * t
		}
		if s.Eye != nil && s.Target != nil {
			lerp := func(a [3]float32, b *[3]float32) rl.Vector3 {
				if b != nil {
					for i := range a {
						a[i] += ((*b)[i] - a[i]) * t
					}
				}
				return rl.Vector3{X: a[0], Y: a[1], Z: a[2]}
			}
			cameras.Each(func(_ ecs.Entity, tr *transform.Transform, _ *render.Camera3d) {
				tr.Translation = lerp(*s.Eye, s.EyeEnd)
				tr.LookAt(lerp(*s.Target, s.TargetEnd), transform.Up)
			})
		}
	}).After(cameraFollowSet).Before(worldStreamSet))
	// Read before HUD drawing for cinematic shots; paper shots read afterwards.
	capture := func(paper bool, exit *illusion.Res[illusion.AppExit]) {
		if at >= len(shots) || captureErr != nil {
			exit.Get().Requested = true
			return
		}
		s := shots[at]
		if s.Paper != paper {
			return
		}
		if frame < warmup {
			frame++
			return
		}
		rl.DrawRenderBatchActive()
		img := rl.LoadImageFromScreen()
		defer rl.UnloadImage(img)
		rl.ImageFormat(img, rl.UncompressedR8g8b8a8)
		if encoder == nil {
			file := filepath.Join(out, s.Name+".mp4")
			encoder = exec.Command("ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pixel_format", "rgba", "-video_size", fmt.Sprintf("%dx%d", img.Width, img.Height), "-framerate", fmt.Sprint(fps), "-i", "pipe:0", "-an", "-vf", "scale=1920:1080:flags=lanczos", "-c:v", "libx264", "-preset", "fast", "-crf", "17", "-pix_fmt", "yuv420p", "-movflags", "+faststart", file)
			encoder.Stderr = os.Stderr
			pipe, err = encoder.StdinPipe()
			if err == nil {
				err = encoder.Start()
			}
			if err != nil {
				captureErr = err
				exit.Get().Requested = true
				return
			}
			fmt.Printf("trailer: %s (%.1fs)\n", s.Name, s.Seconds)
		}
		pixels := unsafe.Slice((*byte)(img.Data), int(img.Width)*int(img.Height)*4)
		if _, err := pipe.Write(pixels); err != nil {
			captureErr = err
			exit.Get().Requested = true
			return
		}
		if frame++; frame >= warmup+int(math.Round(s.Seconds*float64(fps))) {
			finish()
			at, frame = at+1, 0
			if at >= len(shots) {
				exit.Get().Requested = true
			}
		}
	}
	const cleanSet illusion.SystemSet = "game.trailer.clean"
	const paperSet illusion.SystemSet = "game.trailer.paper"
	app.ConfigureSets(illusion.Render, cleanSet.After(render.End2D).Before(render.Draw2D), paperSet.After(stampSet).Before(render.End))
	app.AddSystems(illusion.Render,
		illusion.Fn1(func(e *illusion.Res[illusion.AppExit]) { capture(false, e) }).InSet(cleanSet),
		illusion.Fn1(func(e *illusion.Res[illusion.AppExit]) { capture(true, e) }).InSet(paperSet),
	)
	// Advance exactly one output frame of simulation, even if encoding stalls.
	// The window plugin normally uses wall time, which jitters offline footage.
	app.SetRunner(func(a *illusion.App) {
		rl.SetConfigFlags(rl.FlagMsaa4xHint)
		rl.SetTraceLogLevel(rl.LogWarning)
		rl.InitWindow(1920, 1080, "Earth Two — trailer capture")
		defer rl.CloseWindow()
		defer a.Cleanup()
		rl.SetExitKey(rl.KeyNull)
		a.Startup()
		for !a.ShouldExit() {
			a.Tick(time.Second / time.Duration(fps))
		}
	})
	app.Run()
	finish()
	if captureErr != nil {
		return captureErr
	}
	if at != len(shots) {
		return fmt.Errorf("capture stopped after %d of %d shots", at, len(shots))
	}
	return nil
}
