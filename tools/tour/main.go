// Tour runs the game with no menus and takes a frame at each of a list of
// places, to compare the look before and after a change:
//
//	go run ./tools/tour [views.json] [out dir]
//
// views.json (tools/tour/views.json by default) lists the places: a name,
// where the player stands, and how the camera looks at them or from where
// (see game.View). Run it from the repository root, as the game; the
// frames go to out/tour by default.
package main

import (
	"fmt"
	"os"

	"github.com/struckchure/earth-two/game"
)

func main() {
	views, out := "tools/tour/views.json", "out/tour"
	if len(os.Args) > 1 {
		views = os.Args[1]
	}
	if len(os.Args) > 2 {
		out = os.Args[2]
	}
	if err := game.Tour(views, out); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
