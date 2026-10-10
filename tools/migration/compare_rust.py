#!/usr/bin/env python3
"""Alternate preserved and current native release tours; retain raw A/B evidence."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import struct

from performance import ROOT, run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path, help="preserved release parity_tour executable")
    parser.add_argument("output", type=Path, help="new evidence directory")
    parser.add_argument("--after", type=Path, default=ROOT / "build/rust/release/examples/parity_tour")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--before-cloth", choices=("on", "off"))
    parser.add_argument("--after-cloth", choices=("on", "off"))
    args = parser.parse_args()
    binaries = {"before": args.before.resolve(), "after": args.after.resolve()}
    out = args.output.resolve()
    if out.exists() or args.runs < 1 or any(not p.is_file() for p in binaries.values()):
        parser.error("use existing release binaries, positive runs, and a new output directory")
    views = ROOT / "tools/migration/presentation-views.json"
    expected = [v["name"] for v in json.loads(views.read_text())]
    out.mkdir(parents=True)
    report = {
        "started_utc": datetime.now(timezone.utc).isoformat(),
        "host": platform.platform(),
        "binaries": {k: {"path": str(p), "sha256": hashlib.sha256(p.read_bytes()).hexdigest()}
                     for k, p in binaries.items()},
        "views_sha256": hashlib.sha256(views.read_bytes()).hexdigest(),
        "runs": [],
        "scope": "Native release frame cadence; identical scenes and default render settings; no sampler attached.",
    }
    env = {k: v for k, v in os.environ.items() if not k.startswith("EARTH_TWO_")}
    for repetition in range(args.runs):
        order = ("before", "after") if repetition % 2 == 0 else ("after", "before")
        for variant in order:
            folder = out / f"{variant}-{repetition + 1}"
            folder.mkdir()
            captures = folder / "captures"
            command = [str(binaries[variant]), str(views), str(captures)]
            cloth = args.before_cloth if variant == "before" else args.after_cloth
            if cloth is not None:
                command.append(f"--cloth={cloth}")
            code, elapsed = run(command, ROOT, env, folder / "stdout.log", folder / "stderr.log", 600)
            entry = {"variant": variant, "repetition": repetition + 1, "argv": command,
                     "exit_code": code, "elapsed_seconds": elapsed, "views": {}}
            report["runs"].append(entry)
            (out / "run.json").write_text(json.dumps(report, indent=2) + "\n")
            if code:
                raise SystemExit(f"{variant} failed; inspect {folder}")
            for view in expected:
                stats = json.loads((captures / f"{view}.perf.json").read_text())
                with (captures / f"{view}.png").open("rb") as image:
                    header = image.read(24)
                size = struct.unpack(">II", header[16:24])
                if header[:8] != b"\x89PNG\r\n\x1a\n" or size != (2560, 1440) or stats["frames"] != 60:
                    raise SystemExit(f"unexpected resolution/sample count in {folder}/{view}")
                entry["views"][view] = {k: stats[k] for k in ("frames", "mean_ms", "p95_ms", "fps_from_mean")}
                entry["views"][view].update({k: stats[k] for k in ("cloth", "cloth_iterations") if k in stats})
            (out / "run.json").write_text(json.dumps(report, indent=2) + "\n")
            print(f"{variant}-{repetition + 1}: " + ", ".join(
                f"{v}={s['fps_from_mean']:.1f} FPS" for v, s in entry["views"].items()), flush=True)
    report["summary"] = {}
    for view in expected:
        summary = report["summary"][view] = {}
        for variant in binaries:
            samples = [r["views"][view] for r in report["runs"] if r["variant"] == variant]
            summary[variant] = {k: statistics.median(s[k] for s in samples)
                                for k in ("mean_ms", "p95_ms", "fps_from_mean")}
            summary[variant]["fps_range"] = [min(s["fps_from_mean"] for s in samples),
                                             max(s["fps_from_mean"] for s in samples)]
        summary["fps_change_percent"] = 100 * (summary["after"]["fps_from_mean"] / summary["before"]["fps_from_mean"] - 1)
    report["finished_utc"] = datetime.now(timezone.utc).isoformat()
    (out / "run.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report["summary"], indent=2))


if __name__ == "__main__":
    main()
