"""Landfall, the first city (docs/settlement.md), laid out from the built
pieces: the Hull in the middle with its decks, Charter Row behind a fence to
the north, the Pads to the east with the drifter on its pad, the dome line
round all of it with the South gate, and outside, the Fringe: the caravan
road, the farms to the west, the salvage fields to the east, a Fringer camp
and the wind farm.

Positions are in Blender's frame (Z up), in metres; turns are quarter turns
about the vertical, anticlockwise from above, so a piece turned once faces
+X (see hull_block.py). Every piece's front faces -Y at no turns.

    y  64  =============== dome line ===============
           |      Charter Row (fenced, y 20..52)    |
       12  |  +----------- the Hull -----------+    |
           |  | lower | market  | the Exchange |  the Pads
           |  | decks | (block) |              |  (x 14..62)
      -20  |  +--------------------------------+    |
           |            gate checks                 |
      -56  ==================#gate#==================
                             |
      -90   farms ----------road--------- salvage fields
         x -65               0                       65

The Hull's middle deck is the Hull test block (hull_block.py), moved into
the city, so every traversal move is in it; the Stacks are a deck up, on
catwalk sections (floors don't collide, catwalks do). The Fringe's rocks
are scattered from a fixed seed, so the layout comes out the same each
build.
"""
import random

import hull_block
from kit import DECK

Layout = list[tuple[str, float, float, float, int]]

# The dome line: a rectangle of dome_wall sections 4 m wide, close round
# the city. The South gate's opening is in its south side, on the road
# from the Hull.
DOME_W, DOME_E = -48, 64
DOME_S, DOME_N = -44, 56
# Its sides are whole numbers of sections, so neighbours meet exactly:
# overlapping glass in one plane flickers.
GATE_X = 0
# How far either side of the gate's middle the dome line stops: at the
# outer edges of its pillars (±4), inside its 9.9 m frame, so there's no way
# round it.
GATE_HALF = 4

# The Hull: its outer walls, and the walls inside between its parts.
HULL_W, HULL_E, HULL_S, HULL_N = -40, 8, -20, 12
LOWER_DECKS = -28      # the wall between the lower decks and the market
EXCHANGE = -4          # the wall between the market and the Exchange
# The test block, moved to the market: its middle is here.
BLOCK = (-16, -6)
# The Stacks: catwalk sections a deck up over the north of the market.
STACKS_X = range(-25, -4, 2)
STACKS_Y0, STACKS_ROWS = 0.75, 9

# Charter Row, behind its fence; the Pads; the road outside.
CHARTER_S, CHARTER_N, CHARTER_W, CHARTER_E = 20, 52, -30, 22
PADS_W, PADS_E, PADS_S, PADS_N = 14, 62, -40, 20
ROAD_Y = -90
FARMS = (-150, -100)   # x from, to
SALVAGE = (100, 150)

# Where the player starts: on the Pads by the drifter's ramp, where new
# players arrive (docs/settlement.md). Blender's frame, like the rest.
SPAWN = (44, -30, 0)


def layout() -> Layout:
    out: Layout = []

    def put(piece, x, y, z=0.0, turns=0):
        out.append((piece, float(x), float(y), float(z), turns))

    _dome(put)
    _gate(put)
    _hull(put, out)
    _stacks(put)
    _exchange(put)
    _lower_decks(put)
    _charter_row(put)
    _pads(put)
    _fringe(put)
    return out


def _wall_run(put, piece, xs, y, z=0.0, turns=0, skip=(), doors=None):
    """A row of 2 m walls along X at y, centred on xs; doors maps a centre
    to the piece to put there instead."""
    for x in xs:
        if x in skip:
            continue
        put((doors or {}).get(x, piece), x, y, z, turns)


def _spans(a, b):
    """The middles of 4 m sections from a to b, which must be a whole
    number of them apart."""
    n, rest = divmod(round(b - a), 4)
    assert rest == 0 and n > 0, f"{a}..{b} isn't a whole number of 4 m sections"
    return [a + 4 * i + 2 for i in range(n)]


