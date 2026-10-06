//go:build js

package game

// In the browser each line drawn is a call out to JavaScript, so only the
// brightest stars are drawn, each a single dot (see stars.go).
const (
	starBudget   = 2500
	starsCrossed = false
)
