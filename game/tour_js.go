//go:build js

package game

import "errors"

// View is a place the tour stops; see tour.go. The tour is desktop only.
type View struct {
	Name   string      `json:"name"`
	Player [3]float32  `json:"player"`
	Ground bool        `json:"ground"`
	Yaw    float32     `json:"yaw"`
	Pitch  float32     `json:"pitch"`
	Eye    *[3]float32 `json:"eye,omitempty"`
	Target *[3]float32 `json:"target,omitempty"`
}

// Tour can't run in the browser: illusion's raylib there can't save
// images (rl.ExportImage).
func Tour(viewsFile, out string) error {
	return errors.New("the tour runs on the desktop only")
}
