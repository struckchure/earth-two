// The browser build: make web compiles this package with GOOS=js
// GOARCH=wasm, where it runs the game instead of the server in main.go.
package main

import "github.com/struckchure/earth-two/game"

func main() {
	game.Run()
}
