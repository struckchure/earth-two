#!/usr/bin/env python3
"""Assemble the 55-second Earth Two teaser. Requires Python 3 and FFmpeg."""
import argparse
import array
import json
import math
from pathlib import Path
import random
import subprocess
import sys
import wave

ROOT = Path(__file__).resolve().parents[2]
CUES = [
    (0.8, 5.6, 'The company left. The colony stayed.'),
    (6.3, 10.8, "Now you're here. Two thousand marks in debt."),
    (11.5, 15.8, 'On the Red, nothing comes free.'),
    (16.7, 20.7, 'Air. Water. A place to sleep.'),
    (21.2, 24.8, 'And every deal starts with a contract.'),
    (25.6, 29.8, 'Take the work. Learn the roads.'),
    (30.5, 34.8, 'Beyond the dome, the frontier is wide open.'),
    (35.4, 39.8, 'One job. One journey. One step closer to freedom.'),
    (40.6, 45.2, 'They called this place Earth Two.'),
    (46.2, 49.6, 'What will you make of it?'),
    (50.5, 53.9, 'Earth Two. Your passage is only the beginning.'),
]


def run(*args):
    subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', *map(str, args)], check=True)


def ass_time(t):
    n = round(t * 100)
    return f'{n // 360000}:{n // 6000 % 60:02}:{n // 100 % 60:02}.{n % 100:02}'


def srt_time(t):
    n = round(t * 1000)
    return f'{n // 3600000:02}:{n // 60000 % 60:02}:{n // 1000 % 60:02},{n % 1000:03}'


def ass_header():
    return '''[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080
WrapStyle: 2
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Main,Arial,62,&H00E7EBF1,&H00E7EBF1,&H00140F0C,&H00000000,-1,0,0,0,100,100,3,0,1,1,1,7,0,0,0,1
Style: Small,Menlo,22,&H00ABB4C5,&H00ABB4C5,&H00000000,&H00000000,0,0,0,0,100,100,2,0,1,0,0,7,0,0,0,1
Style: Cue,Arial,32,&H00E7EBF1,&H00E7EBF1,&H00140F0C,&H00000000,0,0,0,0,100,100,0,0,1,0,0,2,80,80,54,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
'''


def event(start, end, style, text, tags='', layer=1):
    return f'Dialogue: {layer},{ass_time(start)},{ass_time(end)},{style},,0,0,0,,{{{tags}}}{text}\n'


