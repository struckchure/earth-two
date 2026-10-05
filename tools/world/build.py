"""Builds the world's pieces into assets/world: a .glb for each piece,
world.json with their colliders and ladders, and the layouts in LAYOUTS
(hull_block.json, the Hull test block, is where the game starts).

    make world
    make world-layouts    # only the layouts, against the pieces built
    make world-fast       # unfinished: no bake, seconds

or, in Blender: blender -b --factory-startup --python tools/world/build.py
It builds with the choices in tools/world/style.json (scenes/world.py saves
them from the Shape Lab).

The pieces come from the modules in MODULES, one for each part of the world
(see docs/settlement.md). Each has PIECES, its builders, and CATEGORY, the
name it's shown under."""
import importlib
import json
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import bpy  # noqa: E402

from kit import Style, export, game_vec  # noqa: E402

ROOT = HERE.parent.parent
OUT = ROOT / "assets" / "world"
STYLE = HERE / "style.json"

# The parts of the world, as the Shape Lab lists them.
MODULES = [
    "hull_kit",     # the Hull's middle decks: deck, walls, catwalks, stairs
    "props",        # the Hull's market: crates, stalls, kiosks, the board
    "hull_decks",   # the lower decks' air plant, the Stacks' rooms, warehouses
    "exchange",     # the Exchange floor in the old cargo bay
    "charter_row",  # the Charter Families' clean white street
    "pads",         # landing pads turned freight yards
    "domes",        # the dome line and the South gate
    "fringe",       # outside the domes: farms, salvage, wind, wrecks
    "items",        # what's carried: money, paper, food, parts, goods, tools
    "weapons",      # guns, lasers, blades, batons, armour
    "vehicles",     # haulers, bikes and the ships
    "second_light", # the grounded colony ship's hull round the Hull
    "hull_levels",  # ceilings, hatches, stairwells and lifts between decks
    "ground",       # roads, tracks, paving, lawns, fields, pad markings
    "dressing",     # signs, marking panels, rugs, banners, bunting
]


def modules() -> list:
    """The modules there are (one being written may not exist yet)."""
    out = []
    for name in MODULES:
        try:
            out.append(importlib.import_module(name))
        except ModuleNotFoundError as e:
            if e.name != name:
                raise
    return out


def builders() -> list[tuple[str, object]]:
    """Every piece's builder, with the category it's shown under."""
    return [(m.CATEGORY, make) for m in modules() for make in m.PIECES]


def build_pieces(style: Style, collections: dict[str, bpy.types.Collection] | None = None, want=None,
                 finished: bool = False) -> list:
    """The pieces as objects, each with its world.json entry: every piece,
    or those want(category, name) is true of. collections, by category, is
    where each goes. finished gives them the detailed finish (finish.py):
    what the game gets, a bake a piece."""
    if finished:
        import finish
        finish.prepare()
    built = []
    names = set()
    for category, make in builders():
        p = make(style)
        if p.name in names:
            raise ValueError(f"two pieces are called {p.name}")
        names.add(p.name)
        if want and not want(category, p.name):
            p.bm.free()
            continue
        p.category = category
        entry = p.entry(f"world/{p.name}.glb")
        obj = p.build((collections or {}).get(category))
        if obj.get("source"):
            entry["sources"] = [s.strip() for s in obj["source"].split(",")]
        if finished:
            finish.finish(obj, category, p.kind)
            print(f"world: {p.name} finished", flush=True)
        built.append((p.name, obj, entry))
    return built


def dumps(data) -> str:
    """JSON with an entry per line, its vectors kept on one."""
    import re
    text = json.dumps(data, indent=1)
    return re.sub(r"\[\s+([^\[\]{}]*?)\s+\]", lambda m: "[" + ", ".join(v.strip() for v in m.group(1).split(",")) + "]", text) + "\n"


# The layouts: modules with a layout() of (piece, x, y, z, turns), in
# Blender's frame, each written to assets/world/<module>.json for the game
# to place (world.Kit.SpawnLayout). One being written may not exist yet.
LAYOUTS = ["hull_block", "landfall"]


def write_layouts(out: Path = OUT) -> None:
    """Every layout, checked against the pieces in world.json."""
    pieces = json.loads((out / "world.json").read_text())["pieces"]
    for name in LAYOUTS:
        try:
            module = importlib.import_module(name)
        except ModuleNotFoundError as e:
            if e.name != name:
                raise
            continue
        placed = []
        for piece, x, y, z, turns in module.layout():
            if piece not in pieces:
                raise ValueError(f"{name} places {piece}, which isn't a piece")
            placed.append({"piece": piece, "at": game_vec((x, y, z)), "turns": turns % 4})
        (out / f"{name}.json").write_text(dumps({"pieces": placed}))


