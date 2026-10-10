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

## Search and sharing metadata

The public origin defaults to `https://earthtwo.world`. Override it with
`LANDING_SITE_URL` or `-site-url` when moving domains. Use the landing
page's domain, not the playable game's domain. This one setting supplies the
canonical URL, Open Graph URL/image, X large-image card, and `/sitemap.xml`.
`/robots.txt` advertises the sitemap. The share image is the existing 1600×900
Landfall screenshot, with descriptive alt text. VideoGame microdata describes
the game without adding JavaScript or changing the page's script policy.

Use `-site-url ""` (or an explicitly empty `LANDING_SITE_URL`) for local previews
to mark them `noindex` and omit public canonical/share URLs.
Request Host and forwarded-host headers never override the configured origin.
After deployment, submit `/sitemap.xml` in Google Search Console and request
a new scrape in social sharing debuggers if an older preview is cached.

Metadata follows the [Open Graph protocol](https://ogp.me/) and the
[VideoGame schema](https://schema.org/VideoGame).

## Installer links

Installer links come from the public `earth-two/installers/latest.json` manifest
published by CI after verifying the installer uploads. The server refreshes it
on page requests at most once a minute, with a three-second request timeout.
The manifest's CDN cache lasts 60 seconds, so new links normally appear within
two minutes. All four installers and their checksums must be present, use the
same version, and point into the manifest origin's matching release directory.
Invalid or unavailable manifests leave the last valid links in place; a fresh
server uses the bundled fallback release until a valid manifest is available.

`INSTALLER_MANIFEST_URL` or `-downloads` can override the manifest URL (HTTPS,
ending in `/installers/latest.json`). Set `-downloads ""` to use only bundled
links. The default points at the existing public download origin. Deploy this
server update once to enable automatic link refreshes for future releases.
No browser JavaScript, CORS change, or AWS credentials are needed by the page.

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
