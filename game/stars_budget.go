//go:build !js

package game

// On the desktop every star's drawn, each a small cross (see stars.go).
const (
	starBudget   = 1 << 30
	starsCrossed = true
)
