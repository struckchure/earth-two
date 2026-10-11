package game

import (
	"fmt"
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/render"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

type combatPickup struct {
	ScopeBench bool
	DepotStock bool // free-issue display stock stays until purchases replace collection
	Weapon     weaponState
	Rounds     int
}
type sparringPartner struct {
	Home           rl.Vector3
	Angry          bool
	Rest, Cooldown float32
}
type heldPistol struct {
	Visible bool
	Kind    weaponKind
}
type punchContact struct {
	Clip character.Anim
	Time float32
	Hit  bool
}
type combatState struct {
	WeaponRaise                                  float32
	ADS                                          float32
	Spare                                        weaponState
	SecondSlot                                   bool
	ShotSound, HitSound, ReloadSound, EmptySound bool
	Weapon                                       weaponState
	Nearby                                       ecs.Entity
	CanPickup, Aiming, Fire, Active, Down        bool
	ShotAge                                      float32
	HP                                           float32
	Note                                         string
	NoteTime, HitTime                            float32
	From                                         rl.Vector3
	Traces                                       []bulletTrace
	Flash                                        muzzleFlash
	Punches                                      map[ecs.Entity]punchContact
}

type combatActors struct {
	roots        illusion.Query4[character.Health, character.Intent, transform.Transform, character.Traversal]
	players      illusion.Query1[character.Player]
	partners     illusion.Query1[sparringPartner]
	seated       illusion.Query1[character.Seated]
	controllers  illusion.Query1[physics.CharacterController]
	bodies       illusion.Query3[character.State, render.AnimationPlayer, transform.Transform]
	cameras      illusion.Query2[transform.Transform, render.Camera3d]
	pickups      illusion.Query2[combatPickup, transform.Transform]
	weapons      illusion.Query1[heldPistol]
	weaponWorld  illusion.Query2[heldPistol, transform.GlobalTransform]
	scopes       illusion.Query1[heldScope]
	pickupScopes illusion.Query1[pickupScope]
	family       illusion.Res[character.Family]
}

func (a *combatActors) InitParam(w *ecs.World) {
	a.roots.InitParam(w)
	a.players.InitParam(w)
	a.partners.InitParam(w)
	a.seated.InitParam(w)
	a.controllers.InitParam(w)
	a.bodies.InitParam(w)
	a.cameras.InitParam(w)
	a.pickups.InitParam(w)
	a.weapons.InitParam(w)
	a.weaponWorld.InitParam(w)
	a.scopes.InitParam(w)
	a.pickupScopes.InitParam(w)
	a.family.InitParam(w)
}

type combatInput struct {
	keys     illusion.Res[input.Keys]
	buttons  illusion.Res[input.MouseButtons]
	menu     illusion.Res[menu]
	settings illusion.Res[physics.Settings]
	time     illusion.Res[illusion.Time]
	orbit    illusion.Res[orbit]
}

func (i *combatInput) InitParam(w *ecs.World) {
	i.keys.InitParam(w)
	i.buttons.InitParam(w)
	i.menu.InitParam(w)
	i.settings.InitParam(w)
	i.time.InitParam(w)
	i.orbit.InitParam(w)
}
func (i *combatInput) playing() bool {
	return i.menu.Get().screen() == playing && !i.settings.Get().Paused
}
func (s *combatState) notify(note string) { s.Note, s.NoteTime = note, 2.5 }

func spawnCombat(cmd *illusion.Commands, roster *illusion.Res[character.Roster], kit *illusion.Res[world.Kit], wardrobe *illusion.Res[character.Wardrobe]) {
	// Keep the sparring lane beside the depot, away from its entrance.
	at := depotPosition(7, .05, 0)
	roster.Get().Spawn(cmd, 0, at, illusion.C(sparringPartner{Home: at}), illusion.C(startingOutfit(wardrobe.Get())))
}
func spawnCombatPickup(cmd *illusion.Commands, kit *world.Kit, at rl.Vector3, item combatPickup) {
	name := item.Weapon.spec().Model
	if item.Rounds > 0 {
		name = "ammo_box"
	}
	cmd.Spawn(illusion.C(item), illusion.C(transform.FromTranslation(at)), illusion.C(render.Model3d{Model: kit.Model(name)}))
}
func (a *combatActors) player() (ecs.Entity, *character.Health, *character.Intent, *transform.Transform, *character.Traversal, bool) {
	e, _, ok := a.players.Single()
	if !ok {
		return e, nil, nil, nil, nil, false
	}
	h, in, tr, tv, ok := a.roots.Get(e)
	return e, h, in, tr, tv, ok
}
func (a *combatActors) ready(e ecs.Entity, h *character.Health, tv *character.Traversal) bool {
	cc, ok := a.controllers.Get(e)
	return ok && cc.Grounded && h.State == character.Healthy && !a.seated.Contains(e) && (tv.Mode == character.Idle || tv.Mode == character.Crouch)
}

// Before animation: translate both desktop and touch actions into the same intent.
func prepareCombat(cmd *illusion.Commands, actors *combatActors, input *combatInput, state *illusion.Res[combatState], p *physics.Physics, kit *illusion.Res[world.Kit]) {
	s := state.Get()
	s.Fire, s.CanPickup, s.Aiming, s.Active = false, false, false, false
	input.orbit.Get().aiming, input.orbit.Get().armed = false, false
	e, h, in, tr, tv, ok := actors.player()
	if !ok {
		return
	}
	in.LoadKg = s.carriedWeight()
	in.ADS = false
	input.orbit.Get().scoped = false
	input.orbit.Get().scopeBlend = 0
	input.orbit.Get().aimSensitivity = 1
	in.Strafe = false
	in.WeaponPose = false
	in.HipFire = false
	in.WeaponRaise = 0
	s.HP, s.Down = h.Remaining(), h.State != character.Healthy
	input.orbit.Get().armed = s.Weapon.Equipped && !s.Down && !actors.seated.Contains(e)
	if !input.playing() {
		s.ADS = 0
		s.WeaponRaise = 0
		return
	}
	dt := input.time.Get().DeltaSecs()
	s.ShotAge += dt
	s.NoteTime = max(0, s.NoteTime-dt)
	s.HitTime = max(0, s.HitTime-dt)
	s.advanceTraces(dt)
	if !actors.ready(e, h, tv) {
		s.ADS = 0
		s.WeaponRaise = 0
		s.Weapon.cancel()

		if in.Hold == character.PistolIdle || in.Hold == character.PistolAim {
			in.Hold = character.Idle
		}
		return
	}
	s.Active = true
	s.Weapon.tick(dt)
	keys := input.keys.Get()
	if keys.JustPressed(rl.KeyTab) {
		s.cycleWeapon()
	}
	if keys.JustPressed(rl.KeyZ) && s.Weapon.Owned {
		at := rl.Vector3Add(tr.Translation, rl.Vector3Scale(input.orbit.Get().forward(), .7))
		at.Y = walkHeight(at.X, at.Z) + .12
		item := s.dropWeapon()
		spawnCombatPickup(cmd, kit.Get(), at, combatPickup{Weapon: item})
		s.notify(item.spec().Name + " dropped")
	}
	nearest := float32(2)
	actors.pickups.Each(func(item ecs.Entity, pickup *combatPickup, at *transform.Transform) {
		if pickup.ScopeBench {
			if !s.Weapon.scopeCompatible() {
				return
			}
		} else if pickup.Rounds > 0 && (!s.Weapon.Owned || s.Weapon.Reserve >= s.Weapon.spec().Reserve) || pickup.Rounds == 0 && s.ownsKind(pickup.Weapon.Kind) {
			return
		}
		delta := rl.Vector3Subtract(at.Translation, tr.Translation)
		dist := rl.Vector3Length(delta)
		if dist >= nearest {
			return
		}
		if hit, blocked := p.CastRayExcluding(tr.Translation, rl.Vector3Normalize(delta), dist, e); blocked && hit.Distance < dist-.15 {
			return
		}
		nearest, s.Nearby, s.CanPickup = dist, item, true
	})
	if keys.JustPressed(rl.KeyQ) && s.CanPickup {
		item, _, found := actors.pickups.Get(s.Nearby)
		if found {
			if item.ScopeBench {
				s.Weapon.Mods.Scope = !s.Weapon.Mods.Scope
				s.ADS = 0
				if s.Weapon.Mods.Scope {
					s.notify("3x scope fitted")
				} else {
					s.notify("Scope removed")
				}
			} else if item.Rounds > 0 {
				count := min(item.Rounds, s.Weapon.spec().Reserve-s.Weapon.Reserve)
				s.Weapon.Reserve += count
				if !item.DepotStock {
					item.Rounds -= count
				}
				s.notify(fmt.Sprintf("+%d rounds", count))
				if item.Rounds == 0 && !item.DepotStock {
					cmd.Despawn(s.Nearby)
				}
			} else {
				old := s.takeWeapon(item.Weapon)
				if old.Owned {
					at := tr.Translation
					at.Y = walkHeight(at.X, at.Z) + .12
					spawnCombatPickup(cmd, kit.Get(), at, combatPickup{Weapon: old})
				}
				if !item.DepotStock {
					cmd.Despawn(s.Nearby)
				}
				s.notify(s.Weapon.spec().Name + " equipped")
			}
			s.CanPickup = false
		}
	}
	in.LoadKg = s.carriedWeight()
	input.orbit.Get().armed = s.Weapon.Equipped
	if s.Weapon.Equipped {
		if in.Act == character.Punch {
			in.Act = character.Idle
		}
		if keys.JustPressed(rl.KeyX) {
			s.ReloadSound = s.Weapon.reload()
		}
		s.Aiming = (input.buttons.Get().Pressed(rl.MouseButtonRight) || keys.Pressed(rl.KeyV)) && s.Weapon.Reload == 0
		s.advanceADS(s.Aiming, dt)
		input.orbit.Get().aimSensitivity = s.Weapon.aimSensitivity(s.aimBlend())
		input.orbit.Get().aiming = s.Aiming
		input.orbit.Get().scoped = s.sniperScoped()
		if s.sniperScoped() {
			input.orbit.Get().scopeBlend = s.aimBlend()
		}
		s.updateWeaponStance(dt)
		in.WeaponRaise = s.WeaponRaise
		in.HipFire = !s.Aiming

		in.ADS = s.Aiming
		in.Strafe = true
		in.WeaponPose = true
		in.LongGun = s.Weapon.Kind != pistol
		in.SupportReach = [...]float32{0, .37, .32, .40}[s.Weapon.Kind]
		in.ShotAge = s.ShotAge
		in.WeaponPitch = input.orbit.Get().pitch
		in.Hold = character.PistolIdle
		if s.Aiming {
			in.Hold = character.PistolAim
			in.Run = false
			input.orbit.Get().still = 0
		}

		if keys.JustPressed(rl.KeyF) || input.buttons.Get().JustPressed(rl.MouseButtonLeft) || (s.Weapon.spec().Automatic && (keys.Pressed(rl.KeyF) || input.buttons.Get().Pressed(rl.MouseButtonLeft))) {
			busy := false
			actors.family.Get().EachChild(e, func(body ecs.Entity) {
				if st, anim, _, ok := actors.bodies.Get(body); ok && st.Current.OneShot() && !anim.Finished() {
					busy = true
				}
			})
			if !busy {
				s.Fire = true
			}
			if s.Weapon.Magazine == 0 && s.Weapon.Reload == 0 {
				s.EmptySound = true
				s.notify("Empty magazine")
				s.Weapon.reload()
			}
		}
	} else {
		if in.Hold == character.PistolIdle || in.Hold == character.PistolAim {
			in.Hold = character.Idle
		}
		if input.buttons.Get().JustPressed(rl.MouseButtonLeft) {
			in.Act = character.Punch
		}
	}
	if in.Act == character.Punch || s.Weapon.Equipped {
		actors.family.Get().EachChild(e, func(body ecs.Entity) {
			if _, _, at, ok := actors.bodies.Get(body); ok {
				at.Rotation = rl.QuaternionFromEuler(0, input.orbit.Get().yaw+math.Pi, 0)
			}
		})
	}
}

func thinkCombatPartner(cmd *illusion.Commands, actors *combatActors, input *combatInput) {
	if !input.playing() {
		return
	}
	_, playerHealth, _, tr, _, ok := actors.player()
	if !ok {
		return
	}
	dt := input.time.Get().DeltaSecs()
	// The training partner is the only NPC allowed to fight in this slice.
	actors.partners.Each(func(npc ecs.Entity, enemy *sparringPartner) {
		nh, ni, nt, ntv, exists := actors.roots.Get(npc)
		if !exists {
			return
		}
		if nh.State != character.Healthy {
			enemy.Rest += dt
			if enemy.Rest >= 8 {
				character.Revive(cmd, npc, enemy.Home)
				enemy.Rest = 0
				enemy.Angry = false
			}
			return
		}
		enemy.Cooldown = max(0, enemy.Cooldown-dt)
		ni.Move = rl.Vector3{}
		distance := rl.Vector3Distance(tr.Translation, nt.Translation)
		if distance > 25 || playerHealth.State != character.Healthy {
			enemy.Angry = false
		}
		if !enemy.Angry {
			home := rl.Vector3Subtract(enemy.Home, nt.Translation)
			home.Y = 0
			if rl.Vector3Length(home) > .4 {
				ni.Move = rl.Vector3Normalize(home)
			}
			return
		}
		direction := rl.Vector3Subtract(tr.Translation, nt.Translation)
		direction.Y = 0
		if distance > 1.2 {
			ni.Move = rl.Vector3Normalize(direction)
		}
		if distance < 1.5 && enemy.Cooldown == 0 && actors.ready(npc, nh, ntv) {
			ni.Act = character.Punch
			actors.family.Get().EachChild(npc, func(body ecs.Entity) {
				if _, _, at, ok := actors.bodies.Get(body); ok {
					at.Rotation = rl.QuaternionFromEuler(0, float32(math.Atan2(float64(direction.X), float64(direction.Z))), 0)
				}
			})
			enemy.Cooldown = 1.2
		}
	})
}

// Only the local player and sparring NPC exchange damage. This intentionally
// does not grant client-authoritative damage against other residents/players.
func combatHit(a *combatActors, s *combatState, attacker, target ecs.Entity, damage float32, lethal bool) bool {
	h, _, _, _, ok := a.roots.Get(target)
	if !ok || attacker == target || (!a.players.Contains(target) && !a.partners.Contains(target)) {
		return false
	}
	cause := "Unarmed strike"
	if lethal {
		cause = "Gunshot wound"
	}
	if !h.Strike(damage, lethal, attacker, cause) {
		return false
	}
	if enemy, ok := a.partners.Get(target); ok {
		enemy.Angry = true
	}
	s.HitSound = true
	if a.players.Contains(attacker) {
		s.HitTime = .18
	}
	return true
}

func resolveCombat(actors *combatActors, input *combatInput, state *illusion.Res[combatState], p *physics.Physics) {
	if !input.playing() {
		return
	}
	s := state.Get()
	actors.cameras.Each(func(_ ecs.Entity, _ *transform.Transform, camera *render.Camera3d) {
		target := float32(45)
		if s.Aiming && s.Weapon.Equipped && !s.sniperScoped() {
			target = 45 + (s.Weapon.aimFOV()-45)*s.aimBlend()
		}
		current := camera.Fovy
		if current == 0 {
			current = 45
		}
		camera.Fovy = current + (target-current)*min(1, input.time.Get().DeltaSecs()*14)
	})
	player, health, _, tr, traversal, ok := actors.player()
	if !ok {
		return
	}
	if s.Weapon.Equipped && actors.ready(player, health, traversal) {
		actors.family.Get().EachChild(player, func(body ecs.Entity) {
			if _, _, at, ok := actors.bodies.Get(body); ok {
				at.Rotation = rl.QuaternionFromEuler(0, input.orbit.Get().yaw+math.Pi, 0)
			}
		})
	}
	if s.Fire && actors.ready(player, health, traversal) && s.Weapon.fire() {
		_, camera, _, found := actors.cameras.Single()
		if found {
			muzzle := actors.muzzle(s.Weapon.Kind, rl.Vector3Add(tr.Translation, rl.Vector3{Y: .45}))
			spec := s.Weapon.spec()
			aim := rl.Vector3{}
			for pellet := 0; pellet < spec.Pellets; pellet++ {
				direction := shotDirection(camera.Forward(), spec, pellet, s.Weapon.Magazine, s.ADS >= 1)
				hit, hitOK, end := traceWeapon(p, player, camera.Translation, direction, muzzle, spec.Range)
				if pellet == 0 {
					aim = end
				}
				s.addTrace(muzzle, end, s.Weapon.Kind)
				if hitOK {
					combatHit(actors, s, player, hit.Entity, spec.Damage, true)
				}
			}
			s.ShotAge = 0
			if _, intent, _, _, ok := actors.roots.Get(player); ok {
				intent.ShotAge = 0
			}

			s.From = muzzle
			s.Flash = muzzleFlash{From: muzzle, Direction: rl.Vector3Normalize(rl.Vector3Subtract(aim, muzzle)), Kind: s.Weapon.Kind}
			s.ShotSound = true
			input.orbit.Get().pitch = max(orbitLow, input.orbit.Get().pitch-s.Weapon.spec().Recoil)
		}
		s.Fire = false
	}
	// Animation time is the contact clock, so rejected/interrupted punches do no damage.
	alive := map[ecs.Entity]bool{}
	actors.roots.Each(func(e ecs.Entity, h *character.Health, _ *character.Intent, at *transform.Transform, tv *character.Traversal) {
		if !actors.players.Contains(e) && !actors.partners.Contains(e) {
			return
		}
		actors.family.Get().EachChild(e, func(body ecs.Entity) {
			st, anim, bodyAt, found := actors.bodies.Get(body)
			if !found {
				return
			}
			alive[body] = true
			old := s.Punches[body]
			time := anim.Time()
			punching := st.Current == character.Punch || st.Current == character.PunchRight
			if !punching || !actors.ready(e, h, tv) || (actors.players.Contains(e) && s.Weapon.Equipped) {
				delete(s.Punches, body)
				return
			}
			contact := advancePunch(&old, st.Current, time)
			if contact {
				forward := rl.Vector3RotateByQuaternion(rl.Vector3{Z: 1}, bodyAt.Rotation)
				from := rl.Vector3Add(at.Translation, rl.Vector3{Y: .3})
				// A small hand-sized sweep accepts near-centre punches, stopping at scenery.
				hit, hitOK := p.SweepCapsuleExcluding(from, rl.Vector3Scale(forward, 1.25), .12, .25, e)
				if hitOK {
					combatHit(actors, s, e, hit.Entity, 20, false)
				}
			}
			s.Punches[body] = old
		})
	})
	for body := range s.Punches {
		if !alive[body] {
			delete(s.Punches, body)
		}
	}
}

func showCombatWeapon(cmd *illusion.Commands, actors *combatActors, state *illusion.Res[combatState], kit *illusion.Res[world.Kit], scope *illusion.Res[scopeVisuals]) {
	actors.pickups.Each(func(e ecs.Entity, pickup *combatPickup, _ *transform.Transform) {
		if pickup.Weapon.Mods.Scope && !actors.pickupScopes.Contains(e) {
			parent := cmd.Entity(e).Insert(illusion.C(pickupScope{}))
			spawnScopeParts(parent, scope.Get(), pickup.Weapon.Kind, false)
		}
	})

	s := state.Get()
	player, _, _, _, _, ok := actors.player()
	if !ok {
		return
	}
	visible := heldWeaponVisible(s.Weapon, s.Down, actors.seated.Contains(player))
	count := 0
	actors.weapons.Each(func(e ecs.Entity, held *heldPistol) {
		count++
		if held.Visible == visible && held.Kind == s.Weapon.Kind {
			return
		}
		held.Visible, held.Kind = visible, s.Weapon.Kind
		cmd.Entity(e).Insert(illusion.C(render.BoneAttachment{Bone: "hand_r", Offset: weaponGrip(s.Weapon.Kind)}))
		if visible {
			cmd.Entity(e).Insert(illusion.C(render.Model3d{Model: kit.Get().Model(s.Weapon.spec().Model)}))
		} else {
			cmd.Entity(e).Remove(ecs.C[render.Model3d]())
		}
	})
	actors.scopes.Each(func(e ecs.Entity, part *heldScope) {
		show := visible && s.Weapon.Mods.Scope
		if part.Visible == show && part.Kind == s.Weapon.Kind {
			return
		}
		part.Visible, part.Kind = show, s.Weapon.Kind
		visual := scope.Get().Parts[part.Part]
		tr := visual.Transform
		tr.Translation = rl.Vector3Add(tr.Translation, scopeMount(s.Weapon.Kind))
		cmd.Entity(e).Insert(illusion.C(tr))
		if show {
			cmd.Entity(e).Insert(illusion.C(render.Mesh3d{Mesh: visual.Mesh}))
		} else {
			cmd.Entity(e).Remove(ecs.C[render.Mesh3d]())
		}
	})

	if count == 0 {
		actors.family.Get().EachChild(player, func(body ecs.Entity) {
			if !actors.bodies.Contains(body) {
				return
			}
			gun := cmd.Spawn(illusion.C(heldPistol{}), illusion.C(transform.Identity()), illusion.C(render.BoneAttachment{Bone: "hand_r", Offset: weaponGrip(s.Weapon.Kind)})).ChildOf(body)
			spawnScopeParts(gun, scope.Get(), s.Weapon.Kind, true)
		})
	}
}

func drawCombat(state *illusion.Res[combatState], mu *illusion.Res[menu], win *illusion.Res[window.Window], fonts *illusion.Res[uiFonts], actors *combatActors) {
	s := state.Get()
	syncTouchCombat(s.Active, s.Weapon.Owned, s.Weapon.Equipped, s.CanPickup, s.Down)
	if mu.Get().screen() != playing {
		return
	}
	w := win.Get()
	p := newPainter(fonts.Get(), w)
	x, y := float32(w.Width)/2, float32(w.Height)/2
	if s.Active && s.Weapon.Equipped && !s.sniperScoped() {
		color := colText
		if s.HitTime > 0 {
			color = colAccent
		}
		gap := float32(6)
		if s.Aiming {
			gap = 3
		}
		rl.DrawLineV(rl.Vector2{X: x - gap - 5, Y: y}, rl.Vector2{X: x - gap, Y: y}, color)
		rl.DrawLineV(rl.Vector2{X: x + gap, Y: y}, rl.Vector2{X: x + gap + 5, Y: y}, color)
		rl.DrawLineV(rl.Vector2{X: x, Y: y - gap - 5}, rl.Vector2{X: x, Y: y - gap}, color)
		rl.DrawLineV(rl.Vector2{X: x, Y: y + gap}, rl.Vector2{X: x, Y: y + gap + 5}, color)
	}
	hudTop := drawWeaponHUD(p, s, float32(w.Width), float32(w.Height), touchActive())
	r := rl.Rectangle{X: x - p.px(240), Y: hudTop, Width: p.px(480), Height: p.px(28)}
	if s.NoteTime > 0 {
		r.Y -= p.px(30)
		p.textIn(s.Note, r, 14, semibold, colAccent, centre)
	}
	if _, _, _, at, _, ok := actors.player(); ok && s.NoteTime == 0 && !s.CanPickup {
		actors.partners.Each(func(e ecs.Entity, enemy *sparringPartner) {
			health, _, position, _, found := actors.roots.Get(e)
			if !found || rl.Vector3DistanceSqr(at.Translation, position.Translation) > 144 {
				return
			}
			text := "Sparring partner · strike to begin"
			if enemy.Angry {
				text = fmt.Sprintf("Sparring partner · %.0f health", health.Remaining())
			}
			if health.State != character.Healthy {
				text = "Sparring partner · recovering"
			}
			r.Y -= p.px(30)
			p.textIn(text, r, 14, semibold, colAccent, centre)
		})
	}
	if s.CanPickup {
		r.Y -= p.px(30)
		label := "Pick up ammunition"
		if item, _, ok := actors.pickups.Get(s.Nearby); ok {
			if item.ScopeBench {
				label = "Fit 3x scope"
				if s.Weapon.Mods.Scope {
					label = "Remove scope"
				}
			} else if item.Rounds == 0 {
				label = "Take " + item.Weapon.spec().Name
				if s.Spare.Owned {
					label = "Replace " + s.Weapon.spec().Name + " with " + item.Weapon.spec().Name
				}
			}
		}
		if touchActive() {
			p.textIn(label, r, 14, semibold, colAccent, centre)
		} else {
			drawInteractionHint(p, "Q", label, rl.Vector2{X: p.px(20), Y: float32(w.Height) - p.px(166)})
		}
	}
}
func drawCombatTrace(state *illusion.Res[combatState], mu *illusion.Res[menu], actors *combatActors) {
	s := state.Get()
	if mu.Get().screen() != playing {
		return
	}
	s.drawTraces()
	_, _, _, player, _, ok := actors.player()
	if !ok {
		return
	}
	actors.pickups.Each(func(_ ecs.Entity, _ *combatPickup, at *transform.Transform) {
		if rl.Vector3DistanceSqr(player.Translation, at.Translation) < 400 {
			drawCombatRing(at.Translation, .35, colAccent)
		}
	})
	actors.partners.Each(func(e ecs.Entity, enemy *sparringPartner) {
		_, _, at, _, ok := actors.roots.Get(e)
		if ok && rl.Vector3DistanceSqr(player.Translation, at.Translation) < 400 {
			color := colAccent
			if enemy.Angry {
				color = rl.Red
			}
			floor := at.Translation
			floor.Y = walkHeight(floor.X, floor.Z) + .03
			drawCombatRing(floor, .65, color)
		}
	})
}

const combatResolveSet illusion.SystemSet = "game.CombatResolve"
const scopeVisualSet illusion.SystemSet = "game.ScopeVisuals"

type combatPlugin struct{}

func (combatPlugin) Build(app *illusion.App) {
	app.InsertResource(illusion.R(&scopeVisuals{}), illusion.R(&scopeScene{}))
	app.AddSystems(illusion.Startup, illusion.Fn3(setupScopeVisuals).InSet(scopeVisualSet))
	app.InsertResource(illusion.R(&combatState{Punches: map[ecs.Entity]punchContact{}}))
	app.AddSystems(illusion.Startup, illusion.Fn4(spawnCombat), illusion.Fn6(spawnWeaponsDepot).After(scopeVisualSet))
	app.AddSystems(illusion.Update, illusion.Fn3(thinkCombatPartner).After(character.Input).Before(character.Act))
	app.AddSystems(illusion.Update, illusion.Fn6(prepareCombat).After(character.Input).Before(character.Act))
	app.AddSystems(illusion.Update, illusion.Fn4(resolveCombat).After(cameraFollowSet).InSet(combatResolveSet))
	app.AddSystems(illusion.Update, illusion.Fn2(soundCombat).After(cameraFollowSet).After(combatResolveSet))
	app.AddSystems(illusion.PostUpdate, illusion.Fn5(showCombatWeapon).Before(render.Animate))
	app.AddSystems(illusion.Render, illusion.Fn5(drawCombat).InSet(render.Draw2D))
	app.AddSystems(illusion.Render, illusion.Fn4(drawSniperScope).After(render.End2D).Before(render.Draw2D))
	render.AddSceneSchedule(app, scopeScenePass)
	app.AddSystems(scopeScenePass, illusion.Fn4(drawStars).After(render.Draw3D).Before(render.End3D), illusion.Fn2(drawEffects).After(render.Draw3D).Before(render.End3D), illusion.Fn3(drawCombatTrace).After(render.Draw3D).Before(render.End3D))
	app.AddSystems(illusion.Render, scopeRenderer(app).After(render.End3D).Before(render.Begin2D))
	app.AddSystems(illusion.PostUpdate, illusion.Fn5(scopePlayerVisibility).Before(render.Animate))
	app.AddSystems(illusion.Render, illusion.Fn3(drawCombatTrace).After(render.Draw3D).Before(render.End3D))
}

// Aim from the camera, then resolve from the actor. Both rays stop at the
// first collider, including scenery and actors outside the damage allowlist.
func tracePistol(p *physics.Physics, actor ecs.Entity, eye, direction, muzzle rl.Vector3) (physics.RayHit, bool, rl.Vector3) {
	return traceWeapon(p, actor, eye, direction, muzzle, 80)
}

func traceWeapon(p *physics.Physics, actor ecs.Entity, eye, direction, muzzle rl.Vector3, distance float32) (physics.RayHit, bool, rl.Vector3) {
	aim := rl.Vector3Add(eye, rl.Vector3Scale(direction, distance))
	if hit, ok := p.CastRayExcluding(eye, direction, distance, actor); ok {
		aim = hit.Point
	}
	delta := rl.Vector3Subtract(aim, muzzle)
	hit, ok := p.CastRayExcluding(muzzle, rl.Vector3Normalize(delta), min(distance, rl.Vector3Length(delta)+.02), actor)
	if ok {
		aim = hit.Point
	}
	return hit, ok, aim
}
func advancePunch(contact *punchContact, clip character.Anim, time float32) bool {
	if contact.Clip != clip || time < contact.Time {
		contact.Hit = false
	}
	hit := !contact.Hit && time >= .25
	contact.Hit = contact.Hit || hit
	contact.Clip, contact.Time = clip, time
	return hit
}

func soundCombat(state *illusion.Res[combatState], audio *ears) {
	s := state.Get()
	if audio.menu.Get().screen() == playing {
		voice := audio.voice()
		if s.ShotSound {
			name := [...]string{"pistol", "assault", "sniper", "shotgun"}[s.Weapon.Kind]
			volume, pan := voice.ear.spatial(s.From, 3, s.Weapon.spec().Range)
			// Recorded variants supply the variation; preserve each gun's pitch.
			// Audio aliases let rapid fire overlap without cutting off the tails.
			voice.play(name, .7*volume*mixEffects*voice.duck, pan, 1, .012)
		}
		if s.HitSound {
			voice.play("thump", .4, 0, 1, .08)
		}
		if s.ReloadSound {
			voice.play("clank", .3, 0, 1.3, .03)
		}
		if s.EmptySound {
			voice.play("ui_deny", .3, 0, 1, .02)
		}
	}
	s.ShotSound, s.HitSound, s.ReloadSound, s.EmptySound = false, false, false, false
}

func drawCombatRing(at rl.Vector3, radius float32, color rl.Color) {
	previous := rl.Vector3Add(at, rl.Vector3{X: radius})
	for i := 1; i <= 32; i++ {
		angle := float64(i) * 2 * math.Pi / 32
		next := rl.Vector3Add(at, rl.Vector3{X: radius * float32(math.Cos(angle)), Z: radius * float32(math.Sin(angle))})
		rl.DrawLine3D(previous, next, color)
		previous = next
	}
}
