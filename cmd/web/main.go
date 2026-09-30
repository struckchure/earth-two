//go:build !js

// Command web serves the browser build that make web writes to build/web.
// Compiled for the browser (GOOS=js), this package is the game itself; see
// game_js.go.
//
//	go run ./cmd/web [-addr :8080] [-dir build/web]
package main

import (
	"flag"
	"log"
	"net/http"
	"os"
	"path/filepath"
)

func main() {
	addr := flag.String("addr", ":8080", "address to listen on")
	dir := flag.String("dir", "build/web", "directory holding the browser build")
	flag.Parse()

	if _, err := os.Stat(filepath.Join(*dir, "index.html")); err != nil {
		log.Fatalf("no browser build in %s (run make web first): %v", *dir, err)
	}

	files := http.FileServer(http.Dir(*dir))
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Revalidate every request so a rebuild shows up on reload.
		w.Header().Set("Cache-Control", "no-cache")
		files.ServeHTTP(w, r)
	})

	log.Printf("serving %s on http://localhost%s", *dir, *addr)
	log.Fatal(http.ListenAndServe(*addr, handler))
}