def _dome(put):
    # The south side either side of the gate's opening, and the rest.
    south = _spans(DOME_W, GATE_X - GATE_HALF) + _spans(GATE_X + GATE_HALF, DOME_E)
    for x in south:
        put("dome_wall", x, DOME_S, turns=0)       # front out, to the south
    for x in _spans(DOME_W, DOME_E):
        put("dome_wall", x, DOME_N, turns=2)       # front out, to the north
    for y in _spans(DOME_S, DOME_N):
        put("dome_wall", DOME_E, y, turns=1)       # front out, to the east
        put("dome_wall", DOME_W, y, turns=3)       # front out, to the west
    # The struts the glass hangs from, and frames up from it every 16 m (it
    # leans in 1.3 m by its top), and over the gate.
    for x in range(DOME_W + 9, DOME_E - 4, 16):
        if abs(x - GATE_X) > 8:
            put("dome_strut_anchor", x, DOME_S + 2.5)
            put("dome_frame", x, DOME_S + 1.3, 5.0)
        put("dome_strut_anchor", x, DOME_N - 2.5)
        put("dome_frame", x, DOME_N - 1.3, 5.0, 2)
    for y in range(DOME_S + 9, DOME_N - 4, 16):
        put("dome_strut_anchor", DOME_E - 2.5, y)
        put("dome_strut_anchor", DOME_W + 2.5, y)
        put("dome_frame", DOME_E - 1.3, y, 5.0, 1)
        put("dome_frame", DOME_W + 1.3, y, 5.0, 3)
    put("dome_frame", GATE_X, DOME_S, 6.6)


def _gate(put):
    put("south_gate", GATE_X, DOME_S)
    # The gate checks, inside: a booth, barriers across both lanes, scanners.
    put("checkpoint_booth", GATE_X + 8, DOME_S + 7, turns=3)
    put("boom_barrier", GATE_X - 4.6, DOME_S + 9)
    put("boom_barrier", GATE_X + 0.4, DOME_S + 9)
    put("scanner_arch", GATE_X - 2.6, DOME_S + 14)
    put("scanner_arch", GATE_X + 2.6, DOME_S + 14)
    for x in (GATE_X - 9.4, GATE_X - 10.6):
        put("mask_station", x, DOME_S + 2.2, turns=2)
    put("weapon_rack", GATE_X + 10.4, DOME_S + 4, turns=3)
    put("stun_baton", GATE_X + 7, DOME_S + 7.8, 1.0)
    put("floodlight_tower", GATE_X - 9, DOME_S + 12)
    put("floodlight_tower", GATE_X + 9, DOME_S + 12)
    for x in (-7, -5, 5, 7):
        put("concrete_barrier", GATE_X + x, DOME_S + 18, turns=1)
    put("signpost", GATE_X + 6, DOME_S + 17)
    put("status_light", GATE_X + 5.2, DOME_S + 0.5, 4.5, 2)


