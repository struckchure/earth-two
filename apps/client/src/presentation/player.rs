//! The clip clock: illusion's render/animation.go AnimationPlayer and
//! Animations, kept apart from Bevy's own player so the state machine's
//! timing (once-clips that hold, crossfades on wall time, manual time) is the
//! Go game's. graph.rs hands the result to Bevy's AnimationGraph each frame.

use bevy::prelude::*;
use std::collections::HashMap;

/// ClipLibrary is what the state machine needs to know about a model's
/// clips: their names, in file order, and how long each lasts. It stands in
/// for illusion's Animations; the viewer fills it from the glTF's clips.
#[derive(Clone, Debug, Default)]
pub struct ClipLibrary {
    names: Vec<String>,
    durations: Vec<f32>,
    by_name: HashMap<String, usize>,
}

impl ClipLibrary {
    pub fn new(clips: impl IntoIterator<Item = (String, f32)>) -> ClipLibrary {
        let mut lib = ClipLibrary::default();
        for (name, duration) in clips {
            lib.by_name.insert(name.clone(), lib.names.len());
            lib.names.push(name);
            lib.durations.push(duration);
        }
        lib
    }

    /// keyframed makes a library as raylib loads glTF clips: n keyframes at 60
    /// a second last (n-1)/60 seconds.
    pub fn keyframed(clips: impl IntoIterator<Item = (String, u32)>) -> ClipLibrary {
        ClipLibrary::new(
            clips
                .into_iter()
                .map(|(name, keyframes)| (name, keyframes.saturating_sub(1) as f32 / 60.0)),
        )
    }

    /// clip returns the index of the clip called name.
    pub fn clip(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    /// names returns the clips' names, in file order.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// duration returns the length of clip i in seconds.
    pub fn duration(&self, i: usize) -> f32 {
        self.durations[i]
    }

    pub fn duration_of(&self, name: &str) -> Option<f32> {
        self.clip(name).map(|i| self.duration(i))
    }
}

/// Libraries is a resource: a clip library for each model path, filled by
/// whoever loads the models.
#[derive(Resource, Default)]
pub struct Libraries {
    pub by_model: HashMap<String, ClipLibrary>,
}

impl Libraries {
    pub fn get(&self, model: &str) -> Option<&ClipLibrary> {
        self.by_model.get(model)
    }
}

/// Playback is one clip's clock.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Playback {
    pub clip: String,
    pub time: f32,
    pub once: bool,
}

/// AnimationPlayer plays clips by name on the entity's model, in the Go
/// engine's terms. graph.rs carries its state to Bevy's player.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct AnimationPlayer {
    /// Model is the path of the model whose clips it plays (a key into
    /// Libraries), so the same component serves bodies and garments.
    pub model: String,
    /// Speed scales playback; 0 means 1, and negative plays backwards.
    pub speed: f32,
    /// Paused holds the current pose.
    pub paused: bool,
    /// ManualTime lets Seek drive the current pose while crossfades still
    /// advance: for animation synchronised to physics displacement or a
    /// fixed-step clock.
    pub manual_time: bool,

    pub(crate) current: Playback,
    pub(crate) previous: Playback,
    /// Crossfade progress and length, in seconds.
    pub(crate) fade: f32,
    pub(crate) fade_length: f32,
    pub(crate) finished: bool,
    /// The last Play or PlayOnce started a clip, so FadeIn applies.
    pub(crate) fresh: bool,
}

impl AnimationPlayer {
    pub fn new(model: &str) -> AnimationPlayer {
        AnimationPlayer {
            model: model.to_string(),
            ..Default::default()
        }
    }

    /// Play loops clip. If it's already looping, it carries on.
    pub fn play(&mut self, clip: &str) -> &mut AnimationPlayer {
        if self.current.clip == clip && !self.current.once {
            self.fresh = false;
            return self;
        }
        self.start(clip, false);
        self
    }

    /// PlayOnce plays clip once and holds its last frame, sending
    /// AnimationFinished when it ends. If it's already the current clip, it
    /// carries on; use replay to start it over.
    pub fn play_once(&mut self, clip: &str) -> &mut AnimationPlayer {
        if self.current.clip == clip && self.current.once {
            self.fresh = false;
            return self;
        }
        self.start(clip, true);
        self
    }

    /// FadeIn blends from the previous clip to the one just started over
    /// seconds, instead of switching at once. Call it right after play or
    /// play_once; it does nothing if they didn't start a clip.
    pub fn fade_in(&mut self, seconds: f32) -> &mut AnimationPlayer {
        if self.fresh && !self.previous.clip.is_empty() && seconds > 0.0 {
            self.fade = 0.0;
            self.fade_length = seconds;
        }
        self
    }

