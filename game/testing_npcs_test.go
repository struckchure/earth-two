package game

import (
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

func TestNPCPopulationEditor(t *testing.T) {
	app := illusion.New()
	keys := input.NewButtonInput[input.Key]()
	n := &testNPCs{population: 25}
	app.InsertResource(illusion.R(keys), illusion.R(n))
	addTestSwitch(app, npcSwitch())
	app.AddSystems(illusion.Update, illusion.Fn3(flipSwitches))
	press := func(key input.Key) {
		keys.Press(key)
		app.Tick(time.Second / 60)
		keys.Release(key)
		keys.Clear()
	}
	press(rl.KeyF8)
	if !n.enabled {
		t.Fatal("F8 should enable NPCs")
	}
	press(rl.KeyF9)
	if !n.editing || !ecs.GetResource[testSwitches](app.World).shown {
		t.Fatal("F9 should show the population editor")
	}
	for _, key := range []input.Key{rl.KeyOne, rl.KeyTwo, rl.KeyBackspace, rl.KeyKp7, rl.KeyEnter} {
		press(key)
	}
	if n.population != 17 || n.editing {
		t.Fatalf("edited count: %+v", n)
	}
	press(rl.KeyF9)
	press(rl.KeyZero)
	press(rl.KeyEscape)
	if n.population != 17 || n.editing {
		t.Fatal("Esc should cancel the edit")
	}
	press(rl.KeyF9)
	for _, key := range []input.Key{rl.KeyNine, rl.KeyEight, rl.KeySeven, rl.KeySix, rl.KeyEnter} {
		press(key)
	}
	if n.population != maxTestNPCs {
		t.Fatalf("count must leave one slot for the player: %d", n.population)
	}
	press(rl.KeyF9)
	press(rl.KeyZero)
	press(rl.KeyEnter)
	if n.population != 0 {
		t.Fatal("zero should be accepted")
	}
	press(rl.KeyF8)
	if n.enabled {
		t.Fatal("F8 should disable NPCs")
	}
}

// Exercise the actual roster/command hierarchy, rather than a separate
// counter: shrinking and toggling off must remove bodies and clothes too.
func TestNPCCrowdLifecycle(t *testing.T) {
	app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{}, residentsPlugin{})
	t.Cleanup(app.Cleanup)
	n := &testNPCs{enabled: true, population: 19}
	app.InsertResource(illusion.R(n), illusion.R(&menu{}),
		illusion.R(&character.Roster{Skins: []character.Skin{{Scale: 1, Clips: makehuman}}}),
		illusion.R(&character.Wardrobe{}))
	app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		cmd.Spawn(illusion.C(character.Player{}), illusion.C(transform.FromXYZ(0, .9, 0)))
	}))
	app.AddSystems(illusion.Update, illusion.Fn7(syncTestNPCs).Before(residentsSet))
	rs := ecs.GetResource[residents](app.World)
	tick := func() { app.Tick(time.Second / 60) }
	count := func() (roots, bodies int) {
		q := ecs.NewFilter1[residentOf](app.World).Query()
		for q.Next() {
			roots++
		}
		b := ecs.NewFilter1[character.Body](app.World).Query()
		for b.Next() {
			bodies++
		}
		return
	}
	check := func(want int) {
		t.Helper()
		roots, bodies := count()
		if roots != want || bodies != want || n.live != want || len(rs.list) != want {
			t.Fatalf("want %d NPCs, roots=%d bodies=%d live=%d residents=%d", want, roots, bodies, n.live, len(rs.list))
		}
	}
	tick()
	tick()
	tick()
	check(19)
	// Attach a garment beneath each body, as the wardrobe system does.
	app.AddSystems(illusion.Update, illusion.Fn2(func(cmd *illusion.Commands, bodies *illusion.Query0Where[illusion.With[character.Body]]) {
		if n.population == 19 {
			bodies.Each(func(e ecs.Entity) { cmd.Spawn(illusion.C(character.Garment{})).ChildOf(e) })
		}
	}))
	tick()
	n.population = 3
	tick()
	check(3)
	n.enabled = false
	tick()
	check(0)
	garments := ecs.NewFilter1[character.Garment](app.World).Query()
	if garments.Next() {
		garments.Close()
		t.Fatal("NPC clothes should be removed too")
	}
	n.enabled, n.population = true, 5
	tick()
	tick()
	check(5)
	// A teleport must remove the old crowd before recreating it nearby.
	player := ecs.NewFilter1[transform.Transform](app.World).With(ecs.C[character.Player]()).Query()
	for player.Next() {
		player.Get().Translation.X = 1000
	}
	tick()
	check(0)
	tick()
	tick()
	check(5)
	for _, r := range rs.list {
		if r.home.X < 980 {
			t.Errorf("NPC stayed in the previous area: %v", r.home)
		}
	}
}

