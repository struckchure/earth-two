//! The listener and loop envelope from game/sound.go.
use bevy::prelude::*;
use earth_two_world::terrain::smoothstep;

#[cfg(feature = "viewer")]
#[path = "sound_output.rs"]
pub mod output;

pub const MIX_AMBIENCE: f32 = 0.55;
pub const MENU_DUCK: f32 = 0.4;
pub const MAX_PAN: f32 = 0.7;
pub const LOOP_EASE: f64 = 3.;

#[derive(Clone, Copy, Debug, Default)]
pub struct Listener {
    pub at: Vec3,
    pub right: Vec3,
    pub active: bool,
}
impl Listener {
    pub fn spatial(self, at: Vec3, near: f32, far: f32) -> (f32, f32) {
        if !self.active {
            return (1., 0.);
        }
        let to = at - self.at;
        let d = to.length();
        if d >= far {
            return (0., 0.);
        }
        let k = 1. - smoothstep(near, far, d);
        let pan = if d > 0.01 {
            MAX_PAN * (to / d).dot(self.right) * smoothstep(0., near, d)
        } else {
            0.
        };
        (k * k, pan)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LoopState {
    pub playing: bool,
    pub volume: f32,
    pub pitch: f32,
    pub pan: f32,
}
impl Default for LoopState {
    fn default() -> Self {
        Self {
            playing: false,
            volume: 0.,
            pitch: 1.,
            pan: 0.,
        }
    }
}
impl LoopState {
    /// Preserve Go's start/stop thresholds and real-time fades while menus are up.
    pub fn set(&mut self, volume: f32, pitch: f32, pan: f32, dt: f32) {
        let k = (1. - (-LOOP_EASE * dt as f64).exp()) as f32;
        self.volume += (volume - self.volume) * k;
        self.pitch += ((if pitch == 0. { 1. } else { pitch }) - self.pitch) * (4. * k).min(1.);
        self.pan += (pan - self.pan) * k;
        if !self.playing && volume > 0.01 {
            self.playing = true;
        } else if self.playing && volume <= 0.01 && self.volume < 0.005 {
            self.playing = false;
            self.volume = 0.;
        }
    }
}

/// raylib raudio.c MixAudioFrames' stereo pan law (centre is 0.6875 per ear).
/// Applying Bevy's positional audio as well would attenuate the sound twice.
pub fn pan_levels(pan: f32) -> [f32; 2] {
    let right = (pan.clamp(-1., 1.) + 1.) / 2.;
    [1. - right, right].map(|x| 0.5 * x * (3. - x * x))
}

/// The Go cue logging switch; browser previews use `?cues=1`.
#[cfg(feature = "viewer")]
pub fn log_cues() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("EARTH_TWO_CUES").is_ok_and(|v| !v.is_empty())
    }
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.location().search().ok())
            .and_then(|s| web_sys::UrlSearchParams::new_with_str(&s).ok())
            .is_some_and(|p| p.get("cues").as_deref() == Some("1"))
    }
}