def _hull(put, out):
    xs = range(HULL_W + 1, HULL_E, 2)
    ys = range(HULL_S + 1, HULL_N, 2)
    # The deck, under all of it (the test block's own is left out below).
    bx, by = BLOCK
    for x in xs:
        for y in ys:
            put("deck_floor", x, y)
    # Outer walls, two decks high, the lower one with its doors.
    south_doors = {-33: "roller_door", -15: "bulkhead_door", 1: "airlock_door", 3: "airlock_door"}
    north_doors = {-1: "bulkhead_door"}
    _wall_run(put, "hull_wall", xs, HULL_S, doors=south_doors)
    _wall_run(put, "hull_wall_port", xs, HULL_N, doors=north_doors)
    _wall_run(put, "hull_wall", xs, HULL_S, DECK)
    _wall_run(put, "hull_wall_port", xs, HULL_N, DECK)
    for y in ys:
        put("bulkhead_door" if y == -7 else "hull_wall", HULL_W, y, turns=1)
        put("bulkhead_door" if y in (-5, -3) else "hull_wall", HULL_E, y, turns=1)
        put("hull_wall", HULL_W, y, DECK, 1)
        put("hull_wall_port" if y % 6 == 1 else "hull_wall", HULL_E, y, DECK, 1)
    # Ribs along the outside, every 6 m.
    for x in range(HULL_W + 4, HULL_E, 6):
        put("rib_pillar", x, HULL_S - 0.4)
        put("rib_pillar", x, HULL_S - 0.4, DECK)
    # The walls inside: lower decks | market, market | Exchange.
    for y in ys:
        if y not in (-17, 5):
            put("hull_wall", LOWER_DECKS, y, turns=1)
        if y not in (-15, -7):
            put("hull_wall", EXCHANGE, y, turns=1)
    # The market's middle: the test block, but for its deck.
    for piece, x, y, z, turns in hull_block.layout():
        if piece != "deck_floor":
            out.append((piece, x + bx, y + by, z, turns))
    # The market's south street, between the block and the outer wall:
    # food and parts stalls, facing north.
    for x in (-25, -22, -19):
        put("food_stall", x, HULL_S + 2, turns=2)
    for x in (-11, -8):
        put("parts_stall", x, HULL_S + 2, turns=2)
    for x, y, piece in ((-25, -17.2, "produce_crate"), (-22, -17.2, "protein_pack"),
                        (-19, -17.2, "coffee_tin"), (-11, -17.2, "valve"), (-8, -17.2, "electronics_box")):
        put(piece, x, y, 0.9)
    for x in (-24, -20):
        put("stool", x, HULL_S + 4.4)
    put("folding_table", -21, HULL_S + 5.5)
    put("cooker", -21, HULL_S + 5.5, 0.8)
    put("tarp_awning", -22, HULL_S + 2.2, 0.4, 2)
    put("work_lamp", -27, HULL_S + 5)
    put("status_light", -15, HULL_S + 0.2, 2.9, 2)
    # Under the Stacks, north of the block: the Crew's workshop.
    for x in (-25, -21):
        put("storage_rack", x, HULL_N - 0.8)
    for x in (-17.75, -17.2, -16.65):
        put("locker", x, HULL_N - 0.4)
    put("parts_stall", -12, HULL_N - 1.4)
    put("toolbox", -12, HULL_N - 1.4, 0.95)
    put("pallet", -9, 8)
    put("grain_sack", -9, 8, 0.15)
    put("weapon_rack", -6, HULL_N - 0.4)
    put("junction_box", -14, HULL_N - 0.1, 1.5, 0)
    # A bike and a trike by the market's south door, outside.
    put("bike", -18, HULL_S - 3, turns=1)
    put("trike", -12, HULL_S - 3.5, turns=1)


def _stacks(put):
    """The Stacks: a deck of catwalk sections over the north of the market,
    railed along its open edge, with rooms of curtains, bunks and lockers,
    up the block's ladder or stairs of their own."""
    rows = [STACKS_Y0 + 1.2 * i for i in range(STACKS_ROWS)]
    for x in STACKS_X:
        for y in rows:
            put("catwalk", x, y)  # 1.2 m deep, so the rows meet
    # Ribs under it, and railings along its south edge, where it looks
    # down on the block's catwalk.
    for x in range(-24, -4, 4):
        for y in (4, 8):
            put("rib_pillar", x, y)
    # Stairs up from the market's east end, rising north onto it.
    put("stairs", -5, STACKS_Y0 - 0.6 - 3.06)
    for x in STACKS_X:
        if x != -5:
            put("railing", x, rows[0] - 0.5, DECK, 2)
    # Rooms along the north wall, 4 m each, curtained between and in front.
    up = DECK
    for i, x0 in enumerate(range(-26, -6, 4)):
        for y in (8.95, 11.1):
            put("curtain_partition", x0, y, up, 1)
        put("curtain_partition", x0 + 1, 8.1, up)
        put("bunk_bed", x0 + 2.2, HULL_N - 0.7, up, 2)
        put("locker", x0 + 3.5, 9.6, up, 3)
        put("stool" if i % 2 else "cot", x0 + 1.4, 9.4, up)
    put("curtain_partition", -6, 8.95, up, 1)
    put("curtain_partition", -6, 11.1, up, 1)
    # A shared kitchen and a laundry line over the walkway.
    put("folding_table", -6.6, 6, up)
    put("cooker", -6.6, 6, up + 0.8)
    put("stool", -7.6, 6, up)
    put("water_canister", -6.2, 6.1, up + 0.8)
    put("laundry_line", -16, 6.4, up)
    put("laundry_line", -11, 6.4, up)
    put("work_lamp", -20, 5, up)
    put("status_light", -24, HULL_N - 0.1, up + 2.6, 0)


