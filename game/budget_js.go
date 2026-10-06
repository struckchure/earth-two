package game

// Browser rendering budgets: distant terrain needs far less geometry than
// the ground underfoot. Keep the near meshes and collision resolution, but
// spend less on the horizon, paint, shadows and decorative scatter.
const (
	targetFPS = 0 // browser frames are paced by requestAnimationFrame
	// One drawing pixel per CSS pixel; a Retina display otherwise multiplies
	// all fragment work and framebuffer memory by its scale squared.
	useHighDPI     = false
	useMSAA        = false
	shadowSize     = 1024
	tileCells      = 16
	detailRadius   = 2
	chunkTexels    = 64
	tileTexels     = 32
	chunksPerFrame = 1
	scatterRadius  = 2
	simulateCloth  = false
)