    /// Settle ends any crossfade at once, leaving the current clip alone to
    /// pose the model.
    pub fn settle(&mut self) {
        self.fade = 0.0;
        self.fade_length = 0.0;
    }

    /// Replay starts the current clip over.
    pub fn replay(&mut self) {
        self.current.time = 0.0;
        self.finished = false;
    }

    /// Seek jumps to seconds into the current clip.
    pub fn seek(&mut self, seconds: f32) {
        self.current.time = seconds.max(0.0);
        self.finished = false;
    }

    /// Clip returns the name of the clip playing, or "".
    pub fn clip(&self) -> &str {
        &self.current.clip
    }

    /// Time returns how many seconds into the current clip playback is.
    pub fn time(&self) -> f32 {
        self.current.time
    }

    /// Finished reports whether a PlayOnce clip has reached its end.
    pub fn finished(&self) -> bool {
        self.finished
    }

    pub fn once(&self) -> bool {
        self.current.once
    }

    /// Previous is the clip being faded from, while a crossfade runs.
    pub fn previous(&self) -> Option<&Playback> {
        (self.fade_length > 0.0 && !self.previous.clip.is_empty()).then_some(&self.previous)
    }

    /// Blend is how far the crossfade has come, 0 (all the previous clip) to
    /// 1 (all the current), or 1 with none running.
    pub fn blend(&self) -> f32 {
        if self.fade_length > 0.0 {
            (self.fade / self.fade_length).min(1.0)
        } else {
            1.0
        }
    }

    pub fn fading(&self) -> bool {
        self.fade_length > 0.0
    }

    fn start(&mut self, clip: &str, once: bool) {
        if self.fade_length > 0.0 && self.fade < self.fade_length / 2.0 {
            // Interrupting a crossfade that's still mostly the previous
            // clip: keep fading from that clip rather than jumping to the
            // current one. (Two clips blend at most, so the pose can't be
            // carried over exactly.)
        } else if !self.current.clip.is_empty() {
            self.previous = self.current.clone();
        }
        self.current = Playback {
            clip: clip.to_string(),
            time: 0.0,
            once,
        };
        self.fade = 0.0; // switch at once unless fade_in follows
        self.fade_length = 0.0;
        self.finished = false;
        self.fresh = true;
    }

    /// advance moves the clock by dt seconds of wall time: what illusion's
    /// advanceAnimations does for one player.
    pub fn advance(&mut self, lib: &ClipLibrary, dt: f32) -> bool {
        if self.paused || self.current.clip.is_empty() {
            return false;
        }
        let mut step = dt;
        if self.speed != 0.0 {
            step *= self.speed;
        }
        let mut finished_now = false;
        if let Some(i) = lib.clip(&self.current.clip) {
            let duration = lib.duration(i);
            if !self.manual_time {
                self.current.time = advance(&self.current, step, duration);
            }
            if self.current.once && !self.finished && self.current.time >= duration {
                self.finished = true;
                finished_now = true;
            }
        }
        if self.fade_length > 0.0 {
            if let Some(i) = lib.clip(&self.previous.clip) {
                self.previous.time = advance(&self.previous, step, lib.duration(i));
            }
            self.fade += dt; // blending follows wall time, independently of stride speed
            if self.fade >= self.fade_length {
                self.fade = 0.0;
                self.fade_length = 0.0;
            }
        }
        finished_now
    }
}

/// advance moves playback b by step seconds (negative plays backwards)
/// through a clip lasting duration, keeping the clock within the clip: once
/// clips stop at either end, looping ones wrap.
fn advance(b: &Playback, step: f32, duration: f32) -> f32 {
    let t = b.time + step;
    if b.once {
        return 0.0f32.max(t.min(duration));
    }
    if duration > 0.0 {
        let mut t = t % duration;
        if t < 0.0 {
            t += duration;
        }
        return t;
    }
    0.0
}

/// AnimationFinished is sent when a PlayOnce clip reaches its end.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct AnimationFinished {
    pub entity: Entity,
    pub clip: String,
}

