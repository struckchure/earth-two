# Identity and accounts

The port of `game/identity.go` and its storage files. Protocol, crypto, files
and the SpacetimeDB session live in `crates/identity` (rendering-free; it
compiles for wasm32). This module is the form's state machine and the glue:

- `IdentityPanel` (a resource) holds the four fields, the saved encrypted
  backup, the unlocked key, the live session, the message line and the
  latency reading. `act`, `enabled`, `edit`, `poll` and `close` are the Go
  methods of the same names; the player-facing messages are unchanged.
- `storage` keeps the Go paths and keys: `$EARTH_TWO_IDENTITY_PATH` or
  `~/earth-two/pk`, `pub`, `identities/<id>.earth-two-key.json` and
  `backup.earth-two-key.json` on desktop; `earth-two/identity/active`,
  `earth-two/identity/saved/<id>` and `earth-two/identity/pub` in browser
  localStorage, with the file chooser and download for transfers.
- `jobs` runs each operation off the frame: a thread on desktop, a spawned
  future in the browser.
- `IdentityPlugin` polls the panel in `Update` and types keyboard text into
  the field the menus have focused (`IdentityPanel::focus`).

The live title/pause menus open `game/identity_render.rs`, which draws FORM I-1
with the shared paper primitives. Menu actions route to the panel; editing runs
before navigation, and focus/clipboard capture synchronize afterward. Closing a
form preserves the connection; dropping the panel closes it. Desktop paste uses
Bevy’s system clipboard; browser paste uses the original event adapter.

The renderer gets the pieces `drawIdentity` used:
`account_line()`, `FIELD_LABELS`, `field_text(i)`, `IdentityAction::ITEMS`,
`enabled(action)`, `message`/`failed`, `FOOTNOTE`, and for the HUD
`connection_label()` (text and RGBA, as `drawConnectionHUD`).

In the browser the page must load the existing networking adapter
(`web/spacetime/bridge.ts`, built to expose `globalThis.EarthTwoAccount`)
before the game starts; `tools/rust/web.py` builds/stages it as `account.js` and
the preview imports it before WASM starts. The Rust client calls it exactly as
the Go client did.
Desktop talks to the database in-process through `spacetimedb-sdk`, as the Go
client did through its SDK.
