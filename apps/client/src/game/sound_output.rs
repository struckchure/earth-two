//! Bevy audio adapter. Decode the original clips lazily, with raylib's pan law.
use super::pan_levels;
use crate::game::{ambience::Ambience, drive_sound::DriveSound};
const NAMES: [&str; 14] = [
    "wind",
    "storm",
    "hull_hum",
    "dome_air",
    "generator",
    "fans",
    "fountain",
    "market",
    "engine_bike",
    "engine_trike",
    "engine_buggy",
    "engine_rover",
    "engine_truck",
    "tyres",
];
#[path = "sound_hits.rs"]
pub mod hits;
#[path = "sound_assets.rs"]
mod optional;
use bevy::{
    audio::{ChannelCount, SampleRate, Source, Volume},
    prelude::*,
};
use std::{
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

#[cfg(not(target_arch = "wasm32"))]
use bevy::audio::AddAudioSource;
#[cfg(target_arch = "wasm32")]
#[path = "sound_web.rs"]
mod web;

type Decoder = rodio::Decoder<Cursor<AudioSource>>;
fn decode(source: AudioSource) -> Result<Decoder, rodio::decoder::DecoderError> {
    rodio::Decoder::builder()
        .with_byte_len(source.bytes.len() as u64)
        .with_data(Cursor::new(source))
        .build()
}

#[derive(Asset, TypePath)]
pub struct PannedLoop {
    source: AudioSource,
    pan: Arc<AtomicU32>,
    repeat: bool,
}
/// Repeat the decoder, not already-panned samples, so panning still changes
/// after a complete lap. This also avoids caching an entire decoded loop.
pub struct PannedDecoder {
    source: AudioSource,
    decoder: Option<Decoder>,
    pan: Arc<AtomicU32>,
    right: Option<f32>,
    rate: SampleRate,
    repeat: bool,
}
impl Decodable for PannedLoop {
    type Decoder = PannedDecoder;
    fn decoder(&self) -> PannedDecoder {
        let decoder = decode(self.source.clone()).ok();
        let rate = decoder
            .as_ref()
            .map_or(SampleRate::new(44100).unwrap(), Source::sample_rate);
        PannedDecoder {
            source: self.source.clone(),
            decoder,
            pan: self.pan.clone(),
            right: None,
            rate,
            repeat: self.repeat,
        }
    }
}
impl Iterator for PannedDecoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        let decoder = self.decoder.as_mut()?;
        let left = if let Some(sample) = decoder.next() {
            sample
        } else if self.repeat {
            self.decoder = decode(self.source.clone()).ok();
            self.decoder.as_mut()?.next()?
        } else {
            return None;
        };
        let decoder = self.decoder.as_mut()?;
        let channels = decoder.channels().get();
        let right = if channels == 1 { left } else { decoder.next()? };
        for _ in 2..channels {
            decoder.next()?;
        }
        let levels = pan_levels(f32::from_bits(self.pan.load(Ordering::Relaxed)));
        self.right = Some(right * levels[1]);
        Some(left * levels[0])
    }
}
impl Source for PannedDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[derive(Component)]
pub struct LoopVoice(pub usize);
struct Track {
    pending: Option<bevy::tasks::Task<Option<AudioSource>>>,
    source: Option<Handle<PannedLoop>>,
    pan: Arc<AtomicU32>,
    entity: Option<Entity>,
    failed: bool,
    reported: bool,
}
#[derive(Resource, Default)]
struct Bank(Vec<Track>);
pub struct SoundOutputPlugin;
impl Plugin for SoundOutputPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(not(target_arch = "wasm32"))]
        app.add_audio_source::<PannedLoop>();
        #[cfg(target_arch = "wasm32")]
        web::install(app);
        hits::install(app);
        app.init_resource::<Bank>()
            .add_systems(Startup, load)
            .add_systems(
                Update,
                output
                    .after(super::super::ambience::mix)
                    .after(super::super::drive_sound::drive_cues),
            );
    }
}
fn load(mut bank: ResMut<Bank>, assets: Res<AssetServer>) {
    bank.0 = NAMES
        .iter()
        .map(|name| Track {
            pending: Some(optional::track(assets.clone(), name)),
            source: None,
            pan: Arc::new(AtomicU32::new(0f32.to_bits())),
            entity: None,
            failed: false,
            reported: false,
        })
        .collect();
}
#[allow(clippy::too_many_arguments)]
fn output(
    mut commands: Commands,
    mut bank: ResMut<Bank>,
    mut sources: ResMut<Assets<PannedLoop>>,
    ambience: Res<Ambience>,
    driving: Option<Res<DriveSound>>,
    mut voices: Query<Option<&mut AudioSink>, With<LoopVoice>>,
    mut logging: Local<Option<bool>>,
) {
    let logging = *logging.get_or_insert_with(super::log_cues);
    for (i, track) in bank.0.iter_mut().enumerate() {
        if track.failed {
            continue;
        }
        if let Some(task) = track.pending.as_mut()
            && let Some(result) = bevy::tasks::block_on(bevy::tasks::poll_once(task))
        {
            track.pending = None;
            if let Some(source) = result {
                track.source = Some(sources.add(PannedLoop {
                    source,
                    pan: track.pan.clone(),
                    repeat: true,
                }));
            } else {
                warn!("sound {} is unavailable; leaving it silent", NAMES[i]);
                track.failed = true;
                continue;
            }
        }
        let state = if i < 8 {
            &ambience.loops[i]
        } else if let Some(d) = &driving {
            &d.loops[i - 8]
        } else {
            continue;
        };
        track.pan.store(state.pan.to_bits(), Ordering::Relaxed);
        if let Some(entity) = track.entity {
            match voices.get_mut(entity) {
                Ok(sink) if state.playing => {
                    if let Some(mut sink) = sink {
                        if logging && !track.reported && sink.position().as_secs_f32() > 2. {
                            info!(
                                "loop {} advancing: position {:.2}s volume {:.3} pan {:.3}",
                                NAMES[i],
                                sink.position().as_secs_f32(),
                                state.volume,
                                state.pan
                            );
                            track.reported = true;
                        }
                        sink.set_volume(Volume::Linear(state.volume.min(1.)));
                        sink.set_speed(state.pitch);
                    }
                    // Keep queued playback current when an audio device or file is delayed.
                    commands.entity(entity).insert(
                        PlaybackSettings::ONCE
                            .with_volume(Volume::Linear(state.volume.min(1.)))
                            .with_speed(state.pitch),
                    );
                }
                Ok(_) => {
                    commands.entity(entity).despawn();
                    track.entity = None;
                }
                Err(_) => {
                    track.entity = None;
                }
            }
        }
        if track.entity.is_none()
            && state.playing
            && let Some(source) = &track.source
        {
            track.reported = false;
            if logging {
                info!("loop {} on", NAMES[i]);
            }
            track.entity = Some(
                commands
                    .spawn((
                        LoopVoice(i),
                        AudioPlayer(source.clone()),
                        PlaybackSettings::ONCE
                            .with_volume(Volume::Linear(state.volume.min(1.)))
                            .with_speed(state.pitch),
                    ))
                    .id(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn wave() -> AudioSource {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&22050u32.to_le_bytes());
        bytes.extend_from_slice(&44100u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&16384i16.to_le_bytes());
        bytes.extend_from_slice(&(-16384i16).to_le_bytes());
        AudioSource {
            bytes: bytes.into(),
        }
    }
    #[test]
    fn looping_decoder_keeps_live_pan_after_wrap_and_preserves_pitch_rate() {
        let pan = Arc::new(AtomicU32::new((-1f32).to_bits()));
        let source = PannedLoop {
            source: wave(),
            pan: pan.clone(),
            repeat: true,
        };
        let mut decoder = source.decoder();
        assert_eq!(decoder.sample_rate().get(), 22050);
        assert_eq!(decoder.channels().get(), 2);
        assert_eq!(
            decoder.by_ref().take(4).collect::<Vec<_>>(),
            vec![0.5, 0., -0.5, -0.]
        );
        pan.store(1f32.to_bits(), Ordering::Relaxed);
        assert_eq!(
            decoder.by_ref().take(4).collect::<Vec<_>>(),
            vec![0., 0.5, -0., -0.5]
        );
        pan.store(0f32.to_bits(), Ordering::Relaxed);
        assert_eq!(
            decoder.by_ref().take(4).collect::<Vec<_>>(),
            vec![0.34375, 0.34375, -0.34375, -0.34375]
        );
    }
    #[test]
    fn original_ambience_files_decode_and_have_samples() {
        for name in NAMES {
            let bytes = std::fs::read(format!(
                "{}/sounds/{name}.wav",
                crate::physics::source_assets()
            ))
            .unwrap();
            let mut source = decode(AudioSource {
                bytes: bytes.into(),
            })
            .unwrap();
            assert!(
                source.by_ref().take(48000).any(|x| x.abs() > 0.001),
                "{name} is silent"
            );
        }
    }
    #[test]
    fn queued_loops_despawn_and_recreate_without_an_audio_device() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<AudioSource>()
            .init_asset::<PannedLoop>()
            .init_resource::<Ambience>()
            .add_systems(Update, output);
        let pan = Arc::new(AtomicU32::new(0));
        let handle = app
            .world_mut()
            .resource_mut::<Assets<PannedLoop>>()
            .add(PannedLoop {
                source: wave(),
                pan: pan.clone(),
                repeat: true,
            });
        app.insert_resource(Bank(vec![Track {
            pending: None,
            source: Some(handle),
            pan,
            entity: None,
            failed: false,
            reported: false,
        }]));
        for _ in 0..3 {
            app.world_mut().resource_mut::<Ambience>().loops[0].set(0.5, 1., 0., 1.);
            app.update();
            app.update();
            assert_eq!(
                app.world_mut()
                    .query::<&LoopVoice>()
                    .iter(app.world())
                    .count(),
                1
            );
            app.world_mut().resource_mut::<Ambience>().loops[0].playing = false;
            app.update();
            app.update();
            assert_eq!(
                app.world_mut()
                    .query::<&LoopVoice>()
                    .iter(app.world())
                    .count(),
                0
            );
        }
        assert_eq!(app.world().resource::<Assets<PannedLoop>>().len(), 1);
    }
    #[test]
    fn invalid_audio_is_silent_instead_of_panicking() {
        let source = PannedLoop {
            source: AudioSource {
                bytes: vec![0; 10].into(),
            },
            pan: Arc::new(AtomicU32::new(0)),
            repeat: true,
        };
        assert_eq!(source.decoder().next(), None);
    }
}
