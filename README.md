# Earth Two

A 3D game built with [illusion](https://github.com/struckchure/illusion), with Jolt physics.
The same code runs on the desktop and in the browser.

## Requirements

- Go 1.25 or newer.
- For the desktop: a C toolchain, since raylib compiles with cgo (Jolt
  too; its first build takes about half a minute). On macOS, `xcode-select --install`;
  on Debian/Ubuntu, `gcc g++ libgl1-mesa-dev libx11-dev libxi-dev libxcursor-dev libxrandr-dev
  libxinerama-dev libwayland-dev libxkbcommon-dev`; on Windows, MinGW-w64 (e.g. from MSYS2).
- For the browser: [emscripten](https://emscripten.org) (`brew install emscripten`).

## Running

```sh
make run      # opens a desktop window
make serve    # builds for the browser and serves it on http://localhost:8080
```

`make serve PORT=3000` picks another port. To serve an existing build without
rebuilding, run `go run ./cmd/web` (flags: `-addr`, `-dir`).

`make build` writes a desktop binary to `build/`, and `make web` writes the
browser build to `build/web/` (static files you can host anywhere). The first
web build compiles raylib and Jolt with emscripten, which takes a minute;
later builds take seconds.

## CI

`.github/workflows/ci.yml` vets, tests and builds the game for Linux, macOS
and Windows, and builds it for the browser, on pushes to `main` and on pull
requests. Each run uploads the builds as artifacts.

## Layout

- `game/` is the game: plugins, a startup system that spawns the scene, and
  update systems. See illusion's README for how systems, queries and
  resources work.
- `cmd/desktop/` runs the game in a desktop window.
- `cmd/web/` is the browser side. Built natively (`go run ./cmd/web`) it's
  the HTTP server for `build/web/`; built for the browser (`make web`) it's
  the game, from `game_js.go`.
- `assets/` holds files the game loads by path. Desktop builds read them from
  disk, relative to the working directory, so run the game from here. Web
  builds bundle the directory into the page.

## Notes for the browser

- Everything in `assets/` is downloaded before the game starts, so keep it to
  what the game needs.
- Browsers keep sound off until the player clicks or presses a key.
- Web builds use a browser version of raylib-go that covers the functions
  illusion and its examples use. A raylib function it doesn't have yet fails
  the web build with `undefined: rl.X`.