def write_assets(out):
    text = ass_header()
    text += event(0, 50, 'Small', 'EARTH TWO  /  REVEAL TEASER', r'\pos(90,50)\fad(1000,200)')
    text += event(0, 50, 'Small', 'IN-ENGINE FOOTAGE  /  DEVELOPMENT BUILD', r'\an9\pos(1830,50)\fs18\fad(1000,200)')
    text += event(1.3, 5.6, 'Small', 'THE RED  /  THE PADS', r'\pos(94,759)\fad(500,300)')
    text += event(1.5, 5.6, 'Main', 'THE COMPANY LEFT.', r'\move(90,807,90,790,0,650)\fad(500,300)')
    text += event(6.6, 10.6, 'Small', 'PASSAGE DEBT', r'\pos(94,759)\fad(350,250)')
    text += event(6.8, 10.6, 'Main', '2,000 MARKS.', r'\move(90,807,90,790,0,500)\fad(350,250)')
    text += event(11.7, 15.5, 'Small', 'LANDFALL  /  A COLONY BUILT TO LAST', r'\pos(90,999)\fad(300,200)')
    text += event(16.5, 20.8, 'Small', 'THE EXCHANGE  /  EVERYTHING HAS A PRICE', r'\pos(90,999)\fad(300,200)')
    text += event(21.1, 24.9, 'Small', 'EVERY DEAL IS A CONTRACT.', r'\an2\pos(960,1034)\fs26\fad(250,200)')
    text += event(25.5, 29.8, 'Small', 'THE PADS  /  FIND YOUR WAY', r'\pos(90,999)\fad(300,200)')
    text += event(30.5, 34.8, 'Small', 'THE FRINGE  /  BEYOND THE DOME', r'\pos(90,999)\fad(300,200)')
    text += event(35.5, 39.7, 'Small', 'THE HOLD  /  A LIFE ON THE FRONTIER', r'\pos(90,999)\fad(300,200)')
    text += event(41, 45.6, 'Main', 'MAKE A LIFE ON THE RED.', r'\move(90,807,90,790,0,650)\fs54\fad(500,350)')
    text += event(46.4, 49.7, 'Small', 'YOUR PASSAGE IS ONLY THE BEGINNING.', r'\an2\pos(960,1034)\fs26\fad(300,250)')
    # A Registrar's rule, a large title, and a restrained end card.
    text += event(50.25, 55, 'Small', 'ARRIVAL REGISTRATION  /  FORM A-1', r'\an5\pos(960,387)\fs20\fad(400,800)')
    text += event(50.35, 55, 'Main', 'EARTH TWO', r'\an5\pos(960,505)\fs126\fsp14\fad(500,800)')
    text += event(50.65, 55, 'Small', 'YOUR PASSAGE IS ONLY THE BEGINNING.', r'\an5\pos(960,636)\fs26\fad(500,800)')
    text += event(51.0, 55, 'Small', 'A FREE-ROAM SCI-FI ROLE GAME', r'\an5\pos(960,725)\fs19\fad(500,800)')
    text += event(50.25, 55, 'Main', '', r'\an7\pos(640,587)\1c&H006FA9D8\p1\fad(400,800)')[:-1] + 'm 0 0 l 640 0 640 2 0 2\n'
    (out / 'titles.ass').write_text(text)
    guide = ass_header()
    for a, b, line in CUES:
        guide += event(a, b, 'Cue', line, r'\an2\pos(960,1032)\fad(100,120)')
    (out / 'voiceover-guide.ass').write_text(guide)
    (out / 'voiceover.srt').write_text(''.join(f'{i}\n{srt_time(a)} --> {srt_time(b)}\n{line}\n\n' for i, (a, b, line) in enumerate(CUES, 1)))
    script = 'EARTH TWO — 55-SECOND TRAILER\nVOICE-OVER SCRIPT\n\n'
    script += 'Delivery: grounded, calm and conversational. Leave the pauses in place.\nStart quietly; build resolve through the frontier montage. Let the title breathe.\nThe main trailer contains music and effects, with no voice-over recorded.\n\n'
    for a, b, line in CUES:
        script += f'{a:04.1f}–{b:04.1f}s\n{line}\n\n'
    script += 'CONTINUOUS READ\n\n' + '\n'.join(line for _, _, line in CUES) + '\n'
    (out / 'voiceover-script.txt').write_text(script)
    credits = 'EARTH TWO TRAILER — ASSET CREDITS\n\nFootage captured directly from the game.\nTrailer score: original procedural composition generated for this trailer.\n\n'
    for folder in ['world', 'characters', 'sounds']:
        credit_file = ROOT / 'assets' / folder / 'CREDITS.txt'
        if credit_file.exists():
            credits += f'\n{folder.upper()}\n' + credit_file.read_text() + '\n'
    (out / 'CREDITS.txt').write_text(credits)


def score(out):
    """An original subdued, evolving synth bed, leaving room for narration."""
    sr = 24000
    rng = random.Random(88)
    samples = array.array('h')
    noise = 0.0
    tau = math.tau
    notes = [110.0, 164.814, 146.832, 130.813, 110.0, 146.832, 164.814, 220.0]
    for i in range(55 * sr):
        t = i / sr
        noise = noise * .985 + rng.uniform(-1, 1) * .015
        fade = min(1, t / 2.5, max(0, (55 - t) / 2))
        swell = .6 + .4 * min(1, t / 40)
        drone = .075 * math.sin(tau * 55 * t) + .045 * math.sin(tau * 82.41 * t + .5 * math.sin(t / 4))
        drone += .022 * math.sin(tau * 110.14 * t) + .013 * math.sin(tau * 164.8 * t)
        beat = (t - 11) % (5 / 6)
        pulse = 0.0 if t < 11 or t >= 50 else .09 * math.exp(-beat * 15) * math.sin(tau * (48 * beat + 2.8 * (1 - math.exp(-beat * 8))))
        a = max(0, t - 25) % (5 / 12)
        note = notes[int(max(0, t - 25) / (5 / 12)) % len(notes)]
        arp = 0.0 if t < 25 or t >= 50 else .027 * math.exp(-a * 8) * math.sin(tau * note * a)
        hit = 0.0 if t < 50 else .13 * math.exp(-(t - 50) * 1.4) * math.sin(tau * 44 * (t - 50))
        value = fade * (swell * drone + pulse + arp + hit + .05 * noise)
        pan = .012 * math.sin(tau * 110.0 * t + .8)
        samples.extend((int(max(-1, min(1, value + fade * pan)) * 32767), int(max(-1, min(1, value - fade * pan)) * 32767)))
    if sys.byteorder != 'little': samples.byteswap()
    with wave.open(str(out / 'original-score.wav'), 'wb') as w:
        w.setnchannels(2); w.setsampwidth(2); w.setframerate(sr); w.writeframes(samples.tobytes())