def _exchange(put):
    """The Exchange floor, in the old cargo bay at the Hull's east end:
    counters across it, the Registrars' desks and archive behind, queues and
    benches in front, the floor bell and the arbitration tables."""
    x0, x1 = EXCHANGE, HULL_E
    mid = (x0 + x1) / 2
    for x in (mid - 2, mid + 2):
        put("registrar_counter", x, 4)
        put("mark_stack", x - 1, 4, 1.8)
        put("contract_paper", x + 1, 4, 1.8)
    for x in (mid - 3, mid, mid + 3):
        put("registrar_desk", x, 6.5, turns=2)
        put("office_chair", x, 7.3)
        put("ledger_book", x, 6.5, 0.8)
    put("stamp_station", mid + 4.5, 6)
    put("registrar_stamp", mid + 4.5, 6, 1.1)
    for x in range(x0 + 1, x1, 2):
        put("archive_shelves", x, HULL_N - 0.5)
    for y in (7, 9):
        put("filing_cabinets", x1 - 0.5, y, turns=3)
    put("sealed_filing", x1 - 0.5, 7, 2.0)
    put("registrar_seal_emblem", mid, HULL_N - 0.2, 2.6)
    # The queues to the counters, the benches and terminals in front.
    for x in (mid - 3, mid - 1, mid + 1, mid + 3):
        put("queue_rail", x, 1.5, turns=1)
        put("queue_rail", x, -0.5, turns=1)
    for x in (mid - 3, mid, mid + 3):
        put("waiting_bench", x, -6, turns=2)
    put("terminal_row", mid - 2, -11)
    put("terminal_row", mid + 2.5, -11)
    put("terminal_kiosk", x0 + 1, -15)
    put("exchange_board", x1 - 0.2, -8, turns=3)
    put("floor_bell", mid + 3.5, -3)
    put("arbitration_table", mid - 3.5, -15.5)
    for x in (mid - 4.4, mid - 2.6):
        put("office_chair", x, -16.6)
    put("land_claim", mid - 3.5, -15.5, 0.9)
    for x in (mid - 3, mid + 3):
        for y in (-12, -4, 4, 10):
            put("pendant_lamp", x, y, DECK - 3.6)


def _lower_decks(put):
    """The lower decks, at the Hull's west end: the air plant the Crew run,
    and a warehouse through the roller door."""
    x0 = HULL_W
    for y in (-2, 2, 6, 10):
        put("air_scrubber", x0 + 1.2, y, turns=1)
    for y in (0, 8):
        put("air_fan", x0 + 5, y, turns=1)
    put("generator", x0 + 9, 9)
    put("generator", x0 + 9, 6.5)
    put("control_console", x0 + 6, 3.5, turns=3)
    put("water_tank", x0 + 9.5, 0.5)
    put("water_tank", x0 + 9.5, -2)
    for y in (-4.2, -6.2):
        put("pump_unit", x0 + 6, y, turns=1)
    put("valve_station", x0 + 0.2, -5, 0, 1)
    for x in (x0 + 3, x0 + 7):
        put("junction_box", x, HULL_N - 0.1, 1.5)
    for x in range(x0 + 3, LOWER_DECKS - 1, 2):
        put("power_conduit", x, 4.5)
    for x in (x0 + 4, x0 + 8):
        put("hazard_barrier", x, -8.5)
    put("status_light", x0 + 6, HULL_N - 0.1, 2.9)
    put("work_lamp", x0 + 3, -8)
    # The warehouse, along the south wall.
    for x in (x0 + 2, x0 + 4.5, x0 + 10):
        put("storage_rack", x, HULL_S + 0.8, turns=2)
    for x, y in ((x0 + 3, -15), (x0 + 6, -15), (x0 + 9, -14)):
        put("pallet", x, y)
        put("produce_crate", x, y, 0.15)
    put("grain_sack", x0 + 6, -15, 0.15)
    put("crate", x0 + 3, -12)
    put("crate_tall", x0 + 4.4, -12)
    put("drum", x0 + 9.5, -11.5)


