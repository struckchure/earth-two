# Desktop: make run, make build. Browser: make web (or make serve to build it
# and serve it with cmd/web). Web builds need emscripten (emcc) on PATH.

GAME := earth-two
TITLE := Earth Two
PORT ?= 8080
VERSION ?= 0.1.0
PYTHON ?= python3
DESKTOP_GOFLAGS = $(GOFLAGS)
ifeq ($(shell go env GOOS),windows)
DESKTOP_GOFLAGS += -ldflags=-H=windowsgui
endif
.DEFAULT_GOAL := run

# Rust migration candidate. Existing Go run/build/release defaults stay intact.
.PHONY: rust-check rust-test rust-run rust-smoke rust-web
rust-check:
	cargo check --locked -p earth-two-client --all-targets
	cargo check --locked -p earth-two-client --target wasm32-unknown-unknown

rust-test:
	cargo test --locked --workspace --no-default-features

rust-run:
	cargo run --locked -p earth-two-client

rust-smoke:
	cargo run --locked -p earth-two-client --no-default-features -- --smoke

rust-web: assets
	$(PYTHON) tools/rust/web.py

# Frozen Go evidence for incremental parity review; never changes release defaults.
.PHONY: migration-baseline migration-tools-test
migration-baseline:
	$(PYTHON) tools/migration/baseline.py $(BASELINE_ARGS)

migration-tools-test:
	$(PYTHON) -m unittest discover -s tools/migration -p 'test_*.py' -v

# Source fixes for native dependencies, prepared without changing the Go cache.
NATIVE_GO = env GOWORK="$(CURDIR)/build/deps/native.work" go

# illusion's directory: the module cache, or a local checkout if go.work
# uses one. Its web/build.sh does the browser build. It's downloaded first:
# a module that isn't in the cache yet has no directory to list.
ILLUSION = $(shell go mod download github.com/struckchure/illusion 2>/dev/null; go list -m -f '{{.Dir}}' github.com/struckchure/illusion)

.PHONY: package assets deps run build web serve landing test server server-module server-bindings account-bridge characters people wardrobe traversal-animations paint bindpose world world-fast world-layouts clean

deps:
	go run ./tools/deps

run build test serve people wardrobe paint bindpose: deps

run:
	go run ./tools/buildenv $(NATIVE_GO) run ./cmd/desktop

build: assets
	env GOFLAGS="$(DESKTOP_GOFLAGS)" go run ./tools/buildenv $(NATIVE_GO) build -o build/$(GAME)$(shell go env GOEXE) ./cmd/desktop

# Native installers for the host OS, with packed assets and SHA-256 checksums.
package: build
	$(PYTHON) tools/desktop/package.py --version "$(VERSION)"

# Stage the release asset graph; source assets stay available to make run.
assets:
	go run ./tools/assetpack

# cmd/web compiled for the browser is the game (cmd/web/game_js.go).
account-bridge:
	sh tools/webaccount/build.sh

web: assets account-bridge
	go run ./tools/buildenv sh "$(ILLUSION)/web/build.sh" -m . -o build/web -a build/assets -t "$(TITLE)" ./cmd/web
	cp build/spacetime/account.js build/web/account.js
	go run ./tools/webaccount build/web/index.html
	go run ./tools/webcompress build/web

server: server-module
	python3 tools/server/dev.py --wasm build/server/target/wasm32-unknown-unknown/release/earth_two_server.wasm

server-module:
	CARGO_TARGET_DIR="$(CURDIR)/build/server/target" cargo build --manifest-path internals/server/module/Cargo.toml --target wasm32-unknown-unknown --release --locked

# Publish the module to a local development database before generating Go bindings.
STDB_SERVER ?= http://127.0.0.1:3001
STDB_DATABASE ?= earth-two
server-bindings:
	go run go.digitalxero.dev/stdb-go@v0.7.0 generate client --server "$(STDB_SERVER)" --database "$(STDB_DATABASE)" --out-dir internals/spacetime/bindings --package bindings
	spacetime generate --lang typescript --bin-path build/server/target/wasm32-unknown-unknown/release/earth_two_server.wasm --out-dir web/spacetime/bindings --no-config --yes

serve: web
	$(NATIVE_GO) run ./cmd/web -addr :$(PORT) -dir build/web

# Standalone HTML/CSS website; no game build, Node.js or C toolchain needed.
landing:
	go run ./cmd/landing -addr :$(PORT)

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
