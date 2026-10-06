package game

import (
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/illusion"
)

// The ambience: a bed for each kind of place, mixed by how much the camera
// is in each (the light zones, zones.go), so the sound and the light change
// together. Outside, the wind, and the storm's roar as one blows; under the
// dome, its air handlers; in the Hull, its hum. Over the beds, the places
// that make a noise of their own, heard from the nearest: the machines,
// the market's chatter, the fountain.

// sourceRange is how near (full) and how far (gone) each source's loop is
// heard from it.
var sourceRange = map[string][2]float32{
	"generator": {3, 24},
	"fans":      {3, 18},
	"fountain":  {2, 22},
	"market":    {5, 32},
}

// ambience mixes the beds and the sources' loops for where the camera is.
func ambience(
	e *ears,
	w *illusion.Res[weather],
	driving *illusion.Res[vehicle.Driving],
	l *illusion.Local[loops],
) {
	view := e.view.Get()
	if !view.Active {
		return
	}
	bank, scape, dt, lp := e.bank.Get(), e.scape.Get(), e.dt(), l.Get()
	ear := listenerOf(view)
	duck := float32(1)
	if e.menu.Get().screen() != playing {
		duck = menuDuck
	}
	at := placesAt(ear.at)
	storm := w.Get().storm
	// Driving, the wind rushes past.
	rush := clamp01(abs(driving.Get().Speed) / 25)
	gain := mixAmbience * duck
	set := func(name string, volume float32) { lp.set(bank, &e.au, name, volume*gain, 1, 0, dt) }
	set("wind", (at.outside*(1-.7*storm)+.12*at.dome)*(.75+.5*rush))
	set("storm", storm*(at.outside+.3*at.dome+.1*at.hull))
	set("hull_hum", at.hull+.12*at.dome)
	set("dome_air", .55*at.dome+.2*at.hull)
	for name, r := range sourceRange {
		src, _, ok := scape.nearest(name, ear.at)
		if !ok {
			continue
		}
		vol, pan := ear.spatial(src, r[0], r[1])
		lp.set(bank, &e.au, name, vol*gain, 1, pan, dt)
	}
}
