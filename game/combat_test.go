package game

import (
	"math"
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
)

func TestPistolReloadAndCancellation(t *testing.T) {
	w := weaponState{Owned: true, Equipped: true, Magazine: 3, Reserve: 5}
	if !w.reload() || w.fire() {
		t.Fatal("reload must block fire")
	}
	w.tick(.8)
	if w.Magazine != 3 || w.Reserve != 5 {
		t.Fatal("ammo transferred before reload finished")
	}
	w.swap()
	w.tick(2)
	if w.Magazine != 3 || w.Reserve != 5 || w.Equipped {
		t.Fatal("holster must cancel without losing ammo")
	}
	w.swap()
	w.reload()
	w.tick(2)
	if w.Magazine != 8 || w.Reserve != 0 || w.reload() {
		t.Fatalf("partial reserve reload: %+v", w)
	}
	if !w.fire() || w.fire() {
		t.Fatal("cadence must stop repeat fire")
	}
	w.tick(.25)
	if !w.fire() {
		t.Fatal("cooldown never ended")
	}
	w.Magazine = 0
	w.tick(1)
	if w.fire() || w.Magazine != 0 {
		t.Fatal("empty fire produced ammunition")
	}
}
func TestPunchSingleContactPerAnimation(t *testing.T) {
	var c punchContact
	if advancePunch(&c, character.Punch, .1) {
		t.Fatal("hit before contact")
	}
	if !advancePunch(&c, character.Punch, .3) {
		t.Fatal("missed contact when a frame crossed the threshold")
	}
	if advancePunch(&c, character.Punch, .4) || advancePunch(&c, character.Punch, .9) {
		t.Fatal("duplicate damage")
	}
	if advancePunch(&c, character.PunchRight, .1) || !advancePunch(&c, character.PunchRight, .3) {
		t.Fatal("alternating fist did not reset")
	}
	if advancePunch(&c, character.PunchRight, .1) || !advancePunch(&c, character.PunchRight, .3) {
		t.Fatal("restarted animation did not reset")
	}
}
func TestPistolCameraAndMuzzleOcclusion(t *testing.T) {
	for _, tc := range []struct {
		name    string
		wall    bool
		eye     rl.Vector3
		wantHit bool
	}{
		{"clear", false, rl.Vector3{Y: 1, Z: 3}, true},
		{"wall", true, rl.Vector3{Y: 1, Z: 3}, false},
		{"camera past wall", true, rl.Vector3{Y: 1, Z: -2}, false},
	} {
		t.Run(tc.name, func(t *testing.T) {
			app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{})
			defer app.Cleanup()
			app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
				cmd.Spawn(illusion.C(sparringPartner{}), illusion.C(physics.Static), illusion.C(physics.Cuboid(1, 2, 1)), illusion.C(transform.FromXYZ(0, 1, -5)))
				if tc.wall {
					cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(4, 3, .2)), illusion.C(transform.FromXYZ(0, 1, -1)))
				}
			}))
			app.Tick(time.Second / 60)
			actor := ecs.NewMap[character.Player](app.World).NewEntity(&character.Player{})
			var p physics.Physics
			p.InitParam(app.World)
			hit, ok, _ := tracePistol(&p, actor, tc.eye, rl.Vector3{Z: -1}, rl.Vector3{Y: 1})
			if !ok {
				t.Fatal("no collider hit")
			}
			target := ecs.NewMap[sparringPartner](app.World).Has(hit.Entity)
			if target != tc.wantHit {
				t.Fatalf("target hit=%v, wanted %v", target, tc.wantHit)
			}
		})
	}
}

