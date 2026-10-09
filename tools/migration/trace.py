#!/usr/bin/env python3
"""Run fixed-step vehicle input scripts against an already prepared Go baseline."""
import argparse
import json
import math
import os
from pathlib import Path
import subprocess
from baseline import ROOT, digest, run_process, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("--repeats", type=int, default=3)
    args = parser.parse_args()
    if args.repeats < 2:
        parser.error("at least two runs are needed to measure variability")
    out = args.baseline.resolve()
    source = out / "reference"
    metadata = json.loads((out / "run.json").read_text())
    target = source / "vehicle/migration_trace_test.go"
    if target.exists() or (out / "traces").exists():
        parser.error("trace evidence or injected test already exists; use a fresh baseline")
    template = ROOT / "tools/migration/vehicle_trace_test.go.txt"
    script = ROOT / "tools/migration/vehicle-inputs.json"
    config = json.loads(script.read_text())
    traces = out / "traces"
    traces.mkdir()
    (traces / "inputs.json").write_bytes(script.read_bytes())
    (traces / "observer.go.txt").write_bytes(template.read_bytes())
    info = {"reference_commit":metadata["reference_commit"],"observer_sha256":digest(template),"inputs_sha256":digest(script),"repeats":args.repeats,"runs":[],"status":"running"}
    write_json(traces / "run.json", info)
    env = {k:v for k,v in os.environ.items() if not k.startswith("EARTH_TWO_")}
    env.update(GOWORK=str(source / "build/deps/native.work"), GOFLAGS="", EARTH_TWO_TRACE_INPUT=str(traces / "inputs.json"))
    target.write_bytes(template.read_bytes())
    try:
        for i in range(args.repeats):
            run = traces / str(i+1)
            run.mkdir()
            with (run / "tests.log").open("w") as log:
                code = run_process(["go","test","./vehicle","-run","^TestMigrationVehicleTrace$","-count=1","-v"],cwd=source,env=env | {"EARTH_TWO_TRACE_OUT":str(run)},stdout=log,stderr=subprocess.STDOUT,timeout=300)
            info["runs"].append({"repeat":i+1,"exit_code":code})
            write_json(traces / "run.json", info)
            if code:
                info["status"]="failed"
                write_json(traces / "run.json",info)
                return 1
    except (subprocess.TimeoutExpired, OSError, KeyboardInterrupt) as error:
        info.update(status="failed", error=str(error))
        write_json(traces / "run.json", info)
        raise
    finally:
        target.unlink()
    summary = {}
    count = sum(p["ticks"] for p in config["phases"])
    for path in sorted((traces / "1").glob("*.json")):
        runs = [json.loads((traces / str(i+1) / path.name).read_text())["samples"] for i in range(args.repeats)]
        if any(len(r) != count for r in runs):
            raise ValueError("incomplete trace")
        max_position_delta = max(math.dist(a["position"],b["position"]) for r in runs[1:] for a,b in zip(runs[0],r))
        summary[path.stem] = {"samples_per_run":count,"min_up_y_each_run":[min(s["up_y"] for s in r) for r in runs],"final_speed_each_run":[r[-1]["speed"] for r in runs],"max_position_delta_from_first_run":max_position_delta}
    if set(summary) != {"bike", "buggy", "hauler", "hauler_tanker", "rover", "trike"}:
        raise ValueError("incomplete vehicle coverage")
    info.update(status="passed",summary=summary,note="Measured repeatability only; does not set cross-engine acceptance tolerances. Keyboard requests and post-tick consumed controls are both recorded.")
    write_json(traces / "run.json", info)
    print(json.dumps(summary,indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
