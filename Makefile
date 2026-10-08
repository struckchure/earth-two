# Desktop: make run, make build. Browser: make web (or make serve to build it
# and serve it with cmd/web). Web builds need emscripten (emcc) on PATH.

GAME := earth-two
TITLE := Earth Two
PORT ?= 8080
.DEFAULT_GOAL := run

# Source fixes for native dependencies, prepared without changing the Go cache.
NATIVE_GO = env GOWORK="$(CURDIR)/build/deps/native.work" go

# illusion's directory: the module cache, or a local checkout if go.work
# uses one. Its web/build.sh does the browser build. It's downloaded first:
# a module that isn't in the cache yet has no directory to list.
ILLUSION = $(shell go mod download github.com/struckchure/illusion 2>/dev/null; go list -m -f '{{.Dir}}' github.com/struckchure/illusion)

.PHONY: assets deps run build web serve test characters people wardrobe traversal-animations paint bindpose world world-fast world-layouts clean

deps:
	go run ./tools/deps

run build test serve people wardrobe paint bindpose: deps

run:
	$(NATIVE_GO) run ./cmd/desktop

build: assets
	$(NATIVE_GO) build -o build/$(GAME)$(shell go env GOEXE) ./cmd/desktop

# Stage the release asset graph; source assets stay available to make run.
assets:
	go run ./tools/assetpack

# cmd/web compiled for the browser is the game (cmd/web/game_js.go).
web: assets
	sh "$(ILLUSION)/web/build.sh" -m . -o build/web -a build/assets -t "$(TITLE)" ./cmd/web
	go run ./tools/webcompress build/web

serve: web
	$(NATIVE_GO) run ./cmd/web -addr :$(PORT) -dir build/web

test:
	$(NATIVE_GO) test ./...

# The characters. Who they are and what they can look like is in
# tools/makehuman/cast.py; make characters builds all of assets/characters
# from it: the people, then their wardrobe and faces, then the painted look
# on all of it. It needs Blender with MPFB installed in
# build/makehuman/blender, Quaternius's animation library and the Mixamo
# clips (see the README).
#
# Its steps run on their own too:
#   make people                the bodies and their clips
#   make wardrobe              clothes, hair, glasses, faces, skins and wardrobe.json
#   make traversal-animations  the traversal clips and mirrored punch, on the bodies as they are
#   make paint                 the painted look, on what isn't painted yet
BLENDER ?= /Applications/Blender.app/Contents/MacOS/Blender
UAL ?= build/makehuman/dl/ual/Universal Animation Library[Standard]/Unreal-Godot/UAL1_Standard.glb
MIXAMO ?= build/mixamo
MAKEHUMAN = BLENDER_USER_RESOURCES="$(CURDIR)/build/makehuman/blender" "$(BLENDER)" -b --python-exit-code 1 --python
# people and wardrobe paint what they build, unless PAINT=0.
PAINT ?= 1
PAINT_CHARACTERS = $(if $(filter 0,$(PAINT)),@true,$(NATIVE_GO) run ./tools/paint assets/characters)

characters:
	$(MAKE) people PAINT=0
	$(MAKE) wardrobe

people:
	$(MAKEHUMAN) tools/makehuman/people.py -- "$(UAL)" assets/characters "$(MIXAMO)"
	$(PAINT_CHARACTERS)

wardrobe:
	python3 tools/makehuman/community.py
	$(MAKEHUMAN) tools/makehuman/wardrobe.py -- assets/characters
	$(PAINT_CHARACTERS)

traversal-animations:
	"$(BLENDER)" --background --factory-startup --python-exit-code 1 --python tools/makehuman/traversal.py -- assets/characters "$(MIXAMO)"
	python3 tools/makehuman/merge_animations.py assets/characters build/traversal
	"$(BLENDER)" --background --factory-startup --python-exit-code 1 --python tools/makehuman/check_traversal.py -- assets/characters

paint:
	$(NATIVE_GO) run ./tools/paint assets/characters

# Prepares a skinned glTF model from elsewhere for raylib (see
# tools/bindpose) and puts it in assets/characters:
# make bindpose SRC="model.glb"
SRC ?=
bindpose:
	@test -n "$(SRC)" || { echo 'set SRC to the .glb files to prepare'; exit 1; }
	$(NATIVE_GO) run ./tools/bindpose -o assets/characters $(SRC)

# The world's pieces: the Hull kit and the props, their colliders and the
# Hull test block's layout, all into assets/world (see tools/world). It
# builds with the choices in tools/world/style.json.
world:
	"$(BLENDER)" --background --factory-startup --python-exit-code 1 --python tools/world/build.py

# Unfinished: the pieces as modelled, without the bake. Seconds, for trying
# things; make world is what the game should ship.
world-fast:
	"$(BLENDER)" --background --factory-startup --python-exit-code 1 --python tools/world/build.py -- --fast

# Only the layouts (tools/world/build.py's LAYOUTS), against the pieces
# already in assets/world.
world-layouts:
	"$(BLENDER)" --background --factory-startup --python-exit-code 1 --python tools/world/build.py -- --layouts

clean:
	rm -rf build