// Residents far from the player are data, and become characters again as
// the player nears, where they've got to.
func TestResidentsAreCharactersOnlyNearby(t *testing.T) {
	app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{}, residentsPlugin{})
	t.Cleanup(app.Cleanup)
	app.InsertResource(illusion.R(&menu{}), illusion.R(&character.Roster{Skins: []character.Skin{{Scale: 1, Clips: makehuman}}}))
	app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		cmd.Spawn(illusion.C(character.Player{}), illusion.C(transform.FromXYZ(0, .9, 0)))
	}))
	rs := ecs.GetResource[residents](app.World)
	near, far := rl.Vector3{X: 5}, rl.Vector3{X: 300}
	rs.add(resident{feet: near, home: near, target: near, left: 100})
	rs.add(resident{feet: far, home: far, target: rl.Vector3{X: 301}, left: 100})
	tick := func(n int) {
		for range n {
			app.Tick(time.Second / 60)
		}
	}
	characters := func() (n int) {
		q := ecs.NewFilter1[residentOf](app.World).Query()
		for q.Next() {
			n++
		}
		return
	}
	tick(2)
	if characters() != 1 || rs.near != 1 {
		t.Fatalf("only the resident nearby should be a character: %d (near %d)", characters(), rs.near)
	}
	tick(120)
	if got := rs.list[1].feet.X; got < 300.5 {
		t.Fatalf("the resident far off should go about their business as data: at x=%v", got)
	}
	// The player goes over to them: the near one's let go, and the far
	// one's a character where they'd got to.
	player := ecs.NewFilter1[transform.Transform](app.World).With(ecs.C[character.Player]()).Query()
	for player.Next() {
		player.Get().Translation.X = 300
	}
	tick(2)
	q := ecs.NewFilter2[residentOf, transform.Transform](app.World).Query()
	found := 0
	for q.Next() {
		of, tr := q.Get()
		found++
		if of.index != 1 || tr.Translation.X < 300.5 {
			t.Errorf("want resident 1 a character where they'd got to, got %d at %v", of.index, tr.Translation)
		}
	}
	if found != 1 {
		t.Fatalf("want one character after the player moved, got %d", found)
	}
}

func TestNPCWandersOnThePadsAndPausesWithMenu(t *testing.T) {
	l := newLandfall(t, arrival)
	mu := &menu{}
	rs := &residents{}
	l.app.InsertResource(illusion.R(mu), illusion.R(rs))
	l.app.AddSystems(illusion.Update, illusion.Fn5(steerResidents).Before(character.Input))
	q := ecs.NewFilter1[character.Intent](l.app.World).Query()
	var root ecs.Entity
	for q.Next() {
		root = q.Entity()
	}
	var cmd illusion.Commands
	cmd.InitParam(l.app.World)
	start := l.feet()
	rs.add(resident{feet: start, home: start, target: start})
	cmd.Entity(root).Insert(illusion.C(residentOf{}))
	l.tick(1)
	// Adding the marker moves the actor to a different archetype; refresh
	// the harness's component pointers after that structural change.
	l.in = ecs.NewMap[character.Intent](l.app.World).Get(root)
	l.cc = ecs.NewMap[physics.CharacterController](l.app.World).Get(root)
	l.tr = ecs.NewMap[transform.Transform](l.app.World).Get(root)
	l.tick(120)
	if flatDistance(start, l.feet()) < .4 || !l.cc.Grounded {
		t.Fatalf("NPC should walk on the Pads: start=%v feet=%v grounded=%v", start, l.feet(), l.cc.Grounded)
	}
	mu.open(paused)
	l.tick(1)
	if l.in.Move != (rl.Vector3{}) {
		t.Fatal("NPC should stop requesting movement while a menu is up")
	}
}
