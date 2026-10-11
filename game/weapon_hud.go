package game

import (
	"fmt"
	rl "github.com/gen2brain/raylib-go/raylib"
)

// The same card follows the selected weapon; cycling never moves the HUD.
// Health has its own bottom strip, separate from the reload indicator.
func drawWeaponHUD(p painter, s *combatState, width, height float32, touch bool) float32 {
	const cardW, cardH = 238, 77
	bottom := float32(14)
	if touch {
		bottom = 6
	}
	x, y := (width-p.px(cardW))/2, height-p.px(bottom+cardH)
	rl.DrawRectangleRec(rl.Rectangle{X: x, Y: y, Width: p.px(cardW), Height: p.px(cardH)}, rl.NewColor(16, 20, 20, 225))
	text := func(label string, dx, dy, size float32, c rl.Color) {
		p.text(label, rl.Vector2{X: x + p.px(dx), Y: y + p.px(dy)}, size, semibold, c)
	}
	w := s.Weapon
	name, status := "Fists", "UNARMED"
	if w.Equipped {
		name = w.spec().Name
		status = "READY"
		if w.Mods.Scope {
			status = "3× OPTIC"
		}
	}
	p.textIn(name, rl.Rectangle{X: x + p.px(12), Y: y + p.px(7), Width: p.px(152), Height: p.px(18)}, 14, semibold, colText, left)
	slot := 1
	if s.SecondSlot {
		slot = 2
	}
	hint := fmt.Sprintf("%d / 2", slot)
	if !w.Equipped {
		hint = "FISTS"
	}
	if !touch {
		hint += "  ·  TAB"
	}
	p.textIn(hint, rl.Rectangle{X: x + p.px(165), Y: y + p.px(8), Width: p.px(61), Height: p.px(15)}, 9, semibold, colMuted, right)
	if w.Equipped {
		text(fmt.Sprintf("%02d", w.Magazine), 12, 27, 26, colText)
		text(fmt.Sprintf("/ %03d", w.Reserve), 55, 39, 12, colMuted)
		if w.Magazine == 0 {
			status = "EMPTY"
		}
		if w.Reload > 0 {
			status = "RELOADING"
		}
	} else {
		text("Unarmed", 12, 34, 16, colMuted)
	}
	p.textIn(status, rl.Rectangle{X: x + p.px(119), Y: y + p.px(34), Width: p.px(107), Height: p.px(18)}, 10, semibold, colAccent, right)
	if w.Equipped && w.Reload > 0 {
		progress := max(float32(0), min(float32(1), 1-w.Reload/w.spec().Reload))
		rl.DrawRectangleRec(rl.Rectangle{X: x, Y: y, Width: p.px(cardW) * progress, Height: p.px(2)}, colAccent)
	}
	// Keep health visible during reloads and while the player is unarmed.
	text(fmt.Sprintf("HP %.0f", s.HP), 12, 61, 9, colText)
	bar := rl.Rectangle{X: x + p.px(56), Y: y + p.px(64), Width: p.px(cardW - 68), Height: p.px(4)}
	rl.DrawRectangleRec(bar, colControl)
	bar.Width *= max(float32(0), min(float32(1), s.HP/100))
	color := rl.NewColor(136, 175, 147, 255)
	if s.HP < 30 {
		color = rl.NewColor(230, 91, 69, 255)
	}
	rl.DrawRectangleRec(bar, color)
	return y
}