func TestCombatInputPausePickupAndVehicle(t *testing.T) {
	app := illusion.New().AddPlugins(transform.Plugin{}, physics.Plugin{})
	defer app.Cleanup()
	keys := input.NewButtonInput[input.Key]()
	buttons := input.NewButtonInput[input.MouseButton]()
	state := &combatState{Punches: map[ecs.Entity]punchContact{}}
	mu := &menu{}
	app.InsertResource(illusion.R(keys), illusion.R(buttons), illusion.R(mu), illusion.R(newOrbit()), illusion.R(state), illusion.R(&world.Kit{}), illusion.R(&character.Family{}))
	var player ecs.Entity
	app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		cmd.Spawn(illusion.C(character.Player{}), illusion.C(character.Health{}), illusion.C(character.Intent{}), illusion.C(character.Traversal{}), illusion.C(physics.CharacterController{Radius: .3, Height: 1.8, StepHeight: .3}), illusion.C(transform.FromXYZ(0, .9, 0)))
		cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(20, 1, 20)), illusion.C(transform.FromXYZ(0, -.5, 0)))
		cmd.Spawn(illusion.C(combatPickup{Weapon: weaponState{Owned: true, Equipped: true, Magazine: 7, Reserve: 19}}), illusion.C(transform.FromXYZ(.5, .15, 0)))
		cmd.Spawn(illusion.C(render.Camera3d{}), illusion.C(transform.FromXYZ(0, 1.5, 4)))
	}))
	app.AddSystems(illusion.Update, illusion.Chain(illusion.Fn6(prepareCombat), illusion.Fn4(resolveCombat)))
	tick := func(n int) {
		for range n {
			app.Tick(time.Second / 60)
		}
	}
	tick(20)
	q := ecs.NewFilter1[character.Player](app.World).Query()
	for q.Next() {
		player = q.Entity()
	}
	if !state.CanPickup {
		t.Fatal("nearby weapon not offered")
	}
	keys.Press(rl.KeyQ)
	tick(1)
	keys.Release(rl.KeyF)
	keys.Release(rl.KeyQ)
	keys.Clear()
	if !state.Weapon.Equipped || state.Weapon.Magazine != 7 || state.Weapon.Reserve != 19 {
		t.Fatalf("pickup did not preserve ammo: %+v", state.Weapon)
	}
	tick(1)
	if state.CanPickup {
		t.Fatal("consumed weapon still offered")
	}
	state.Weapon.reload()
	before := state.Weapon.Reload
	mu.stack = []page{{screen: paused}}
	keys.Press(rl.KeyF)
	tick(10)
	keys.Release(rl.KeyF)
	keys.Release(rl.KeyQ)
	keys.Clear()
	if state.Weapon.Reload != before || state.Weapon.Magazine != 7 {
		t.Fatal("paused combat changed ammo or reload time")
	}
	mu.stack = nil
	seats := ecs.NewMap[character.Seated](app.World)
	seats.Add(player, &character.Seated{})
	keys.Press(rl.KeyF)
	tick(1)
	keys.Release(rl.KeyF)
	keys.Release(rl.KeyQ)
	keys.Clear()
	if state.Active || state.Weapon.Reload != 0 || state.Weapon.Magazine != 7 {
		t.Fatal("seated actor could continue combat")
	}
	seats.Remove(player)
	tick(30)
	keys.Press(rl.KeyF)
	tick(1)
	keys.Release(rl.KeyF)
	keys.Release(rl.KeyQ)
	keys.Clear()
	if state.Weapon.Magazine != 6 {
		t.Fatal("standing actor could not fire")
	}

	traversal := ecs.NewMap[character.Traversal](app.World).Get(player)
	traversal.Mode = character.Crouch
	tick(30)
	keys.Press(rl.KeyV)
	keys.Press(rl.KeyF)
	tick(1)
	keys.Release(rl.KeyF)
	keys.Release(rl.KeyV)
	keys.Clear()
	intent := ecs.NewMap[character.Intent](app.World).Get(player)
	if state.Weapon.Magazine != 5 || !state.Aiming || !intent.WeaponPose || !intent.Strafe {
		t.Fatal("crouched actor must retain ADS, directional weapon pose, and firing")
	}
	traversal.Mode = character.Idle
	state.Weapon.Magazine = 6
	stock := ecs.NewMap2[combatPickup, transform.Transform](app.World)
	depot := stock.NewEntity(&combatPickup{DepotStock: true, Rounds: 36}, ptrTransform(rl.Vector3{X: .5, Y: .78}))
	// Include the actual table collider: the stock must be reachable across its top.
	var cmd illusion.Commands
	cmd.InitParam(app.World)
	cmd.Spawn(illusion.C(physics.Static), illusion.C(physics.Cuboid(1.2, .75, .7)), illusion.C(transform.FromXYZ(.5, .375, 0)))
	// Approach the near edge, rather than standing inside the table.
	ecs.NewMap[transform.Transform](app.World).Get(player).Translation = rl.Vector3{X: .5, Y: .9, Z: -.85}
	tick(5)
	for _, want := range []int{55, 91, 120} {
		keys.Press(rl.KeyQ)
		tick(1)
		keys.Release(rl.KeyQ)
		keys.Clear()
		tick(1)
		if state.Weapon.Reserve != want {
			t.Fatalf("depot reserve=%d, want %d", state.Weapon.Reserve, want)
		}
		item, _ := stock.Get(depot)
		if item.Rounds != 36 {
			t.Fatal("depot stock depleted")
		}
	}
	if state.CanPickup {
		t.Fatal("full ammunition inventory still offered a pickup")
	}
	station, _ := stock.Get(depot)
	station.ScopeBench, station.Rounds = true, 0
	tick(1)
	if state.CanPickup {
		t.Fatal("scope bench accepted the pistol")
	}
	state.Weapon.Kind = assaultRifle
	tick(1)
	if !state.CanPickup {
		t.Fatal("scope bench did not offer fitting for a rifle")
	}
	keys.Press(rl.KeyQ)
	tick(1)
	keys.Release(rl.KeyQ)
	keys.Clear()
	if !state.Weapon.Mods.Scope {
		t.Fatal("scope fitting failed")
	}
	tick(1)
	keys.Press(rl.KeyQ)
	tick(1)
	keys.Release(rl.KeyQ)
	keys.Clear()
	if state.Weapon.Mods.Scope {
		t.Fatal("scope removal failed")
	}

}

