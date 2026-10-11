package game

import (
	rl "github.com/gen2brain/raylib-go/raylib"
	"math"
)

// These rules are local prototype values. Public PvP needs server validation.
const pistolMagazine = 12
const pistolReserveLimit = 120

type weaponKind uint8

const (
	pistol weaponKind = iota
	assaultRifle
	sniperRifle
	shotgun
)

type weaponSpec struct {
	Name, Model                                  string
	Magazine, Reserve                            int
	Delay, Reload, Damage, Range, Spread, Recoil float32
	Pellets                                      int
	HipSpread                                    float32
	Automatic                                    bool
}

var weaponSpecs = [...]weaponSpec{
	{"Earth pistol", "earth_pistol", 12, 120, .25, 1.6, 34, 80, 0, .012, 1, .012, false},
	{"Assault rifle", "laser_rifle", 30, 240, .10, 2.2, 24, 140, .012, .009, 1, .018, true},
	{"Sniper rifle", "earth_rifle", 5, 40, 1.2, 2.8, 100, 300, 0, .045, 1, .035, false},
	{"Shotgun", "shotgun", 8, 64, .85, 2.6, 18, 35, .075, .035, 8, .075, false},
}

func (w weaponState) spec() weaponSpec { return weaponSpecs[w.Kind] }

type weaponState struct {
	Mods              weaponMods
	Kind              weaponKind
	Owned, Equipped   bool
	Magazine, Reserve int
	Cooldown, Reload  float32
}

func (w *weaponState) cancel() { w.Reload = 0 }
func (w *weaponState) tick(dt float32) {
	w.Cooldown = max(0, w.Cooldown-dt)
	if w.Reload <= 0 {
		return
	}
	w.Reload = max(0, w.Reload-dt)
	if w.Reload == 0 {
		rounds := min(w.spec().Magazine-w.Magazine, w.Reserve)
		w.Magazine += rounds
		w.Reserve -= rounds
	}
}
func (w *weaponState) reload() bool {
	if !w.Equipped || w.Reload > 0 || w.Magazine >= w.spec().Magazine || w.Reserve == 0 {
		return false
	}
	w.Reload = w.spec().Reload
	return true
}
func (w *weaponState) fire() bool {
	if !w.Equipped || w.Reload > 0 || w.Cooldown > 0 || w.Magazine == 0 {
		return false
	}
	w.Magazine--
	w.Cooldown = w.spec().Delay
	return true
}
func (w *weaponState) swap() {
	if !w.Owned {
		return
	}
	w.Equipped = !w.Equipped
	w.cancel()
	w.Cooldown = max(w.Cooldown, .35)
}

// Deterministic per-shot spread, shared by mouse and touch firing.
func shotDirection(forward rl.Vector3, spec weaponSpec, pellet, shot int, aiming bool) rl.Vector3 {
	spread := spec.Spread
	if !aiming {
		spread = max(spread, spec.HipSpread)
	}
	if aiming && spec.Pellets == 1 {
		spread *= .25
	}
	if spread == 0 {
		return forward
	}
	angle := float64(pellet)*2.399963 + float64(shot)*1.618
	radius := spread
	if spec.Pellets > 1 {
		radius *= float32(math.Sqrt(float64(pellet) / float64(spec.Pellets-1)))
	}
	right := rl.Vector3Normalize(rl.Vector3CrossProduct(forward, rl.Vector3{Y: 1}))
	up := rl.Vector3Normalize(rl.Vector3CrossProduct(right, forward))
	return rl.Vector3Normalize(rl.Vector3Add(forward, rl.Vector3Add(rl.Vector3Scale(right, radius*float32(math.Cos(angle))), rl.Vector3Scale(up, radius*float32(math.Sin(angle))))))
}

// Attachments belong to the weapon, so dropping or switching preserves them.
type weaponMods struct{ Scope bool }

func (w weaponState) weight() float32 {
	if !w.Owned {
		return 0
	}
	mass := [...]float32{.9, 3.6, 5.2, 3.4}[w.Kind]
	round := [...]float32{.012, .012, .025, .045}[w.Kind]
	if w.Mods.Scope {
		mass += .45
	}
	return mass + float32(w.Magazine+w.Reserve)*round
}
func (w weaponState) scopeCompatible() bool { return w.Owned && w.Kind != pistol }
func (w weaponState) aimFOV() float32 {
	if !w.Mods.Scope {
		return 45
	}
	return 16 // approximately 3x magnification
}

