"""The Hull test block: a stretch of market street on the middle decks, laid
out from the kit to try every traversal move. Vault the crates and the
stall counters, mantle the tall crate, slide under the duct, wall-kick up
the east corridor, climb the ladder to the catwalk and come down the
stairs.

The street runs along X; the catwalk and the stalls are along the north
wall (+Y), and the south side is open, towards the camera. Positions are
in Blender's frame (Z up), in metres; turns are quarter turns about the
vertical, anticlockwise from above, so a piece turned once faces +X.

  y  6  ===wall==port==wall==...=====  | |
        [catwalk 3.6 m, ladder at x -5, stairs up at x 7]
        stalls, kiosks, ribs under it    | |  east corridor,
     0  door  crates  duct  crates       | |  walls 2 m apart
    -6  board  tarp        lamps         | |
       x -8                        8    10
"""
from kit import DECK, SLAB

# (piece, x, y, z, turns)
Layout = list[tuple[str, float, float, float, int]]


def layout() -> Layout:
    out: Layout = []

    def put(piece, x, y, z=0.0, turns=0):
        out.append((piece, x, y, z, turns))

    # The deck: the street, and the corridor east of it.
    for x in range(-7, 10, 2):
        for y in range(-5, 6, 2):
            put("deck_floor", x, y)
    # The north wall, the west wall with its door, and the corridor's walls.
    for i, x in enumerate(range(-7, 10, 2)):
        put("hull_wall_port" if i % 3 == 1 else "hull_wall", x, 6)
    for y in range(-5, 6, 2):
        put("bulkhead_door" if y == 1 else "hull_wall", -8, y, turns=1)
        put("hull_wall", 8, y, turns=1)
        put("hull_wall", 10, y, turns=1)
    # The catwalk along the north wall, a deck up, railed but for where the
    # ladder and the stairs come up, on ribs.
    y_walk = 6 - 0.15 - 0.6
    front = y_walk - 0.6
    for x in range(-5, 8, 2):
        put("catwalk", x, y_walk)
        if x not in (-5, 7):
            put("railing", x, front + 0.1, DECK)
    for x in (-6, 0, 4):
        put("rib_pillar", x, front + 0.2)
    put("ladder", -5, front - 0.28)
    # Stairs up to the catwalk's east end, rising north.
    put("stairs", 7, front - 3)
    # Under the catwalk: the market.
    put("market_stall", -3, y_walk - 0.4)
    put("market_stall", 2, y_walk - 0.4)
    put("terminal_kiosk", -1.1, 5.3)
    put("terminal_kiosk", 0.7, 5.3)
    put("rebreather", 2.4, y_walk - 1.1, 0.9)
    for x in (-4.5, 2.6):
        put("status_light", x, 5.85, 2.9)
    for y in (-5, -3):
        put("pipe_run", 7.85, y, turns=-1)
    # The street: crates and a drum to vault, a tall crate to mantle, and a
    # duct across a gap between two tall crates to slide under.
    put("crate", 1, -1)
    put("filter_cartridge", 1.3, -1, 0.9)
    put("crate", 3.5, -1.8, turns=1)
    put("drum", -0.8, -2.2)
    put("crate_tall", 5, -3)
    put("crate", 5, -4.4)
    put("crate_tall", -4.45, 1, turns=1)
    put("crate_tall", -1.55, 1, turns=1)
    put("low_duct", -3, 1)
    # The Exchange's board by the west wall, a tarp over some drums, lamps.
    put("exchange_board", -7.6, -3, turns=1)
    put("tarp_awning", -3.5, -4.6)
    put("drum", -4.4, -4.4)
    put("drum", -3.7, -4.9)
    put("work_lamp", -6, -5.2)
    put("work_lamp", 6, -5.3)
    return out
