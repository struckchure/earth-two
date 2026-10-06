package game

import (
	"testing"
	"time"
)

func TestStormsComeAndGo(t *testing.T) {
	// Over a month: storms on most days, the air clear most of the time,
	// and every storm between nothing and its peak.
	start := time.Date(2026, 10, 1, 0, 0, 0, 0, time.UTC)
	days, stormy, steps := map[int]bool{}, 0, 0
	for at := start; at.Before(start.AddDate(0, 1, 0)); at = at.Add(time.Minute) {
		s := stormAt(at)
		if s < 0 || s > 1 {
			t.Fatalf("at %v the storm's %v, want 0 to 1", at, s)
		}
		if s > .3 {
			stormy++
			days[at.YearDay()] = true
		}
		steps++
	}
	if len(days) < 20 {
		t.Errorf("storms on %d days of 31, want most", len(days))
	}
	if share := float32(stormy) / float32(steps); share < .03 || share > .25 {
		t.Errorf("stormy %.0f%% of the time, want now and then", 100*share)
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