func (s *combatState) carriedWeight() float32 { return s.Weapon.weight() + s.Spare.weight() }
func (s *combatState) cycleWeapon() {
	s.ADS = 0
	s.Weapon.cancel()
	if s.Weapon.Equipped {
		if !s.SecondSlot && s.Spare.Owned {
			s.Weapon, s.Spare = s.Spare, s.Weapon
			s.SecondSlot = true
		} else {
			s.Weapon.Equipped = false
			return
		}
	} else if s.SecondSlot && s.Spare.Owned {
		s.Weapon, s.Spare = s.Spare, s.Weapon
		s.SecondSlot = false
	}
	s.Spare.Equipped = false
	s.Weapon.Equipped = s.Weapon.Owned
	s.Weapon.Cooldown = max(s.Weapon.Cooldown, .35)
}

// Take a free slot first, otherwise exchange only the selected gun. Return
// the displaced weapon for a world pickup, preserving ammo and attachments.
func (s *combatState) takeWeapon(item weaponState) weaponState {
	s.ADS = 0
	old := weaponState{}
	s.Weapon.cancel()
	if s.Weapon.Owned {
		if !s.Spare.Owned {
			s.Spare = s.Weapon
			s.SecondSlot = true
		} else {
			old = s.Weapon
		}
	}
	s.Weapon = item
	s.Weapon.Owned, s.Weapon.Equipped = true, true
	s.Weapon.cancel()
	s.Weapon.Cooldown = .5
	s.Spare.Equipped = false
	return old
}
func (s *combatState) dropWeapon() weaponState {
	s.ADS = 0
	old := s.Weapon
	old.cancel()
	old.Cooldown = 0
	s.Weapon, s.Spare = s.Spare, weaponState{}
	s.Weapon.Equipped = s.Weapon.Owned
	s.Weapon.Cooldown = .35
	s.SecondSlot = false
	return old
}
func (s *combatState) ownsKind(kind weaponKind) bool {
	return s.Weapon.Owned && s.Weapon.Kind == kind || s.Spare.Owned && s.Spare.Kind == kind
}
func heldWeaponVisible(w weaponState, down, seated bool) bool {
	return w.Owned && w.Equipped && !down && !seated
}

func (s *combatState) sniperScoped() bool {
	return s.Active && s.ADS > 0 && s.Weapon.Equipped && s.Weapon.Kind == sniperRifle && s.Weapon.Mods.Scope
}

func (w weaponState) adsTime() float32 {
	duration := [...]float32{.18, .27, .46, .30}[w.Kind]
	if w.Mods.Scope {
		duration += .09
	}
	return duration
}
func (s *combatState) advanceADS(held bool, dt float32) {
	if !s.Active || !s.Weapon.Equipped || s.Weapon.Reload > 0 {
		s.ADS = 0
		return
	}
	if held {
		s.ADS = min(1, s.ADS+max(0, dt)/s.Weapon.adsTime())
	} else {
		s.ADS = max(0, s.ADS-max(0, dt)/.18)
	}
}
func (s *combatState) aimBlend() float32   { return s.ADS * s.ADS * (3 - 2*s.ADS) }
func (s *combatState) scopeBlend() float32 { return max(0, min(1, (s.aimBlend()-.2)/.8)) }

// Match angular input to the visible zoom rather than magnifying each mouse
// count. The circular sniper lens is a separate 3x render at normal camera FOV.
func (w weaponState) aimSensitivity(blend float32) float32 {
	ratio := float32(math.Tan(float64(w.aimFOV())*math.Pi/360) / math.Tan(45*math.Pi/360))
	if w.Kind == sniperRifle && w.Mods.Scope {
		ratio = 1.0 / 3
	}
	return 1 + (ratio-1)*blend
}

// Keep the equipped weapon raised, easing into the pose when drawing it.
func (s *combatState) updateWeaponStance(dt float32) {
	s.WeaponRaise += (1 - s.WeaponRaise) * float32(1-math.Exp(float64(-18*dt)))
}
