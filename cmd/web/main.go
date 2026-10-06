//go:build !js

// Command web serves the browser build that make web writes to build/web.
// Compiled for the browser (GOOS=js), this package is the game itself; see
// game_js.go.
//
//	go run ./cmd/web [-addr :8080] [-dir build/web]
//
// Without -addr it listens on $PORT, if that's set.
package main

import (
	"flag"
	"log"
	"net/http"
	"os"
	"path/filepath"
)

func main() {
	// Hosts that run this for us say which port to listen on in PORT.
	listen := ":8080"
	if port := os.Getenv("PORT"); port != "" {
		listen = ":" + port
	}
	addr := flag.String("addr", listen, "address to listen on")
	dir := flag.String("dir", "build/web", "directory holding the browser build")
	flag.Parse()

	if _, err := os.Stat(filepath.Join(*dir, "index.html")); err != nil {
		log.Fatalf("no browser build in %s (run make web first): %v", *dir, err)
	}

	handler, err := newFileHandler(*dir)
	if err != nil {
		log.Fatal(err)
	}

	log.Printf("serving %s on http://localhost%s", *dir, *addr)
	log.Fatal(http.ListenAndServe(*addr, handler))
}
