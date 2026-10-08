EARTH TWO / 55-SECOND CINEMATIC TEASER

The trailer uses the game's native renderer and the existing world assets.
It has an original atmospheric score, game sound effects and no recorded VO.
The end card avoids a release date or unverified storefront call to action.

From the repository root, with Go and FFmpeg available:

  make deps
  GOWORK="$PWD/build/deps/native.work" go build -o build/trailer ./tools/trailer
  EARTH_TWO_HOUR=11 EARTH_TWO_STORM=0 build/trailer
  python3 tools/trailer/assemble.py

If the default Go build cache is unavailable, add GOCACHE="$PWD/build/go-cache".
On macOS the capture needs access to the WindowServer, like normal desktop play.
The game window should remain open until all shots finish. Esc does not stop it;
closing the window cancels capture and reports an incomplete run.

shots.json specifies duration, player/streaming anchor, camera start/end,
faction clothing and time of day. Camera positions are in metres, Y up.
A normal shot is captured before the HUD draws. A paper shot includes the menu.
The 150-frame warmup at each cut lets terrain and lighting settle.
The cinematic camera is applied before streaming and visibility decisions.
Capture advances simulation at a fixed 30 fps, regardless of encoding stalls.
Clothing uses skeletal animation; procedural scatter is omitted for clean
cinematic backgrounds. Normal play keeps its existing cloth and scatter.
Keyboard movement is disabled during capture.
The shot durations and the timed script total 55 seconds.

assemble.py joins the clips, writes title graphics, generates the score,
mixes existing game effects, and exports the final MP4 plus a narration guide.
It needs only Python's standard library and an FFmpeg build with libass/libx264.
The timed script is in out/trailer/voiceover-script.txt; the subtitle cues are
in out/trailer/voiceover.srt. The main MP4 contains no narration cues.

To record VO, use the guide copy and place your full recording at time 00:00.
In your editor, keep narration on its own track and lower the soundtrack as
needed. Reuse soundtrack.wav, footage.mp4 and clips/ for further edits.
Source asset credits accompany the output in CREDITS.txt.

If Homebrew FFmpeg reports a missing libvpx.11.dylib while libvpx.12.dylib is
installed, the current workspace has a local compatibility lookup at
build/media-libs. Prefix capture and assembly with:

  DYLD_LIBRARY_PATH="$PWD/build/media-libs"

This lookup is local to these commands; no system installation was changed.

Revision QA:
  python3 tools/trailer/review.py out/trailer/v2/clips

The review tool checks every decoded frame for abrupt within-shot changes
and creates temporal contact sheets for visual inspection. Review these
before assembly; the numerical check alone does not prove visual quality.
