# Desktop: make run, make build. Browser: make web (or make serve to build it
# and serve it with cmd/web). Web builds need emscripten (emcc) on PATH.

GAME := earth-two
TITLE := Earth Two
PORT ?= 8080

# illusion's directory: the module cache, or a local checkout if go.mod
# replaces it. Its web/build.sh does the browser build.
ILLUSION = $(shell go list -m -f '{{.Dir}}' github.com/struckchure/illusion)

.PHONY: run build web serve test characters people clean

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

# Prepares skinned glTF characters for raylib (see tools/bindpose) and puts
# them in assets/characters: make characters SRC="man.glb woman.glb"
SRC ?=
characters:
	@test -n "$(SRC)" || { echo 'set SRC to the .glb files to prepare'; exit 1; }
	go run ./tools/bindpose -o assets/characters $(SRC)

# Builds the people in assets/characters from MakeHuman bodies, Quaternius's
# animation library and Mixamo clips, in Blender with MPFB installed in
# build/makehuman/blender (see the README).
BLENDER ?= /Applications/Blender.app/Contents/MacOS/Blender
UAL ?= build/makehuman/dl/ual/Universal Animation Library[Standard]/Unreal-Godot/UAL1_Standard.glb
MIXAMO ?= build/mixamo
people:
	BLENDER_USER_RESOURCES="$(CURDIR)/build/makehuman/blender" "$(BLENDER)" -b --python tools/makehuman/people.py -- "$(UAL)" assets/characters "$(MIXAMO)"

clean:
	rm -rf build
