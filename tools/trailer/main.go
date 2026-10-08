// Trailer captures cinematic footage directly from Earth Two's renderer.
package main

import (
	"flag"
	"fmt"
	"os"

	"github.com/struckchure/earth-two/game"
)

func main() {
	shots := flag.String("shots", "tools/trailer/shots.json", "shot list")
	out := flag.String("out", "out/trailer/clips", "clip directory")
	fps := flag.Int("fps", 30, "output frame rate")
	flag.Parse()
	if err := game.Trailer(*shots, *out, *fps); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
