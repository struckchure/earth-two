//! Own CPAL's browser stream so a real input callback can resume its context.
//! Bevy 0.20's private AudioOutput doesn't expose that context. We still use
//! Bevy audio assets/sinks and the same Rodio mixer and decoders as native.
use super::PannedLoop;
use bevy::prelude::*;
use rodio::{
    cpal::{
        self,
        traits::{DeviceTrait, HostTrait, StreamTrait},
    },
    mixer::Mixer,
};
use wasm_bindgen::{JsCast, closure::Closure};

struct BrowserOutput {
    stream: cpal::Stream,
    mixer: Mixer,
    window: web_sys::Window,
    unlock: Closure<dyn FnMut()>,
}
impl Drop for BrowserOutput {
    fn drop(&mut self) {
        for event in ["keydown", "pointerdown"] {
            let _ = self
                .window
                .remove_event_listener_with_callback(event, self.unlock.as_ref().unchecked_ref());
        }
        let _ = self.stream.pause();
    }
}
fn open() -> Result<BrowserOutput, String> {
    let host = cpal::platform::WebAudioHost::new().map_err(|e| e.to_string())?;
    let device = host
        .default_output_device()
        .ok_or("No browser audio device")?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let config = supported.config();
    let (mixer, mut samples) = rodio::mixer::mixer(
        bevy::audio::ChannelCount::new(config.channels).ok_or("No output channels")?,
        bevy::audio::SampleRate::new(config.sample_rate).ok_or("No output sample rate")?,
    );
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                for sample in data {
                    *sample = samples.next().unwrap_or(0.);
                }
            },
            |error| warn!("browser audio: {error}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    let context = stream.audio_context().clone();
    let window = web_sys::window().ok_or("No browser window")?;
    let unlock = Closure::wrap(Box::new(move || {
        if context.state() != web_sys::AudioContextState::Running {
            // Call synchronously in the trusted gesture; do not defer to Update.
            match context.resume() {
                Ok(promise) => wasm_bindgen_futures::spawn_local(async move {
                    if let Err(error) = wasm_bindgen_futures::JsFuture::from(promise).await {
                        warn!("browser audio resume failed: {error:?}");
                    } else if crate::game::sound::log_cues() {
                        info!("browser audio context resumed by user gesture");
                    }
                }),
                Err(error) => warn!("browser audio resume failed: {error:?}"),
            }
        }
    }) as Box<dyn FnMut()>);
    let output = BrowserOutput {
        stream: stream.into(),
        mixer,
        window,
        unlock,
    };
    for event in ["keydown", "pointerdown"] {
        output
            .window
            .add_event_listener_with_callback(event, output.unlock.as_ref().unchecked_ref())
            .map_err(|e| format!("{e:?}"))?;
    }
    output.stream.play().map_err(|e| e.to_string())?;
    Ok(output)
}
fn setup(world: &mut World) {
    match open() {
        Ok(output) => world.insert_non_send(output),
        Err(error) => warn!("browser audio unavailable: {error}"),
    }
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<GlobalVolume>()
        .init_asset::<AudioSource>()
        .init_asset_loader::<bevy::audio::AudioLoader>()
        .init_asset::<PannedLoop>()
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, play_queued.after(super::hits::play));
}
fn play_queued(
    mut commands: Commands,
    output: Option<NonSend<BrowserOutput>>,
    sources: Res<Assets<PannedLoop>>,
    volume: Res<GlobalVolume>,
    voices: Query<(Entity, &AudioPlayer<PannedLoop>, &PlaybackSettings), Without<AudioSink>>,
) {
    let Some(output) = output else {
        return;
    };
    for (entity, handle, settings) in &voices {
        let Some(source) = sources.get(&handle.0) else {
            continue;
        };
        let player = rodio::Player::connect_new(&output.mixer);
        player.set_volume(settings.volume.to_linear() * volume.volume.to_linear());
        player.set_speed(settings.speed);
        player.append(source.decoder());
        commands.entity(entity).insert(AudioSink::new(player));
    }
}
