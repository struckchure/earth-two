//! Sequential Go bank discovery: bare name first, then contiguous _0, _1, ...;
//! OGG before WAV. Finished one-shots and their per-play pan assets are released.
use super::*;
use crate::game::cues::{Cue, CueRandom, CueSet};
pub const HIT_NAMES: [&str; 27] = [
    "step_sand",
    "step_soil",
    "step_paving",
    "step_rug",
    "step_grating",
    "land_loose",
    "land_hard",
    "slide",
    "whoosh",
    "cloth",
    "thump",
    "grab",
    "creak",
    "clank",
    "bell",
    "door",
    "crash_metal",
    "crash_ground",
    "skid",
    "ui_move",
    "ui_stamp",
    "ui_back",
    "ui_open",
    "ui_page",
    "ui_mark",
    "ui_arrive",
    "ui_deny",
];
struct Variant {
    source: AudioSource,
    voices: [Option<Entity>; 8],
}
struct Hit {
    name: &'static str,
    variants: Vec<Variant>,
    pending: Option<bevy::tasks::Task<Vec<AudioSource>>>,
}
#[derive(Resource, Default)]
pub(super) struct Hits(Vec<Hit>);
#[derive(Component)]
pub struct HitVoice {
    pub name: &'static str,
    queued_at: f64,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<Hits>()
        .add_message::<Cue>()
        .init_resource::<CueRandom>()
        .init_resource::<super::MasterGain>()
        .add_systems(Startup, load_hits)
        .add_systems(Update, discover)
        .add_systems(PostUpdate, (cleanup, play).chain().after(CueSet));
}
fn load_hits(mut bank: ResMut<Hits>, assets: Res<AssetServer>) {
    bank.0 = HIT_NAMES
        .into_iter()
        .map(|name| Hit {
            name,
            variants: vec![],
            pending: Some(optional::hit(assets.clone(), name)),
        })
        .collect();
}
fn discover(mut bank: ResMut<Hits>) {
    for hit in &mut bank.0 {
        let Some(task) = hit.pending.as_mut() else {
            continue;
        };
        if let Some(sources) = bevy::tasks::block_on(bevy::tasks::poll_once(task)) {
            hit.variants = sources
                .into_iter()
                .map(|source| Variant {
                    source,
                    voices: [None; 8],
                })
                .collect();
            hit.pending = None;
        }
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn play(
    mut commands: Commands,
    mut cues: MessageReader<Cue>,
    mut bank: ResMut<Hits>,
    voices: Query<(), With<HitVoice>>,
    mut sources: ResMut<Assets<PannedLoop>>,
    mut random: ResMut<CueRandom>,
    master: Res<super::MasterGain>,
    time: Res<Time>,
    mut logging: Local<Option<bool>>,
) {
    let logging = *logging.get_or_insert_with(crate::game::sound::log_cues);
    for hit in &mut bank.0 {
        for variant in &mut hit.variants {
            for slot in &mut variant.voices {
                if slot.is_some_and(|e| !voices.contains(e)) {
                    *slot = None;
                }
            }
        }
    }
    for cue in cues.read() {
        if cue.volume <= 0.005 {
            continue;
        }
        let Some(hit) = bank
            .0
            .iter_mut()
            .find(|h| h.name == cue.name && !h.variants.is_empty())
        else {
            continue;
        };
        let pitch = (if cue.pitch == 0. { 1. } else { cue.pitch })
            * (1. + cue.jitter * (2. * random.unit() - 1.));
        let index = (random.unit() * hit.variants.len() as f32) as usize;
        let variant = &mut hit.variants[index];
        // illusion/audio permits eight overlapping copies per clip variant.
        // When full it restarts the original (slot zero), not the oldest alias.
        let slot = variant.voices.iter().position(Option::is_none).unwrap_or(0);
        if let Some(old) = variant.voices[slot] {
            commands.entity(old).despawn();
        }
        let source = sources.add(PannedLoop {
            source: variant.source.clone(),
            pan: Arc::new(AtomicU32::new(cue.pan.to_bits())),
            repeat: false,
            master: master.0.clone(),
        });
        if logging {
            info!(
                "cue {} volume {:.2} pan {:.2} pitch {:.2} variant {}",
                cue.name, cue.volume, cue.pan, pitch, index
            );
        }
        variant.voices[slot] = Some(
            commands
                .spawn((
                    HitVoice {
                        name: cue.name,
                        queued_at: time.elapsed_secs_f64(),
                    },
                    AudioPlayer(source),
                    PlaybackSettings::ONCE
                        .with_volume(Volume::Linear(cue.volume.min(1.)))
                        .with_speed(pitch),
                ))
                .id(),
        );
    }
}
fn cleanup(
    mut commands: Commands,
    time: Res<Time>,
    voices: Query<(Entity, &HitVoice, Option<&AudioSink>)>,
) {
    for (entity, voice, sink) in &voices {
        if sink.is_some_and(AudioSink::empty)
            || (sink.is_none() && time.elapsed_secs_f64() - voice.queued_at > 1.)
        {
            commands.entity(entity).despawn();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_shot_decoder_ends_and_retains_pan() {
        let source = PannedLoop {
            source: super::super::tests::wave(),
            pan: Arc::new(AtomicU32::new(0f32.to_bits())),
            repeat: false,
            master: super::super::MasterGain::default().0,
        };
        assert_eq!(
            source.decoder().collect::<Vec<_>>(),
            vec![0.34375, 0.34375, -0.34375, -0.34375]
        );
    }
    #[test]
    fn overlapping_variants_are_bounded_and_missing_device_voices_expire() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<PannedLoop>()
            .init_resource::<CueRandom>()
            .init_resource::<super::MasterGain>()
            .add_message::<Cue>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_secs_f32(0.1),
            ))
            .insert_resource(Hits(vec![Hit {
                name: "cloth",
                variants: vec![Variant {
                    source: super::super::tests::wave(),
                    voices: [None; 8],
                }],
                pending: None,
            }]))
            .add_systems(PostUpdate, (cleanup, play).chain());
        for _ in 0..100 {
            app.world_mut()
                .write_message(crate::game::cues::Voice::ui("cloth", 1.));
        }
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&HitVoice>()
                .iter(app.world())
                .count(),
            8
        );
        for _ in 0..20 {
            app.update();
        }
        assert_eq!(
            app.world_mut()
                .query::<&HitVoice>()
                .iter(app.world())
                .count(),
            0
        );
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<Assets<PannedLoop>>().len(), 0);
    }
    #[test]
    fn all_original_hit_families_have_decodable_contiguous_variants() {
        let root = std::path::PathBuf::from(crate::physics::source_assets()).join("sounds");
        for name in HIT_NAMES {
            let read = |name: &str| {
                ["ogg", "wav"]
                    .into_iter()
                    .find_map(|ext| std::fs::read(root.join(format!("{name}.{ext}"))).ok())
            };
            let mut variants = vec![];
            if let Some(bytes) = read(name) {
                variants.push(bytes);
            } else {
                for i in 0.. {
                    let Some(bytes) = read(&format!("{name}_{i}")) else {
                        break;
                    };
                    variants.push(bytes);
                }
            }
            assert!(!variants.is_empty(), "{name}");
            for bytes in variants {
                assert!(
                    decode(AudioSource {
                        bytes: bytes.into()
                    })
                    .unwrap()
                    .next()
                    .is_some(),
                    "{name}"
                );
            }
        }
    }
}
