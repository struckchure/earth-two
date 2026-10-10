#!/usr/bin/env python3
"""Compare native Go and Rust release frame pacing over the shared review tour."""
import argparse
from datetime import datetime, timezone
import json
import os
import platform
from pathlib import Path
import re
import statistics
import struct
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
BASELINE = ROOT / "build/migration-baseline/step-1-go-reference-v2"
GO_SOURCE = BASELINE / "reference"


def run(command, cwd, env, stdout_path, stderr_path, timeout):
    start = time.monotonic()
    with stdout_path.open("w") as stdout, stderr_path.open("w") as stderr:
        completed = subprocess.run(command, cwd=cwd, env=env, stdout=stdout,
                                   stderr=stderr, timeout=timeout, check=False)
    return completed.returncode, round(time.monotonic() - start, 3)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="new output directory; must not already exist")
    parser.add_argument("--runs", type=int, default=3)
    args = parser.parse_args()
    out = args.output.resolve()
    if args.runs < 1 or out.exists():
        parser.error("runs must be positive and output must not already exist")
    out.mkdir(parents=True)
    (out / "bin").mkdir()
    go_binary = out / "bin/go-tour"
    rust_binary = ROOT / "build/rust/release/examples/parity_tour"
    if not rust_binary.is_file():
        parser.error(f"build the native release example first: {rust_binary}")
    views = ROOT / "tools/migration/presentation-views.json"
    metadata = {
        "started_utc": datetime.now(timezone.utc).isoformat(),
        "runs_per_language": args.runs,
        "go_reference_commit": json.loads((BASELINE / "run.json").read_text())["reference_commit"],
        "rust_worktree_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "rust_worktree_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip()),
        "host": {"platform": platform.platform(), "machine": platform.machine(), "processor": platform.processor()},
        "views_sha256": __import__("hashlib").sha256(views.read_bytes()).hexdigest(),
        "configuration": {"builds": "Go native optimized default; Rust cargo --release", "resolution": "1280x720 logical / 2560x1440 physical", "vsync": True, "msaa": "4x", "hour": 11, "storm": 0, "test_crowd": "disabled; both default population values are 25", "sample_frames_per_view": 60},
        "runs": [],
        "note": "Frame cadence is wall time between application render/update frames; it includes display pacing and CPU/GPU stalls, not an isolated GPU timestamp.",
    }
    clean = {k: v for k, v in os.environ.items() if not k.startswith("EARTH_TWO_")}
    go_env = clean | {"GOWORK": str(GO_SOURCE / "build/deps/native.work"), "GOFLAGS": ""}
    go_command = ["go", "build", "-trimpath", "-o", str(go_binary), "./tools/tour"]
    code, elapsed = run(go_command, GO_SOURCE, go_env, out / "go-build.stdout.log", out / "go-build.stderr.log", 600)
    metadata["go_build"] = {"argv": go_command, "exit_code": code, "elapsed_seconds": elapsed}
    if code:
        (out / "run.json").write_text(json.dumps(metadata, indent=2) + "\n")
        raise SystemExit(f"Go release build failed; see {out / 'go-build.stderr.log'}")

    for index in range(args.runs):
        # Counterbalance order between languages to reduce thermal/order bias.
        languages = ["go", "rust"] if index % 2 == 0 else ["rust", "go"]
        for language in languages:
            name = f"{language}-{index + 1}"
            run_dir = out / name
            captures = run_dir / "captures"
            env = clean | {"EARTH_TWO_IDENTITY_PATH": str(captures / "disposable-profile/pk"),
                           "EARTH_TWO_HOUR": "11", "EARTH_TWO_STORM": "0"}
            if language == "go":
                env |= {"GOWORK": str(GO_SOURCE / "build/deps/native.work"),
                        "EARTH_TWO_TOUR_STATS": "1"}
                command = [str(go_binary), str(views), str(captures)]
                cwd = GO_SOURCE
            else:
                command = [str(rust_binary), str(views), str(captures)]
                cwd = ROOT
            run_dir.mkdir()
            code, elapsed = run(command, cwd, env, run_dir / "stdout.log", run_dir / "stderr.log", 600)
            stdout = (run_dir / "stdout.log").read_text(errors="replace")
            stats = re.findall(r"tour (.*?): ([\d.]+) FPS, mean ([\d.]+) ms(?:, median ([\d.]+) ms)?, p95 ([\d.]+) ms(?:, p99 ([\d.]+) ms)? \((\d+) frames\)", stdout)
            images = []
            for view_name, *_ in stats:
                if language == "rust":
                    timings = json.loads((captures / f"{view_name}.perf.json").read_text())
                    metadata["configuration"]["cloth_constraint_passes"] = {
                        "go": 6, "rust": timings.get("cloth_iterations", "unrecorded")}
                image = captures / f"{view_name}.png"
                if image.is_file():
                    with image.open("rb") as capture:
                        header = capture.read(24)
                    if header[:8] != b"\x89PNG\r\n\x1a\n":
                        continue
                    width, height = struct.unpack(">II", header[16:24])
                    images.append({"name": view_name, "width": width, "height": height})
            metadata["runs"].append({"name": name, "language": language, "order": languages,
                                     "exit_code": code, "elapsed_seconds": elapsed,
                                     "captures": images,
                                     "views": [{"name": s[0], "fps": float(s[1]), "mean_ms": float(s[2]),
                                                "median_ms": float(s[3]) if s[3] else None,
                                                "p95_ms": float(s[4]), "p99_ms": float(s[5]) if s[5] else None,
                                                "frames": int(s[6])} for s in stats]})
            (out / "run.json").write_text(json.dumps(metadata, indent=2) + "\n")
            print(f"{name}: exit={code}, {elapsed:.1f}s, measured views={len(stats)}", flush=True)
            if code:
                print(f"See {run_dir / 'stderr.log'}", flush=True)

    metadata["summary"] = {}
    for language in ("go", "rust"):
        language_runs = [r for r in metadata["runs"] if r["language"] == language]
        metadata["summary"][language] = {}
        for view in ("gate_orbit", "gate_fixed", "market_fixed"):
            samples = [next(v for v in r["views"] if v["name"] == view) for r in language_runs]
            metadata["summary"][language][view] = {
                "fps_median": statistics.median(v["fps"] for v in samples),
                "fps_range": [min(v["fps"] for v in samples), max(v["fps"] for v in samples)],
                "mean_ms_median": statistics.median(v["mean_ms"] for v in samples),
                "p95_ms_median": statistics.median(v["p95_ms"] for v in samples),
            }
    metadata["finished_utc"] = datetime.now(timezone.utc).isoformat()
    (out / "run.json").write_text(json.dumps(metadata, indent=2) + "\n")
    if any(r["exit_code"] or len(r["views"]) != 3 or len(r["captures"]) != 3
           or any((c["width"], c["height"]) != (2560, 1440) for c in r["captures"])
           for r in metadata["runs"]):
        raise SystemExit(f"incomplete benchmark; see {out / 'run.json'}")


if __name__ == "__main__":
    main()
