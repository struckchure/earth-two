# Desktop: make run, make build. Browser: make web (or make serve to build it
# and serve it with cmd/web). Web builds need emscripten (emcc) on PATH.

GAME := earth-two
TITLE := Earth Two
PORT ?= 8080

# illusion's directory: the module cache, or a local checkout if go.mod
# replaces it. Its web/build.sh does the browser build.
ILLUSION = $(shell go list -m -f '{{.Dir}}' github.com/struckchure/illusion)

.PHONY: run build web serve test clean

run:
	go run ./cmd/desktop

build:
	go build -o build/$(GAME)$(shell go env GOEXE) ./cmd/desktop

# cmd/web compiled for the browser is the game (cmd/web/game_js.go).
web:
	sh "$(ILLUSION)/web/build.sh" -m . -o build/web -a assets -t "$(TITLE)" ./cmd/web

serve: web
	go run ./cmd/web -addr :$(PORT) -dir build/web

test:
	go test ./...

clean:
	rm -rf build
