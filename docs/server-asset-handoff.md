# Multiplayer asset schema handoff

The server has read `asset-network-contract.md` from the asset branch. Stable
IDs and reference-only appearances are accepted as the wire contract. This
checkout implements account identity and OTA reducers; world joins and remote
player replication remain the next integration stage. The constraints below
are agreed schema requirements, not a claim that world admission is already live.

## Identity and ownership

Use the player-owned account ID (`e2_` plus a full public-key fingerprint) as
the durable account owner. SpacetimeDB connection identities and connection IDs
are transport credentials, not character or player ownership IDs. A reconnect
or imported key must keep the same character records.

Each player mutation must pass the module's `authorize_account` guard. A proof
binds database identity, transport identity, live connection ID, action, payload,
expiry, and the account's transactionally advanced authorization sequence. Join
and appearance-change payloads must include all compatibility and appearance
fields inside the signed payload; a client cannot change those fields after signing.

`account.display_name` is an editable Unicode label, bounded to 128 printable
characters and deliberately non-unique. It is independent of the character's
immutable creation name and must never be used as a lookup/ownership key.

## Compatibility and admission

The planned `join_world` request contains:

- `protocol_version: u32`
- `game_version: string`
- `catalogue_version: u32` (initially 1)
- `pack_hash: string` (64 lowercase hex characters)
- `appearance: Appearance`
- `proof: bytes`

The server pins a trusted release with these compatibility values. Require the
exact supported protocol, game version, catalogue version and pack hash before
creating/replicating a player entity. A mismatch returns an explicit compatible
release requirement. Account/key management and public OTA metadata remain
available before world admission so a mismatched client can update.

Asset pack fingerprints are compatibility identifiers, not proof that the
client's files are unmodified. OTA promotion does not automatically change the
active world's supported catalogue. Rollout must coordinate the server's
catalogue and client release.

The initial contract assumes one shared catalogue/pack hash across desktop and
web. Keep shared glTF dependencies under `_packed/` in every delivered pack. If
platform-specific catalogues are introduced, coordinate an explicit compatibility
cohort before changing this strict handshake.

## Appearance fields

`Appearance` is a typed record with these names and meanings:

```text
body: string
tone: string
wear: array<string>, exactly nine entries
```

Use the existing `character.Appearance` JSON names unchanged. Nonempty asset
references are exactly 16 lowercase hex characters. `body` and `tone` must be
nonempty known catalogue IDs; wear entries may be empty. Wear slots retain the
fixed order: face, hair, glasses, mask, top, bottom, outfit, coat, shoes.

Server storage/replication keeps IDs, never local wardrobe indices, render
handles, paths, texture pixels or model bytes. Rust can use a `Vec<String>` on
the wire with an explicit length-nine validation; the Go client retains its
fixed slot array.

Before committing an appearance, validate against the supported release's
trusted `asset-index.json` and `characters/wardrobe.json`: body allowed for the
character, tone belongs to that body, each item belongs to that body and slot,
and an outfit cannot coexist with a top or bottom. A generic known-asset check
alone is insufficient. Reject unknown references rather than deriving a path,
fetching a remote file, or substituting an arbitrary model.

The character body and character name remain immutable after character creation,
as the game design requires. Clothing/tone constraints must be applied before
publishing appearance changes; changing the owning account is a separate signed
slot-transfer operation, not a wardrobe update.

## Client integration

When joining or applying an appearance update, convert through
`wardrobe.Appearance(localOutfit)` and `wardrobe.Outfit(receivedAppearance)`.
Resolve any separately received asset ID only with `assetref.Index.Resolve`.
Do not serialize local render handles or array indices.

Send appearance on join/change. Movement and action updates refer to player or
entity IDs plus simulation state and timing. They carry no asset payloads.

Use staged release assets for compatibility checks. Source-only `make run`
can manage account identity offline, but must not silently bypass catalogue
requirements when world joins are added. The server needs the trusted index
and wardrobe metadata from the supported release; this does not require
loading GLBs or textures into the database module.

Cross-language ID vectors to preserve:

- `characters/man.glb` → `4f2067f31e5850c5`
- `characters/woman.glb` → `c66dcad0bd759da8`

No changes to the asset branch's current ID or appearance APIs are required.
