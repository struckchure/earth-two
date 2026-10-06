package game

import (
	"fmt"
	"slices"
	"syscall/js"
	"time"

	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/render"
)

// ?benchmark=1 exposes rolling timing measurements for a reproducible
// browser test. Normal play installs no timing systems or DOM overlay.
type performancePlugin struct{}

func (performancePlugin) Build(app *illusion.App) {
	params := js.Global().Get("URLSearchParams").New(js.Global().Get("location").Get("search"))
	if params.Call("get", "benchmark").String() != "1" {
		return
	}
	label := "Display-paced"
	if params.Call("get", "timers").String() == "1" {
		// Headless hosts can have no working display clock. This explicitly
		// requested mode measures uncapped frame throughput, not display FPS.
		label = "Timer-paced (uncapped)"
		timer := js.FuncOf(func(_ js.Value, args []js.Value) any {
			return js.Global().Call("setTimeout", args[0], 0)
		})
		js.Global().Set("requestAnimationFrame", timer)
	}
	doc := js.Global().Get("document")
	panel := doc.Call("createElement", "pre")
	panel.Set("id", "benchmark")
	panel.Get("style").Set("cssText", "position:fixed;bottom:0;left:0;margin:0;padding:8px;background:#111d;color:white;pointer-events:none;z-index:10")
	panel.Set("textContent", "Warming up…")
	doc.Get("body").Call("appendChild", panel)
	app.AddSystems(illusion.Startup, illusion.Fn3(func(m *illusion.Res[menu], d *illusion.Res[daylight], w *illusion.Res[weather]) {
		m.Get().stack = nil
		d.Get().hour = 11
		w.Get().forced = 0
	}))
	var started, rendering, previous, report time.Time
	var cpu, draw time.Duration
	var intervals []float64
	frames := 0
	app.AddSystems(illusion.First, illusion.Fn0(func() {
		started = time.Now()
		if frames < 60 {
			panel.Set("textContent", fmt.Sprintf("Warming up… %d/60 frames", frames))
		}
		if report.IsZero() || frames == 60 {
			report = started
		}
	}))
	app.AddSystems(illusion.Render, illusion.Fn0(func() { rendering = time.Now() }).Before(render.Begin))
	app.AddSystems(illusion.Last, illusion.Fn0(func() {
		now := time.Now()
		frames++
		if frames <= 60 {
			previous = now
			return
		}
		intervals = append(intervals, float64(now.Sub(previous))/float64(time.Millisecond))
		previous = now
		cpu += now.Sub(started)
		draw += now.Sub(rendering)
		if now.Sub(report) < 5*time.Second {
			return
		}
		n := len(intervals)
		if n == 0 {
			return
		}
		var elapsed float64
		for _, ms := range intervals {
			elapsed += ms
		}
		slices.Sort(intervals)
		panel.Set("textContent", fmt.Sprintf("%s\n%.1f FPS | frame %.1f ms | p95 %.1f ms\nCPU %.1f ms | render %.1f ms | samples %d", label, 1000*float64(n)/elapsed, elapsed/float64(n), intervals[min(n-1, n*95/100)], float64(cpu)/float64(time.Millisecond)/float64(n), float64(draw)/float64(time.Millisecond)/float64(n), n))
		intervals = intervals[:0]
		cpu, draw, report = 0, 0, now
	}))
}