def _charter_room(put, cx, cy, door_south):
    """One of Charter Row's company buildings: 8 m square, white walls with
    windows and a door onto the street, pillars at its corners, and an
    office inside."""
    s = 4
    front_y = cy - s if door_south else cy + s
    back_y = cy + s if door_south else cy - s
    for i, x in enumerate(range(cx - 3, cx + 4, 2)):
        put("charter_door" if i == 1 else "charter_window", x, front_y, turns=0 if door_south else 2)
        put("charter_window" if i in (1, 2) else "charter_wall", x, back_y, turns=2 if door_south else 0)
    for y in range(cy - 3, cy + 4, 2):
        put("charter_window" if y == cy + 1 else "charter_wall", cx - s, y, turns=3)
        put("charter_wall", cx + s, y, turns=1)
    for x in (cx - s, cx + s):
        for y in (cy - s, cy + s):
            put("charter_pillar", x, y)
    # The office: a desk, its chair, shelves, the safe.
    put("office_desk", cx, cy, turns=0 if door_south else 2)
    put("office_chair", cx, cy + (0.9 if door_south else -0.9))
    put("contract_paper", cx - 0.3, cy, 0.8)
    put("wine_bottle", cx + 0.5, cy, 0.8)
    for x in (cx - 2.4, cx - 1.2):
        put("bookshelf", x, back_y + (-0.4 if door_south else 0.4), turns=0 if door_south else 2)
    put("safe", cx + 3.2, back_y + (-0.5 if door_south else 0.5))
    put("potted_tree", cx - 3.2, cy)


def _charter_row(put):
    """Charter Row: the Charter Families' clean white street behind an iron
    fence, its gate guarded; company buildings either side, and a garden
    plaza in the middle with a fountain and glasshouses."""
    w, e, s, n = CHARTER_W, CHARTER_E, CHARTER_S, CHARTER_N
    gate = -1
    for x in range(w + 1, e, 2):
        if x != gate:
            put("iron_fence", x, s)
        put("iron_fence", x, n, turns=2)
    # The gate stands open by day, swung in on its west post (closed, it
    # has no way through).
    put("iron_gate", gate - 1, s + 1, turns=1)
    for y in range(s + 1, n, 2):
        put("iron_fence", w, y, turns=3)
        put("iron_fence", e, y, turns=1)
    put("guard_booth", gate + 3, s + 2)
    put("street_lamp", gate - 2, s + 1)
    put("street_lamp", gate + 2, s + 1)
    street = (s + n) // 2
    for cx in (-20, 12):
        _charter_room(put, cx, street - 8, door_south=False)
        _charter_room(put, cx, street + 8, door_south=True)
    # The street: lamps, benches and planters along it.
    for x in range(w + 4, e - 1, 8):
        put("street_lamp", x, street - 3)
        put("street_lamp", x, street + 3, turns=2)
    for x in (-24, -16, 8, 16):
        put("stone_bench", x, street - 2.6)
        put("planter", x + 2, street - 2.6)
        put("stone_bench", x, street + 2.6, turns=2)
        put("planter", x + 2, street + 2.6)
    # The plaza between the buildings.
    plaza = -4
    put("fountain", plaza, street)
    for x in (plaza - 3, plaza + 3):
        put("glasshouse", x, street - 8)
        put("glasshouse", x, street + 8, turns=2)
    for x in (plaza - 3, plaza - 1, plaza + 1, plaza + 3):
        put("hedge", x, street - 4.5)
        put("hedge", x, street + 4.5)
    for x, y in ((plaza - 2.5, street - 1.8), (plaza + 2.5, street + 1.8)):
        put("stone_bench", x, y)
    for x in (plaza - 4, plaza + 4):
        put("potted_tree", x, street)
    # The road up from the Exchange's north door to the gate.
    for y in (HULL_N + 3, HULL_N + 6):
        put("street_lamp", -3, y)


