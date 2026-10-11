"""Import CC0 reports from the Free Firearm Sound Library's prepared archive.

Download/extract https://opengameart.org/node/21826, then run:
  python3 tools/sounds/combat.py '/path/to/Prepared SFX Library'
Requires ffmpeg (or --converter afconvert on macOS). Source credit and mapping
are also recorded in assets/sounds/CREDITS.txt. No source archive is shipped.
"""
import argparse
from array import array
from pathlib import Path
import subprocess
import sys
import tempfile
import wave

RATE = 44100
# Each near-perspective recording contains two separate shots. Search windows
# exclude handling noises, speech and subsequent shots; preserve the report tail.
SOURCES = {
    'pistol': ('1911/A_42P.wav', (.8, 4.9), 1.5),
    'assault': ('AR-15/D_32P.wav', (.6, 5.5), 1.3),
    'sniper': ('Tikka/W_29P.wav', (.4, 5.5), 1.8),
    'shotgun': ('Nova/O_21P.wav', (.3, 3.3), 1.6),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('library', type=Path)
    parser.add_argument('--converter', choices=('ffmpeg', 'afconvert'), default='ffmpeg')
    args = parser.parse_args()
    destination = Path(__file__).resolve().parents[2] / 'assets/sounds'
    with tempfile.TemporaryDirectory() as temporary:
        for name, (source, starts, duration) in SOURCES.items():
            converted = Path(temporary) / (name + '.wav')
            if args.converter == 'afconvert':
                command = ['afconvert', str(args.library / source), str(converted),
                           '-f', 'WAVE', '-d', 'LEI16@44100', '-c', '1']
            else:
                command = ['ffmpeg', '-v', 'error', '-y', '-i', str(args.library / source),
                           '-ac', '1', '-ar', str(RATE), '-c:a', 'pcm_s16le', str(converted)]
            subprocess.run(command, check=True)
            with wave.open(str(converted)) as recording:
                samples = array('h', recording.readframes(recording.getnframes()))
            if sys.byteorder != 'little':
                samples.byteswap()
            for variant, search in enumerate(starts):
                # Keep 2 ms before the transient, so firing feels immediate but
                # the attack isn't clipped. Fail loudly if the source changes.
                onset = next(i for i in range(int(search * RATE), int((search + .4) * RATE))
                             if abs(samples[i]) > 32767 * .08)
                begin = max(0, onset - int(.002 * RATE))
                shot = samples[begin:begin + int(duration * RATE)]
                gain = (32767 * .84) / max(map(abs, shot))
                fade = int(.2 * RATE)
                for i, sample in enumerate(shot):
                    shot[i] = round(sample * gain * min(1, i / 22, (len(shot) - 1 - i) / fade))
                if sys.byteorder != 'little':
                    shot.byteswap()
                path = destination / f'{name}_{variant}.wav'
                with wave.open(str(path), 'wb') as output:
                    output.setparams((1, 2, RATE, 0, 'NONE', 'not compressed'))
                    output.writeframes(shot.tobytes())
                print(f'{path.name}: {duration:.1f}s, onset {onset / RATE:.3f}s')
    # Retire the original synthesized single sample; it shadows variant loading.
    (destination / 'pistol.wav').unlink(missing_ok=True)


if __name__ == '__main__':
    main()
