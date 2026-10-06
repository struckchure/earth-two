package game

import (
	"testing"
	"time"
)

func TestTheWeatherIsMostlyGood(t *testing.T) {
	// Over two months, minute by minute: mostly clear, now and then
	// dusty, once in a while a storm, and never out of 0 to 1.
	start := time.Date(2026, 10, 1, 0, 0, 0, 0, time.UTC)
	steps, dusty, stormy := 0, 0, 0
	stormDays := map[int]bool{}
	for at := start; at.Before(start.AddDate(0, 2, 0)); at = at.Add(time.Minute) {
		s := stormAt(at)
		if s < 0 || s > 1 {
			t.Fatalf("at %v the weather's %v, want 0 to 1", at, s)
		}
		switch conditions(s) {
		case "Dusty":
			dusty++
		case "Dust storm":
			stormy++
			stormDays[at.YearDay()] = true
		}
		steps++
	}
	clear := steps - dusty - stormy
	t.Logf("clear %.1f%%, dusty %.1f%%, storm %.1f%% of the time; storms on %d days of 61",
		100*float32(clear)/float32(steps), 100*float32(dusty)/float32(steps), 100*float32(stormy)/float32(steps), len(stormDays))
	if share := float32(clear) / float32(steps); share < .75 {
		t.Errorf("clear %.0f%% of the time, want most of it", 100*share)
	}
	if share := float32(dusty) / float32(steps); share < .03 || share > .2 {
		t.Errorf("dusty %.1f%% of the time, want now and then", 100*share)
	}
	if share := float32(stormy) / float32(steps); share <= 0 || share > .05 {
		t.Errorf("a dust storm %.1f%% of the time, want once in a while", 100*share)
	}
	if n := len(stormDays); n < 5 || n > 40 {
		t.Errorf("dust storms on %d days of 61, want once in a while", n)
	}
}

func TestStormsBlowUpSlowly(t *testing.T) {
	// Minute to minute a storm changes by no more than a rise allows.
	start := time.Date(2026, 10, 6, 0, 0, 0, 0, time.UTC)
	last := stormAt(start)
	for at := start; at.Before(start.Add(72 * time.Hour)); at = at.Add(time.Minute) {
		s := stormAt(at)
		if d := s - last; d > .4 || d < -.4 {
			t.Fatalf("at %v the storm jumped from %v to %v in a minute", at, last, s)
		}
		last = s
	}
}

func TestEveryoneSeesTheSameStorm(t *testing.T) {
	at := time.Date(2026, 10, 6, 14, 30, 0, 0, time.UTC)
	if a, b := stormAt(at), stormAt(at.In(time.FixedZone("Landfall", 5*3600))); a != b {
		t.Fatalf("the same moment in two zones: %v and %v", a, b)
	}
}

func TestAStormCanBeHeld(t *testing.T) {
	t.Setenv("EARTH_TWO_STORM", "0.8")
	if w := newWeather(); w.forced != .8 || w.storm != .8 {
		t.Fatalf("held storm %v (blowing %v), want 0.8", w.forced, w.storm)
	}
	t.Setenv("EARTH_TWO_STORM", "")
	if w := newWeather(); w.forced >= 0 {
		t.Fatalf("with none held, held %v", w.forced)
	}
}
