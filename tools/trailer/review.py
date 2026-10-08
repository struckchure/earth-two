#!/usr/bin/env python3
"""Create temporal contact sheets and flag abrupt changes inside camera shots.

The report helps locate popping; visual inspection is still required. Run:
  python3 tools/trailer/review.py out/trailer/v2/clips
"""
import argparse
import json
import statistics
import subprocess
from pathlib import Path


def ffmpeg(*args):
    return subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', *map(str, args)], check=True, capture_output=True).stdout


def review(clips):
    out = clips.parent / 'motion-review'
    out.mkdir(parents=True, exist_ok=True)
    report = []
    for clip in sorted(clips.glob('*.mp4')):
        # Decode every frame, at a small resolution, to find unusually abrupt
        # within-shot changes. There should be no editorial cuts inside a clip.
        raw = ffmpeg('-i', clip, '-vf', 'scale=96:54,format=gray', '-f', 'rawvideo', 'pipe:1')
        size = 96 * 54
        frames = [raw[i:i + size] for i in range(0, len(raw), size)]
        differences = [sum(abs(a - b) for a, b in zip(previous, current)) / size
                       for previous, current in zip(frames, frames[1:])]
        median = statistics.median(differences) if differences else 0
        threshold = max(1.5, median * 5 + 0.2)
        spikes = [(i + 1, round(d, 3)) for i, d in enumerate(differences) if d > threshold]
        # Four samples a second include the entire move, rather than one still.
        ffmpeg('-i', clip, '-vf', 'fps=4,scale=480:270,tile=6x4', '-frames:v', '1', out / (clip.stem + '.jpg'))
        item = {'clip': clip.name, 'frames': len(frames), 'mean_delta_median': round(median, 3),
                'mean_delta_max': round(max(differences, default=0), 3), 'abrupt_changes': spikes}
        report.append(item)
        print(f'{clip.name}: {len(frames)} frames, median delta {median:.3f}, {len(spikes)} abrupt changes', flush=True)
    (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return out


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('clips', type=Path)
    args = parser.parse_args()
    print(review(args.clips.resolve()))