func TestCombatDoesNotDamageOrdinaryResidents(t *testing.T) {
	app := illusion.New()
	defer app.Cleanup()
	actor := ecs.NewMap[character.Player](app.World).NewEntity(&character.Player{})
	targets := ecs.NewMap4[character.Health, character.Intent, transform.Transform, character.Traversal](app.World)
	npc := targets.NewEntity(&character.Health{}, &character.Intent{}, &transform.Transform{}, &character.Traversal{})
	var a combatActors
	a.InitParam(app.World)
	s := &combatState{}
	if combatHit(&a, s, actor, npc, 34, true) {
		t.Fatal("local client damaged an ordinary resident")
	}
	ecs.NewMap[sparringPartner](app.World).Add(npc, &sparringPartner{})
	if !combatHit(&a, s, actor, npc, 34, true) {
		t.Fatal("training target did not take damage")
	}
	h, _, _, _ := targets.Get(npc)
	if h.Remaining() != 66 {
		t.Fatal("incorrect damage")
	}
}

func TestWeaponClassesCadenceReloadAndSpread(t *testing.T) {
	for kind, spec := range weaponSpecs {
		w := weaponState{Kind: weaponKind(kind), Owned: true, Equipped: true, Magazine: spec.Magazine, Reserve: spec.Reserve}
		if !w.fire() || w.Magazine != spec.Magazine-1 || w.fire() {
			t.Fatalf("%s cadence", spec.Name)
		}
		w.tick(spec.Delay)
		if !w.fire() {
			t.Fatalf("%s never ready", spec.Name)
		}
		if !w.reload() {
			t.Fatalf("%s reload rejected", spec.Name)
		}
		w.tick(spec.Reload)
		if w.Magazine != spec.Magazine || w.Reserve != spec.Reserve-2 {
			t.Fatalf("%s ammo transfer: %+v", spec.Name, w)
		}
	}
	forward := rl.Vector3{Z: -1}
	center := shotDirection(forward, weaponSpecs[shotgun], 0, 0, false)
	edge := shotDirection(forward, weaponSpecs[shotgun], 7, 0, false)
	if center != forward || rl.Vector3Distance(center, edge) < .05 {
		t.Fatal("shotgun has no pellet cone")
	}
	if shotDirection(forward, weaponSpecs[sniperRifle], 0, 0, true) != forward {
		t.Fatal("sniper aim is not precise")
	}
}

