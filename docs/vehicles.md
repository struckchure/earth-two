# Vehicles

How the vehicles look and where they come from. They're near-future
sci-fi: real proportions and real running gear, with invented bodywork.
Everything is then made the Red's: repainted in the colony's colours, worn
and dusty, and given what the story needs. The Receivership's shuttle is
the exception. It stays clean, because it's new and it doesn't belong here
(see [Look and feel](look-and-feel.md)).

## Where they come from

Most are built on CC-BY models from Sketchfab, credited in
`assets/world/CREDITS.txt`, and retrofitted in `tools/world/vehicles.py`.
The trike is modelled from scratch, after Death Stranding's Reverse Trike.

| piece | built on | what we changed |
| --- | --- | --- |
| `buggy` | ["Sci - Fi Buggy"](https://sketchfab.com/3d-models/sci-fi-buggy-085bd97876a64eacbc041299ac945e9c), TiyaMakes | Body repainted and trim in hazard yellow. A rack over the drive with water cans and a crate strapped on, and a whip aerial with a pennant. |
| `bike` | ["Cyberpunk Bike Concept Design"](https://sketchfab.com/3d-models/cyberpunk-bike-concept-design-99290272e69d4cd8a56af2d2e1274e0d), Berk Gedik | A courier's bag on the tail. |
| `hauler` | ["H500 - Explorer Truck"](https://sketchfab.com/3d-models/h500-explorer-truck-89ea8e69b87942a881f9b89fd04ec00f), Render Blue | Cargo body repainted, lids in a second paint, bumper in hazard yellow. A roof rack of crates under a tarp. |
| `hauler_tanker` | the same truck | The same repaint, with a banded water tank on the rack, a manhole, a hose reel and valves. |
| `rover` | [Martian Rover "THOTH"](https://sketchfab.com/3d-models/martian-rover-thoth-e76d2a9172b54a62b89c9f18710d9cbc), Maxim Senchenko | Turned from a robot into a crew hab: repainted, portholes down its flanks, an airlock door, steps and handrails at the back, aerials. Its display sphere is taken off. |
| `drifter_patience` | ["Transporter Spaceship"](https://sketchfab.com/3d-models/transporter-spaceship-fe581b60c8ea4ea591a24a483f25ddad), pirate8888 | Hull repainted grey and armour Crew orange. Guns and pilot taken off. Turned so its hold faces the town. Its level ramp is replaced with one you can walk down, there's cargo and the registration in the hold, and colliders for the hold, hull and feet. |
| `receivership_shuttle` | ["Sci-fi shuttle"](https://sketchfab.com/3d-models/sci-fi-shuttle-d3e5d9303d1d4e8b8a8e30bf22f59a21), Hrvoje Wächter | Repainted Corvane white and kept clean. Its interior is taken off. Stood on five landing legs with blue feet, with a ramp lowered from the belly at the back. |

## How a sourced vehicle is made

A `Sourced` piece names its model as `sketchfab:<uid>`, and `sketchfab.py`
downloads it once into `build/sketchfab/`. That needs `SKETCHFAB_API_KEY`
in the repo's `.env`, which git ignores. Only CC0 and CC-BY models are
accepted; non-commercial ones are refused. The piece then:

- **fits** the model by one size, and turns it so its nose faces −Y;
- **drops** parts by name (`drop=("gunsteel", "hair")`);
- **repaints** some of its materials in palette colours, keeping their wear
  (`paint={"main_body": "Crew orange"}`);
- **adds** parts with kit's helpers (racks, loads, ramps, legs) and the
  colliders.

The finish decimates it to its budget and bakes it onto one texture like
every other piece, so sourced and modelled pieces look of a piece. A
Sketchfab material only glows in the game if its emissive map is mostly
lit, as a lamp's is. A hull with a few lit windows doesn't glow.

## Choosing more

- Avoid fan recreations of other studios' designs (a Death Stranding
  trike, an Expanse dropship, the Nostromo). The uploader's licence doesn't
  clear the original design.
- Big wheels and high clearance suit the Red. A model whose wheels are
  under a quarter of its length across reads as a car, not an off-roader.
- Check `docs/look-and-feel.md` before choosing colours. A model's own
  palette is usually replaced.

## Driving

All but the two ships can be driven (see the README's Vehicles section).
How each kind handles is in `vehicle/handling.go`:

| kind | pieces | what it's like |
| --- | --- | --- |
| `buggy` | buggy | Light four-wheel drive on long travel: lively and forgiving. |
| `trike` | trike | Two steering wheels in front, one driven behind; narrow, so it's kept from rolling over. |
| `bike` | bike | Leans into turns and holds itself up (Jolt's motorcycle controller). |
| `rover` | rover | Six wheels, heavy and low-geared; the back pair steers against the front to turn tighter. |
| `truck` | hauler, hauler_tanker | Eight driven wheels, the front two axles steering. |

The engines are geared for about half a g at the wheels in first gear,
and each kind is limited to a top speed. Each vehicle's weight sits as low
as its axles unless `drive(com=...)` says otherwise.