def assemble(out, guide=True):
    shots = json.loads((ROOT / 'tools/trailer/shots.json').read_text())
    if sum(s['seconds'] for s in shots) != 55:
        raise ValueError('The timed edit and voice-over script require a 55-second shot list.')
    for s in shots:
        if not (out / 'clips' / (s['name'] + '.mp4')).exists():
            raise FileNotFoundError(s['name'] + ': render footage first')
    write_assets(out)
    print('Composing original soundtrack...', flush=True)
    score(out)
    # Original music plus wind, machinery, a filing stamp and Exchange bell.
    run('-i', out / 'original-score.wav', '-stream_loop', '-1', '-i', ROOT / 'assets/sounds/wind.wav',
        '-stream_loop', '-1', '-i', ROOT / 'assets/sounds/hull_hum.wav', '-i', ROOT / 'assets/sounds/ui_stamp_0.ogg',
        '-i', ROOT / 'assets/sounds/bell_0.ogg', '-filter_complex',
        '[0:a]volume=0.85[m];[1:a]volume=0.1,afade=t=in:d=2,afade=t=out:st=52:d=3[w];'
        '[2:a]volume=0.08,afade=t=in:st=14:d=2,afade=t=out:st=24:d=2[h];'
        '[3:a]volume=0.35,adelay=21000|21000[s];[4:a]volume=0.16,adelay=50000|50000[b];'
        '[m][w][h][s][b]amix=inputs=5:duration=first:normalize=0,alimiter=limit=0.9,afade=t=out:st=53:d=2[a]',
        '-map', '[a]', '-t', '55', '-ar', '48000', '-ac', '2', out / 'soundtrack.wav')
    (out / 'concat.txt').write_text(''.join("file '" + str((out / 'clips' / (s['name'] + '.mp4')).resolve()) + "'\n" for s in shots))
    run('-f', 'concat', '-safe', '0', '-i', out / 'concat.txt', '-c', 'copy', '-an', out / 'footage.mp4')
    print('Rendering final trailer...', flush=True)
    # Preserve 16:9 delivery while presenting the world in a cinematic aperture.
    vf = ('eq=contrast=1.03:saturation=0.9:gamma=1.035,'
          'drawbox=x=0:y=0:w=iw:h=120:color=0x090b0e:t=fill,'
          'drawbox=x=0:y=960:w=iw:h=120:color=0x090b0e:t=fill,'
          "drawbox=x=0:y=120:w=iw:h=840:color=0x080a0d@0.78:t=fill:enable='gte(t,50)',"
          f"ass='{out / 'titles.ass'}',fade=t=in:st=0:d=1.2,fade=t=out:st=54:d=1")
    run('-i', out / 'footage.mp4', '-i', out / 'soundtrack.wav', '-vf', vf,
        '-map', '0:v:0', '-map', '1:a:0', '-c:v', 'libx264', '-preset', 'medium', '-crf', '18',
        '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-b:a', '256k', '-t', '55', '-movflags', '+faststart', out / 'earth-two-trailer.mp4')
    if guide:
        print('Rendering voice-over timing copy...', flush=True)
        run('-i', out / 'earth-two-trailer.mp4', '-vf', f"ass='{out / 'voiceover-guide.ass'}'",
            '-c:v', 'libx264', '-preset', 'fast', '-crf', '20', '-c:a', 'copy', '-movflags', '+faststart', out / 'earth-two-voiceover-guide.mp4')
    run('-ss', '51.5', '-i', out / 'earth-two-trailer.mp4', '-frames:v', '1', out / 'poster.jpg')
    (out / 'DELIVERY.txt').write_text('EARTH TWO / REVEAL TEASER\n\n55 seconds / 1920 x 1080 / 30 fps / H.264 MP4 / stereo audio\n\n'
        'earth-two-trailer.mp4 — final cinematic edit; music and effects, no voice-over\n'
        'earth-two-voiceover-guide.mp4 — same edit with timed narration cues\n'
        'voiceover-script.txt — timed script and continuous read\nvoiceover.srt — narration timing for your editor\n'
        'soundtrack.wav — separate stereo music/effects mix\nfootage.mp4 — clean footage without trailer text or sound\n'
        'clips/ — individual editable camera shots\nposter.jpg — end-card thumbnail\nCREDITS.txt — source asset attribution\n\n'
        'All world footage comes from the game renderer. This is an in-development cinematic teaser.\n'
        'To add your recording, align it to 00:00 in your editor and follow voiceover.srt.\n'
        'Keep the soundtrack below your voice; the cues leave intentional gaps between lines.\n'
        'Full capture and edit instructions are in tools/trailer/README.txt.\n')
    print(out / 'earth-two-trailer.mp4', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=ROOT / 'out/trailer')
    parser.add_argument('--no-guide', action='store_true')
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    assemble(args.out.resolve(), not args.no_guide)