/// advance_animations moves every player's clock, in PostUpdate before the
/// pose is handed to Bevy's animation.
pub fn advance_animations(
    mut players: Query<(Entity, &mut AnimationPlayer)>,
    libraries: Res<Libraries>,
    time: Res<Time>,
    mut finished: MessageWriter<AnimationFinished>,
) {
    let dt = time.delta_secs();
    for (entity, mut p) in &mut players {
        let Some(lib) = libraries.get(&p.model) else {
            continue;
        };
        if p.advance(lib, dt) {
            finished.write(AnimationFinished {
                entity,
                clip: p.current.clip.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib() -> ClipLibrary {
        ClipLibrary::keyframed([("Walk".to_string(), 61), ("Jump".to_string(), 31)])
    }

    fn run(p: &mut AnimationPlayer, seconds: f32) -> u32 {
        let lib = lib();
        let mut finished = 0;
        for _ in 0..(seconds * 60.0).round() as u32 {
            if p.advance(&lib, 1.0 / 60.0) {
                finished += 1;
            }
        }
        finished
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    // render/animation_test.go TestAnimationLoops.
    #[test]
    fn loops() {
        let mut p = AnimationPlayer::default();
        p.play("Walk");
        run(&mut p, 1.25);
        assert!(near(p.time(), 0.25) && !p.finished());
        p.play("Walk");
        assert!(near(p.time(), 0.25), "Play restarted the running clip");
    }

    // TestAnimationOnceFinishesAndHolds.
    #[test]
    fn once_finishes_and_holds() {
        let mut p = AnimationPlayer::default();
        p.play_once("Jump");
        let finished = run(&mut p, 1.0);
        assert!(p.finished() && near(p.time(), 0.5));
        assert_eq!(finished, 1);
        p.replay();
        assert_eq!(run(&mut p, 1.0), 1, "Replay didn't finish again");
    }

    // TestAnimationFadeAndSpeed.
    #[test]
    fn fade_and_speed() {
        let mut p = AnimationPlayer::default();
        p.play("Walk");
        run(&mut p, 0.5);
        p.play_once("Jump").fade_in(0.25);
        assert_eq!(p.clip(), "Jump");
        assert_eq!(p.previous.clip, "Walk");
        assert_eq!(p.fade_length, 0.25);
        run(&mut p, 0.1);
        assert!(near(p.previous.time, 0.6) && near(p.fade, 0.1));
        run(&mut p, 0.2);
        assert_eq!(p.fade_length, 0.0, "fade didn't end");

        p.play("Walk");
        p.speed = 2.0;
        run(&mut p, 0.25);
        assert!(near(p.time(), 0.5));
        p.paused = true;
        run(&mut p, 0.25);
        assert!(near(p.time(), 0.5), "paused player moved");
    }

    // TestAnimationsClipLookup.
    #[test]
    fn clip_lookup() {
        let a = lib();
        assert_eq!(a.clip("Jump"), Some(1));
        assert_eq!(a.clip("Swim"), None);
        assert!(near(a.duration(0), 1.0));
        assert_eq!(a.names(), ["Walk", "Jump"]);
    }

    // TestAnimationFadeInNeedsANewClip.
    #[test]
    fn fade_in_needs_a_new_clip() {
        let mut p = AnimationPlayer::default();
        p.play("Walk");
        p.play("Jump").fade_in(0.2);
        p.settle();
        p.play("Jump").fade_in(0.4);
        assert_eq!(p.fade_length, 0.0);
    }

    // TestAnimationPlaysBackwards.
    #[test]
    fn plays_backwards() {
        let mut p = AnimationPlayer::default();
        p.play("Walk");
        p.speed = -1.0;
        run(&mut p, 0.25);
        assert!(near(p.time(), 0.75));
        let mut p = AnimationPlayer::default();
        p.play_once("Jump");
        p.speed = -1.0;
        run(&mut p, 0.25);
        assert_eq!(p.time(), 0.0);
    }

    // TestAnimationInterruptedFadeKeepsDominantClip.
    #[test]
    fn interrupted_fade_keeps_dominant_clip() {
        let mut p = AnimationPlayer::default();
        p.play("Walk");
        p.play("Run").fade_in(1.0);
        p.fade = 0.2;
        p.play_once("Jump").fade_in(0.2);
        assert_eq!(p.previous.clip, "Walk");
    }

    // TestManualTimeHoldsPoseButCompletesBlend.
    #[test]
    fn manual_time_holds_pose_but_completes_blend() {
        let mut p = AnimationPlayer::default();
        p.play("Walk");
        p.play_once("Jump").fade_in(0.2);
        p.manual_time = true;
        p.seek(0.12);
        run(&mut p, 0.3);
        assert!(near(p.time(), 0.12) && p.fade_length == 0.0);
        p.paused = true;
        p.play("Walk").fade_in(0.2);
        run(&mut p, 0.1);
        assert_eq!(p.fade, 0.0, "paused manual blend advanced");
        p.paused = false;
        p.manual_time = false;
        run(&mut p, 0.1);
        assert!(near(p.time(), 0.1));
    }
}