def _pads(put):
    """The Pads: landing pads turned freight yards east of the Hull, nobody's
    and everybody's. The drifter Patience on its pad with the drifter-day
    market at its ramp, a container yard under a gantry crane, fuel tanks,
    the receivership shuttle, floodlights, and a Quiet Book shack tucked in
    the containers."""
    w, e, s, n = PADS_W, PADS_E, PADS_S, PADS_N
    for x in range(w + 2, e, 4):
        for y in range(s + 2, n, 4):
            put("pad_tile", x, y)
    # The drifter, nose north, its ramp down to the south.
    ship = (44, -2)
    put("drifter_patience", *ship)
    for x, y in ((ship[0] - 12, ship[1] - 22), (ship[0] + 12, ship[1] - 22),
                 (ship[0] - 12, ship[1] + 21), (ship[0] + 12, ship[1] + 21)):
        put("pad_beacon", x, y)
    # Drifter day: tables and nets of goods in front of the ramp.
    for i, x in enumerate((35.8, 39, 42.2, 46.8, 50, 53.2)):
        put("trade_table", x, -32)
        put(("wine_bottle", "seed_case", "medkit", "data_core", "coffee_tin", "power_cell")[i], x, -32, 0.8)
    for x in (37.5, 51.5):
        put("cargo_net_pile", x, -34.5)
    for x in (40, 48):
        put("loader", x, -27, turns=2)
    # The container yard, in two rows under the crane, some stacked.
    for i, y in enumerate(range(s + 18, n - 3, 3)):  # clear of the shuttle
        for col, x in enumerate((w + 5.5, w + 12.5)):
            piece = "container_open" if (i + col) % 5 == 2 else "shipping_container"
            put(piece, x, y)
            if (i + col) % 3 == 0 and piece == "shipping_container":
                put("shipping_container", x, y, 2.6)
    put("gantry_crane", w + 9, -6)
    put("gantry_crane", w + 9, 10)
    put("quiet_book_shack", w + 2, s + 2, turns=1)
    put("forged_seal", w + 2, s + 2, 0.9)
    # Fuel tanks at the south-east corner, the tanker east of the drifter,
    # the shuttle south of the yard, a hauler in the street by the Hull.
    for y in (s + 4, s + 11):
        put("fuel_tank", e - 4, y, turns=1)
    put("hauler_tanker", e - 4, 10)
    put("receivership_shuttle", w + 14, s + 9, turns=1)
    put("hauler", w - 3, s + 10)
    for x, y in ((w + 1, s + 1), (e - 1, s + 1), (w + 1, n - 1), (e - 1, n - 1)):
        put("floodlight_tower", x, y)
    # The edge between the Hull and the Pads: barriers and bollards.
    for y in range(s + 4, n, 6):
        put("concrete_barrier", w - 2, y, turns=1)
        put("bollard", w - 2, y + 3)


def _fringe(put):
    """Outside the domes: the caravan road from the South gate to the farms
    and the salvage fields, the wind farm, a Fringer camp by the road, and
    rocks and dust everywhere else."""
    # The caravan road, marked by signposts and old barriers.
    for y in range(DOME_S - 8, ROAD_Y, -10):
        put("signpost", GATE_X + 7, y, turns=2)
    put("signpost", GATE_X + 6, ROAD_Y + 6, turns=2)
    for x in range(FARMS[1] + 10, SALVAGE[0] - 9, 20):
        if abs(x) > 8:
            put("concrete_barrier", x, ROAD_Y + 6)
    # Caravans waiting at the gate.
    put("caravan_cart", GATE_X - 6, DOME_S - 8)
    put("caravan_cart", GATE_X + 9, DOME_S - 10, turns=1)
    put("hauler", GATE_X - 3, DOME_S - 20)
    _farms(put)
    _salvage(put)
    # The Fringer camp, south of the road.
    cx, cy = 40, ROAD_Y - 14
    for dx, dy, turns in ((-5, 0, 0), (0, -4, 1), (5, 0, 2), (0, 4, 3)):
        put("fringer_tent", cx + dx, cy + dy, turns=turns)
    put("camp_stove", cx, cy)
    put("camp_stove", cx + 1.2, cy + 0.6)
    put("water_tank_fringe", cx + 7, cy + 5)
    put("caravan_cart", cx - 8, cy - 4, turns=1)
    put("windsock", cx + 8, cy - 5)
    put("trike", cx - 4, cy + 7, turns=1)
    put("covered_car", cx + 10, cy + 1, turns=1)
    put("rebreather", cx + 1, cy - 1, 0.4)
    put("water_canister", cx - 1, cy + 1)
    # The wind farm, west of the dome.
    for y in range(-70, 61, 26):
        put("wind_turbine", -100, y)
    _rocks(put)


