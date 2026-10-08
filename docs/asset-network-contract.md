# Shared asset references for multiplayer

The asset work is in the `optimize-deduplicate` branch/worktree at
`/Users/mohammed/harnesses/worktrees/earth-two/solar-island`. This contract
covers asset references and appearance conversion; the multiplayer session
owns networking, identities, connection flow and remote-player replication.

## Build and compatibility

`make assets`, `make build`, `make web` and desktop CI run `tools/assetpack`.
The release assets are staged in `build/assets`. Sources in `assets` are
unchanged. Unused assets and catalogue entries are stripped, PNGs are
recompressed losslessly, and identical textures and large binary views are
shared using relative glTF URIs under `_packed/`. Keep that directory with
the GLBs when packaging or updating a client. Web mounts the pack at
`/build/assets`; desktop packages keep it beside the executable as `assets`.

Every release pack includes `asset-index.json`:

- `version`: catalogue schema version, currently 1.
- `packHash`: full SHA-256 of a canonical, path-sorted list of file IDs,
  paths, content SHA-256 digests and sizes. This includes runtime JSON
  manifests and shared dependencies. The index itself and the packer's
  empty ownership marker are excluded.
- `assets`: entries with `id`, `path`, `sha256` and `bytes`.

`assetref.ID` is a string containing 16 lowercase hex characters (64 bits).
The ID algorithm is the first eight bytes of
`SHA256("earth-two:asset:v1:" + slashSeparatedAssetRootRelativePath)`.
For cross-language test vectors, `characters/man.glb` maps to
`4f2067f31e5850c5` and `characters/woman.glb` to `c66dcad0bd759da8`.
Logical IDs stay stable when content changes at the same path. Renames
create new IDs. The packer rejects duplicate/colliding IDs.

In the connection handshake send the catalogue version and `packHash`,
alongside the network protocol/game version. Require matching asset packs
before creating remote players; a mismatch calls for a compatible release
or the normal updater, never asset bytes attached to gameplay messages.
This does not substitute for gameplay/protocol compatibility or player
identity verification. The fingerprint describes build output; it is not
a runtime audit of modified files on disk.

Go API:

```go
index, err := assetref.Load(assetRoot)
// Handle err: the catalogue is required for a release multiplayer session.
err = index.CheckPeer(peerPackHash)
entry, err := index.Resolve(receivedAssetID) // returns only a known local path
id, err := index.ID("characters/man.glb")
```

Unknown references fail; they must not become arbitrary asset paths or
remote asset downloads. Source-only `make run` does not generate the index;
use the staged release pack for compatibility testing.

## Player appearances

`character.Appearance` has these JSON fields:

```json
{
  "body": "<body model asset ID>",
  "tone": "<skin texture asset ID>",
  "wear": ["", "", "", "", "<top ID>", "", "", "", ""]
}
```

The fixed nine wear slots are, in order: face, hair, glasses, mask, top,
bottom, outfit, coat, shoes. Empty means nothing worn. Body, tone and
clothing IDs are derived from paths in `characters/wardrobe.json` and
assigned by the wardrobe loader. Body identity is retained in each
`BodyWardrobe.AssetID`; `Tone.AssetID` and `Item.AssetID` identify textures
and clothing models.

`wardrobe.Appearance(localOutfit)` produces the reference-only value;
`wardrobe.Outfit(receivedAppearance)` maps it to local wardrobe indices.
Resolution rejects unknown bodies, tones, wrong-body/wrong-slot items and
an outfit combined with a top or bottom. Local render handles and array
indices are never valid peer references. Local array order may differ;
IDs still select the intended assets.

Send appearance on player join and on appearance changes. Subsequent
movement/action messages need only player identity, pose/coordinates,
actions and timing. Render and animate other players from installed local
models. The representation contains no asset bytes or filesystem paths.

## Integration ownership

The multiplayer session should choose its server field names, persist or
replicate appearance IDs, enforce the compatible pack/protocol handshake,
and call the Go conversion helpers when creating/updating remote players.
It may use the same ID derivation in TypeScript/Rust, or read the generated
index instead of duplicating the algorithm. The server should validate
appearance selections against the catalogue/wardrobe for its supported
release rather than trusting client-provided IDs.

Please record multiplayer schema decisions or integration constraints in
`docs/server-asset-handoff.md` in the server worktree so this session can
adapt the asset-side APIs before the branches are combined.
