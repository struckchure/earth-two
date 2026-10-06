//go:build !js

package game

// Desktop rendering budgets. Physics uses chunkCells on both platforms.
const (
	// A negative target opts out of illusion's default 60 FPS cap. VSync
	// still paces rendering to the display; fixed physics stays at 60 Hz.
	targetFPS      = -1
	useHighDPI     = true
	useMSAA        = true
	shadowSize     = 4096
	tileCells      = 64
	detailRadius   = 3
	chunkTexels    = 128
	tileTexels     = 64
	chunksPerFrame = 2
	scatterRadius  = 3
	simulateCloth  = true
)