func TestTwoWeaponLoadoutPreservesAmmoAndMods(t *testing.T) {
	first := weaponState{Owned: true, Equipped: true, Kind: assaultRifle, Magazine: 19, Reserve: 41, Mods: weaponMods{Scope: true}}
	second := weaponState{Owned: true, Kind: shotgun, Magazine: 5, Reserve: 12}
	s := combatState{}
	if old := s.takeWeapon(first); old.Owned {
		t.Fatal("first gun displaced something")
	}
	if old := s.takeWeapon(second); old.Owned {
		t.Fatal("second gun did not fill the spare slot")
	}
	weight := s.carriedWeight()
	s.cycleWeapon() // second slot -> fists
	if s.Weapon.Equipped || s.carriedWeight() != weight {
		t.Fatal("holstering lost equipment weight")
	}
	s.cycleWeapon() // fists -> first slot
	if s.Weapon.Kind != assaultRifle || !s.Weapon.Mods.Scope || s.Weapon.Magazine != 19 || s.Weapon.Reserve != 41 {
		t.Fatalf("switch lost attachment/ammo: %+v", s.Weapon)
	}
	s.Weapon.Reload = 1
	s.cycleWeapon()
	if s.Weapon.Kind != shotgun || s.Spare.Reload != 0 {
		t.Fatal("switch did not cancel stored reload")
	}
	old := s.takeWeapon(weaponState{Owned: true, Kind: sniperRifle, Magazine: 5, Reserve: 10})
	if old.Kind != shotgun || old.Magazine != 5 || !s.Spare.Mods.Scope {
		t.Fatal("third pickup displaced the wrong slot")
	}
	dropped := s.dropWeapon()
	if dropped.Kind != sniperRifle || s.Spare.Owned || s.Weapon.Kind != assaultRifle || !s.Weapon.Mods.Scope {
		t.Fatal("drop did not preserve remaining gun")
	}
	s.dropWeapon()
	if s.Weapon.Owned || s.carriedWeight() != 0 {
		t.Fatal("empty loadout retained weight")
	}
}

func TestScopesAndWeaponVisibility(t *testing.T) {
	w := weaponState{Owned: true, Equipped: true, Kind: sniperRifle}
	bare := w.weight()
	if w.aimFOV() != 45 || !w.scopeCompatible() {
		t.Fatal("bare rifle unexpectedly magnified")
	}
	w.Mods.Scope = true
	if w.aimFOV() >= 45 || w.weight() <= bare {
		t.Fatal("scope must add weight and magnification")
	}
	if (weaponState{Owned: true, Kind: pistol}).scopeCompatible() {
		t.Fatal("pistol accepted long-gun scope")
	}
	// Traversal/fire readiness deliberately is not a condition for attachment
	// visibility: jumps, slides, reloads and paused frames must retain the gun.
	if !heldWeaponVisible(w, false, false) {
		t.Fatal("equipped weapon hidden outside firing state")
	}
	if heldWeaponVisible(w, true, false) || heldWeaponVisible(w, false, true) {
		t.Fatal("downed/seated weapon should be stowed")
	}
	w.Equipped = false
	if heldWeaponVisible(w, false, false) {
		t.Fatal("holstered gun remained in the hand")
	}
}

func TestHeldGunRemainsRenderedWhenCombatIsInactive(t *testing.T) {
	app := illusion.New()
	defer app.Cleanup()
	state := &combatState{Weapon: weaponState{Owned: true, Equipped: true, Kind: assaultRifle, Mods: weaponMods{Scope: true}}}
	app.InsertResource(illusion.R(state), illusion.R(&world.Kit{}), illusion.R(&scopeVisuals{}), illusion.R(&character.Family{}))
	var gun ecs.Entity
	app.AddSystems(illusion.Startup, illusion.Fn1(func(cmd *illusion.Commands) {
		cmd.Spawn(illusion.C(character.Player{}), illusion.C(character.Health{}), illusion.C(character.Intent{}), illusion.C(character.Traversal{}), illusion.C(transform.Identity()))
		cmd.Spawn(illusion.C(heldPistol{}), illusion.C(transform.Identity())).Then(func(_ *ecs.World, e ecs.Entity) { gun = e })
	}))
	app.AddSystems(illusion.PostUpdate, illusion.Fn5(showCombatWeapon))
	app.Tick(time.Second / 60)
	models := ecs.NewMap[render.Model3d](app.World)
	if !models.Has(gun) {
		t.Fatal("inactive combat hid the equipped gun (jump/slide regression)")
	}
	state.Active = true
	app.Tick(time.Second / 60)
	state.Active = false
	app.Tick(time.Second / 60)
	if !models.Has(gun) {
		t.Fatal("leaving the grounded combat state removed the gun")
	}
	state.Weapon.Equipped = false
	app.Tick(time.Second / 60)
	if models.Has(gun) {
		t.Fatal("holstering failed to remove the held model")
	}
}