def write(built: list, out: Path = OUT) -> None:
    """The .glb files, world.json and the layouts."""
    out.mkdir(parents=True, exist_ok=True)
    manifest = {}
    for name, obj, entry in built:
        export(obj, out / f"{name}.glb")
        manifest[name] = entry
    (out / "world.json").write_text(dumps({"pieces": manifest}))
    write_layouts(out)


def place(obj: bpy.types.Object, x: float, y: float, z: float, turns: int) -> None:
    obj.location = (x, y, z)
    obj.rotation_euler = (0, 0, turns * math.pi / 2)


def _arg(name: str) -> str | None:
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else sys.argv[1:]
    return args[args.index(name) + 1] if name in args else None


def build_module(module: str, parts: Path, threads: int) -> None:
    """One module's pieces, finished, into OUT, and its share of world.json
    into parts: what each of make world's processes does."""
    import finish
    finish.THREADS = threads
    m = importlib.import_module(module)
    built = build_pieces(Style.load(STYLE), want=lambda c, n: c == m.CATEGORY, finished=True)
    for name, obj, entry in built:
        export(obj, OUT / f"{name}.glb")
    (parts / f"{module}.json").write_text(json.dumps({name: entry for name, _, entry in built}))


def build_all(jobs: int) -> None:
    """Every module in its own process, jobs at a time (each piece's bake
    is Cycles on the CPU), then world.json merged from them in MODULES order,
    models no piece is any longer removed, and the layouts written."""
    import os
    import subprocess
    import tempfile
    import time
    OUT.mkdir(parents=True, exist_ok=True)
    parts = Path(tempfile.mkdtemp())
    names = [m.__name__ for m in modules()]
    threads = max(1, (os.cpu_count() or 4) // jobs)
    blender = bpy.app.binary_path
    if blender and Path(blender).name.lower().startswith("blender"):
        command = [blender, "--background", "--factory-startup", "--python-exit-code", "1", "--python", __file__, "--"]
    else:  # bpy as a Python module
        command = [sys.executable, __file__]
    running, failed = {}, []
    queue = list(names)
    start = time.time()
    while queue or running:
        while queue and len(running) < jobs:
            name = queue.pop(0)
            log = open(parts / f"{name}.log", "w")
            running[name] = (subprocess.Popen(command + ["--module", name, "--parts", str(parts), "--threads", str(threads)],
                                              stdout=log, stderr=subprocess.STDOUT), log)
        time.sleep(0.5)
        for name, (proc, log) in list(running.items()):
            if proc.poll() is not None:
                log.close()
                del running[name]
                if proc.returncode:
                    failed.append(name)
                    print((parts / f"{name}.log").read_text()[-3000:])
                print(f"world: {name} {'failed' if proc.returncode else 'done'} ({time.time() - start:.0f} s)", flush=True)
    if failed:
        raise SystemExit(f"world: {', '.join(failed)} failed")
    manifest = {}
    for name in names:
        for piece, entry in json.loads((parts / f"{name}.json").read_text()).items():
            if piece in manifest:
                raise ValueError(f"two pieces are called {piece}")
            manifest[piece] = entry
    (OUT / "world.json").write_text(dumps({"pieces": manifest}))
    for old in OUT.glob("*.glb"):
        if old.stem not in manifest:
            old.unlink()
    write_layouts(OUT)
    import polyhaven
    used = {s for e in manifest.values() for s in e.get("sources", [])}
    (OUT / "CREDITS.txt").write_text(polyhaven.credits(used))
    print(f"world: {len(manifest)} pieces in {OUT} ({time.time() - start:.0f} s)")


def main() -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    s = bpy.context.scene
    s.unit_settings.system = "METRIC"
    s.unit_settings.scale_length = 1.0
    if _arg("--module"):
        build_module(_arg("--module"), Path(_arg("--parts")), int(_arg("--threads") or 0))
        return
    if "--fast" in sys.argv:
        # Unfinished: the pieces as modelled, no bake (seconds, for trying
        # a layout; not what the game should ship).
        write(build_pieces(Style.load(STYLE)))
        print(f"world: {len(builders())} pieces, unfinished, in {OUT}")
        return
    if "--layouts" in sys.argv:
        # Only the layouts, against the pieces already built.
        write_layouts()
        print(f"world: layouts in {OUT}")
        return
    import os
    build_all(int(_arg("--jobs") or max(1, min(len(MODULES), (os.cpu_count() or 4) // 3))))


if __name__ == "__main__":
    main()
