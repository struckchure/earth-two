#!/usr/bin/env python3
"""Capture normal native Go rendering from a prepared reference, with fresh evidence."""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import struct
import subprocess
import time
from baseline import ROOT, digest, run_process, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('baseline', type=Path)
    args = parser.parse_args()
    baseline = args.baseline.resolve()
    source = baseline / 'reference'
    out = baseline / 'review-captures'
    if out.exists():
        parser.error('review-captures already exists; preserve existing evidence')
    out.mkdir()
    views = out / 'views.json'
    views.write_bytes((ROOT / 'tools/migration/views.json').read_bytes())
    expected = [v['name'] for v in json.loads(views.read_text())]
    metadata = {
        'reference_commit': json.loads((baseline / 'run.json').read_text())['reference_commit'],
        'runner_sha256': digest(Path(__file__)), 'views_sha256': digest(views),
        'started_utc': datetime.now(timezone.utc).isoformat(), 'status': 'running', 'commands': [],
        'settings': {'cloth': 'normal desktop', 'crowd': 25, 'timing': 'display-paced, last 60 frames per view'},
        'excluded': {'screen_map': 'Original tour forces map after mapInput, before zoom initialization; direct capture stalls. Test normal map interaction separately.'},
    }
    env = {k: v for k, v in os.environ.items() if not k.startswith('EARTH_TWO_')}
    env.update(GOWORK=str(source / 'build/deps/native.work'), GOFLAGS='',
               EARTH_TWO_IDENTITY_PATH=str(out / 'disposable-identity/pk'))

    def run(name, command, extra=None):
        entry = {'name': name, 'argv': list(map(str, command)), 'environment': extra or {}}
        started = time.monotonic()
        print(f'Running {name}', flush=True)
        with (out / f'{name}.stdout.log').open('w') as stdout, (out / f'{name}.stderr.log').open('w') as stderr:
            try:
                entry['exit_code'] = run_process(command, cwd=source, env=env | (extra or {}),
                                                stdout=stdout, stderr=stderr, timeout=300)
            except subprocess.TimeoutExpired:
                entry.update(exit_code=124, error='timeout')
            except OSError as error:
                entry.update(exit_code=127, error=str(error))
        entry['elapsed_seconds'] = round(time.monotonic() - started, 3)
        metadata['commands'].append(entry)
        write_json(out / 'run.json', metadata)
        return entry['exit_code'] == 0

    binary = out / ('tour.exe' if os.name == 'nt' else 'tour')
    passed = run('build', ['go', 'build', '-o', str(binary), './tools/tour'])
    if passed:
        for name, hour, storm in [('day', '11', '0'), ('night', '22', '0'), ('storm', '11', '1')]:
            passed = run(name, [str(binary), str(views), str(out / name)],
                         {'EARTH_TWO_TOUR_STATS': '1', 'EARTH_TWO_HOUR': hour, 'EARTH_TWO_STORM': storm}) and passed
            frames = {}
            for view in expected:
                path = out / name / (view + '.png')
                if not path.is_file():
                    frames[view] = {'error': 'missing capture'}
                    passed = False
                    continue
                header = path.read_bytes()[:24]
                if header[:8] != b'\x89PNG\r\n\x1a\n':
                    raise ValueError(f'not a PNG: {path}')
                width, height = struct.unpack('>II', header[16:24])
                frames[view] = {'sha256': digest(path), 'width': width, 'height': height}
            metadata.setdefault('frames', {})[name] = frames
            write_json(out / 'run.json', metadata)
    metadata.update(status='passed' if passed else 'failed', finished_utc=datetime.now(timezone.utc).isoformat())
    write_json(out / 'run.json', metadata)
    print(f"Captures: {metadata['status']} ({out})", flush=True)
    return 0 if passed else 1


if __name__ == '__main__':
    raise SystemExit(main())