func TestSniperOpticOnlyAppearsWhileScopedAndAiming(t *testing.T) {
	s := combatState{Active: true, Aiming: true, ADS: 1, Weapon: weaponState{Owned: true, Equipped: true, Kind: sniperRifle, Mods: weaponMods{Scope: true}}}
	if !s.sniperScoped() {
		t.Fatal("sniper ADS did not activate its optic")
	}
	s.Aiming = false
	s.advanceADS(false, .18)
	if s.sniperScoped() {
		t.Fatal("optic remained after releasing aim")
	}
	s.Aiming = true
	s.advanceADS(true, 1)
	s.Weapon.Kind = assaultRifle
	if s.sniperScoped() {
		t.Fatal("rifle got sniper full-screen optic")
	}
	s.Weapon.Kind = sniperRifle
	s.Weapon.Mods.Scope = false
	if s.sniperScoped() {
		t.Fatal("bare sniper got an optical sight")
	}
}

func TestADSTimingAndCancellation(t *testing.T) {
	s := combatState{Active: true, Weapon: weaponState{Owned: true, Equipped: true, Kind: sniperRifle, Mods: weaponMods{Scope: true}}}
	s.advanceADS(true, .275)
	if math.Abs(float64(s.ADS-.5)) > .001 {
		t.Fatalf("halfway ADS = %v", s.ADS)
	}
	forward := rl.Vector3{Z: -1}
	if shotDirection(forward, s.Weapon.spec(), 0, 0, s.ADS >= 1) == forward {
		t.Fatal("partial ADS gave sniper full accuracy")
	}
	s.advanceADS(true, .275)
	if shotDirection(forward, s.Weapon.spec(), 0, 0, s.ADS >= 1) != forward {
		t.Fatal("settled sniper is inaccurate")
	}
	if s.ADS != 1 {
		t.Fatalf("scope must settle after 550ms: %v", s.ADS)
	}
	s.advanceADS(false, .09)
	if s.ADS <= 0 || s.ADS >= 1 {
		t.Fatal("release must blend out")
	}
	s.advanceADS(false, .09)
	if s.ADS != 0 {
		t.Fatal("scope did not lower")
	}
	for _, cancel := range []func(){func() { s.Weapon.Reload = 1 }, func() { s.Active = false }, func() { s.Weapon.Equipped = false }} {
		s.Active = true
		s.Weapon.Equipped = true
		s.Weapon.Reload = 0
		s.ADS = 1
		cancel()
		s.advanceADS(true, .1)
		if s.ADS != 0 {
			t.Fatal("unavailable weapon retained ADS")
		}
	}
}

func TestScopeSensitivityTracksMagnification(t *testing.T) {
	w := weaponState{Kind: sniperRifle, Mods: weaponMods{Scope: true}}
	if w.aimSensitivity(0) != 1 || math.Abs(float64(w.aimSensitivity(1)-1.0/3)) > 1e-6 {
		t.Fatal("sniper sensitivity does not follow zoom")
	}
	w.Kind = assaultRifle
	if w.aimSensitivity(1) >= .4 || w.aimSensitivity(1) <= .3 {
		t.Fatal("rifle scope sensitivity does not follow its FOV")
	}
	w.Mods.Scope = false
	if w.aimSensitivity(1) != 1 {
		t.Fatal("unzoomed aim changed sensitivity")
	}
}

func TestEquippedWeaponStaysRaised(t *testing.T) {
	s := combatState{}
	s.updateWeaponStance(1)
	if s.WeaponRaise < .99 {
		t.Fatal("equipped weapon did not raise")
	}
	s.updateWeaponStance(1)
	if s.WeaponRaise < .99 {
		t.Fatal("equipped weapon lowered during movement")
	}
}