def _farms(put):
    x0, x1 = FARMS
    for x in range(x0 + 10, x1, 8):
        for y in (ROAD_Y - 14, ROAD_Y - 24, ROAD_Y - 34):
            put("greenhouse", x, y)
    for i in range(10):
        for y in (ROAD_Y - 7.5, ROAD_Y - 9):
            put("crop_bed", x0 + 8 + 4.1 * i, y)
    for x in range(x0 + 8, x1 - 2, 4):
        put("irrigation_pipe", x, ROAD_Y - 10.4)
    for x in (x0 + 4, x1 - 4):
        put("water_tank_fringe", x, ROAD_Y - 18)
    for x in range(x0 + 2, x1, 4):
        put("wire_fence", x, ROAD_Y - 40)
    for y in range(ROAD_Y - 38, ROAD_Y - 6, 4):
        put("wire_fence", x0, y, turns=1)
        put("wire_fence", x1, y, turns=1)
    put("signpost", x1 + 4, ROAD_Y + 4, turns=1)
    put("grain_sack", x1 - 6, ROAD_Y - 18)
    put("produce_crate", x1 - 7, ROAD_Y - 18)
    put("windsock", x0 + 6, ROAD_Y - 4)


def _salvage(put):
    x0, x1 = SALVAGE
    put("wrecked_terraformer", x0 + 12, ROAD_Y - 22)
    for dx, dy in ((-4, -8), (6, -14), (22, -4), (30, -18), (14, -32), (2, -28)):
        put("terraformer_debris", x0 + 10 + dx, ROAD_Y - 22 + dy)
    for i, (dx, dy) in enumerate(((0, -8), (8, -6), (36, -10), (40, -30), (26, -38), (4, -40), (18, -12), (32, -26))):
        put("scrap_pile", x0 + dx, ROAD_Y + dy, turns=i % 4)
    for dx, dy in ((2, -16), (28, -6), (38, -38), (10, -36)):
        put("salvage_frame", x0 + dx, ROAD_Y + dy)
    for dx, dy in ((6, -24), (34, -16), (20, -44)):
        put("filter_cache", x0 + dx, ROAD_Y + dy)
    put("caravan_cart", x0 - 4, ROAD_Y - 6, turns=1)
    put("hauler", x0 - 8, ROAD_Y - 12, turns=3)
    put("terraformer_part", x0 - 4, ROAD_Y - 8)
    put("salvage_scrap", x0 + 1, ROAD_Y - 6)
    put("crowbar", x0 + 1.5, ROAD_Y - 6.4)
    put("covered_car", x0 + 16, ROAD_Y - 44, turns=2)
    put("signpost", x0 - 4, ROAD_Y + 4, turns=3)


def _rocks(put):
    """Rocks, spires, dust, dry brush and trees, dead and quiver, across
    the Fringe, from a fixed seed, kept off the road, the districts and the
    dome."""
    rng = random.Random(7)

    def clear(x, y, r):
        if DOME_W - 8 - r < x < DOME_E + 8 + r and DOME_S - 8 - r < y < DOME_N + 8 + r:
            return False  # the dome, and a margin round it
        if abs(y - ROAD_Y) < 8 + r or (abs(x - GATE_X) < 10 + r and y > ROAD_Y):
            return False  # the road
        if FARMS[0] - 6 < x < FARMS[1] + 6 and ROAD_Y - 46 < y:
            return False
        if SALVAGE[0] - 12 < x < SALVAGE[1] + 6 and ROAD_Y - 50 < y:
            return False
        if 25 < x < 55 and ROAD_Y - 28 < y < ROAD_Y:
            return False  # the camp
        if abs(x + 100) < 8 + r and -78 < y < 68:
            return False  # the wind farm
        return True

    for piece, count, r in (("rock_small", 60, 1), ("rock_large", 34, 3), ("rock_spire", 12, 3), ("dust_mound", 30, 3),
                            ("dry_brush", 50, 1), ("dead_tree", 14, 2), ("quiver_tree", 10, 2)):
        placed = 0
        while placed < count:
            x, y = rng.uniform(-165, 165), rng.uniform(-145, 85)
            if clear(x, y, r):
                put(piece, round(x, 1), round(y, 1), 0, rng.randrange(4))
                placed += 1
