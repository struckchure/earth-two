# Earth Two landing page

Plain HTML and CSS served by Go's standard library. The page reuses the game's
Inter and Courier Prime fonts, Exchange paperwork, paper and ink palette,
rubber stamps, brass seals and actual in-game screenshots. No JavaScript or
frontend build is required.

From the repository root:

```sh
make landing
# http://localhost:8080

# Choose another address or trailer file:
go run ./cmd/landing -addr :8090 -trailer out/trailer/v2/earth-two-trailer.mp4

# Create a standalone server binary:
CGO_ENABLED=0 go build -o build/landing ./cmd/landing
./build/landing -addr :8080 -trailer /path/to/trailer.mp4
```

The server embeds `index.html`, `styles.css`, and `assets/`, so the binary can
run outside this repository. Restart/rebuild after editing embedded files.
`PORT` provides the default port; `-addr` overrides it.

The video is optional and streamed from disk with HTTP byte-range support for
seeking. The default is the revised teaser in
`out/trailer/v2/earth-two-trailer.mp4`. If absent, the page displays a poster and
an honest fallback instead of a broken player. Use `-trailer ""` to omit it.
Ship the video alongside the binary to include it in a deployment.

The game browser build continues to be served separately by `cmd/web`.
There are no newsletter forms, account services or unconfigured play links.
All fonts and images are local; asset and font credits accompany the page.

The layout adapts from 320px phones to wide desktop screens. Narrow tablets
stack the hero, gallery, and work-order content; navigation and links provide
44px touch targets. Images and the native video player fit their container.

## Railpack

Use the root `railpack.landing-page.json` to build only this Go server, with
the HTML, CSS, images and fonts embedded in its executable. The runtime reads
`PORT` automatically and needs no Emscripten, Node.js or native game libraries.

On Railway, set `RAILPACK_CONFIG_FILE=railpack.landing-page.json` for the
landing-page service. The existing `railpack.json` still builds the browser game.
With the Railpack CLI, select it using:

```sh
railpack build --config-file railpack.landing-page.json .
```

The generated trailer is not bundled with the server. To include the optional
video, supply the MP4 on a runtime volume and override the start command with
`./landing-page -trailer /path/to/trailer.mp4`. The generated local video in
`out/` is ignored by Git and is not included automatically. Without it, the
poster fallback remains available.

See [Railpack configuration](https://railpack.com/config/file/) for config-file
selection and deploy settings.

Validation:

```sh
go test ./web/landing ./cmd/landing
```
