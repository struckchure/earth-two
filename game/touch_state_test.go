package game

import (
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
)

func TestTouchMovementUsesPlayerTraversalConfig(t *testing.T) {
	app := illusion.New()
	defer app.Cleanup()
	// Deliberately no TraversalConfig resource: the real game stores it only
	// on the character. Exercise the first moving frame as well as full sprint.
	app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		cfg := character.DefaultTraversal()
		cfg.SlideMin = 4
		cmd.Spawn(illusion.C(character.Player{}), illusion.C(character.Traversal{}), illusion.C(character.Intent{}), illusion.C(physics.CharacterController{Grounded: true}), illusion.C(cfg))
	}))
	var running, sliding bool
	app.AddSystems(illusion.Update, illusion.Fn1(func(players *touchPlayers) { running, sliding = touchMovement(players) }))
	app.Tick(time.Second / 60)
	q := ecs.NewFilter2[character.Intent, physics.CharacterController](app.World).Query()
	q.Next()
	in, cc := q.Get()
	q.Close()
	for _, tt := range []struct {
		name                                      string
		speed                                     float32
		run, grounded, moving, wantRun, wantSlide bool
	}{
		{"idle", 0, false, true, false, false, false},
		{"first joystick movement", .3, true, true, true, true, false},
		{"walking", 1.6, false, true, true, false, false},
		{"below player's slide threshold", 3.5, true, true, true, true, false},
		{"sprinting", 4.6, true, true, true, true, true},
		{"airborne", 4.6, true, false, true, false, false},
		{"released joystick", 4.6, false, true, false, false, false},
	} {
		t.Run(tt.name, func(t *testing.T) {
			in.Run = tt.run
			in.Move = rl.Vector3{}
			if tt.moving {
				in.Move.Z = -1
			}
			cc.Grounded, cc.Velocity = tt.grounded, rl.Vector3{Z: -tt.speed}
			app.Tick(time.Second / 60)
			if running != tt.wantRun || sliding != tt.wantSlide {
				t.Fatalf("running/sliding = %v/%v, want %v/%v", running, sliding, tt.wantRun, tt.wantSlide)
			}
		})
	}
}
