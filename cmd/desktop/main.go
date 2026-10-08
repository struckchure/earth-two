// Command desktop runs Earth Two in a desktop window. Installed builds load
// bundled assets; go run uses assets/ in the source checkout.
package main

import "github.com/struckchure/earth-two/game"

func main() {
	game.Run()
}
