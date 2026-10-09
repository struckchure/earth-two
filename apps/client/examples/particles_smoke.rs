//! Real particle emission and analytic material smoke, plus an explicit spark preview.
use bevy::{
    input::InputSystems,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use earth_two_client::{
    character::{CharacterController, Player},
    game::{GamePlugin, MenuAction, Screen, Session, StartAt},
    vehicle::Driving,
};

fn main() {
    std::fs::create_dir_all("build/migration-baseline/step-5-particles").unwrap();
    let bike = std::env::args().any(|a| a == "--bike");

    App::new()
        .add_plugins(
            DefaultPlugins.set(AssetPlugin {
                file_path: std::env::var("EARTH_TWO_ASSET_ROOT")
                    .unwrap_or_else(|_| earth_two_client::physics::source_assets()),
                ..default()
            }),
        )
        .insert_resource(if bike { StartAt::Bike } else { StartAt::Buggy })
        .add_plugins(GamePlugin)
        .init_resource::<Review>()
        .insert_resource(earth_two_client::sky::Clock::held(11.))
        .insert_resource(earth_two_client::sky::Weather::from_setting(Some("0")))
        .init_resource::<Heard>()
        .init_resource::<ParticlesSeen>()
        .add_systems(
            PostUpdate,
            see_particles.after(earth_two_client::sky::effects::EffectsSystems),
        )
        .add_systems(PostUpdate, hear.after(earth_two_client::game::cues::CueSet))
        .add_systems(PreUpdate, review.after(InputSystems))
        .add_systems(PostUpdate, release_pointer)
        .run();
}
#[derive(Resource, Default)]
struct Review {
    frame: usize,
    player: Option<Entity>,
    before: Vec3,
    parked: Vec3,
}
fn release_pointer(mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut cursor in &mut windows {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}
fn review(world: &mut World) {
    if !world.resource::<Session>().ready
        || *world.resource::<State<Screen>>().get() == Screen::Loading
    {
        return;
    }
    world.resource_scope(|world, mut review: Mut<Review>| {
        review.frame += 1;
        let frame = review.frame;
        let player = world.resource::<Session>().player.unwrap();
        for mut window in world
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .iter_mut(world)
        {
            window.focused = true; // No focus dependency for this synthetic-input test.
        }
        world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        let key = match frame {
            120 | 540 => Some(KeyCode::KeyE),
            710 => Some(KeyCode::Space),
            180..300 | 600..720 => Some(KeyCode::KeyW),
            300..540 => Some(KeyCode::Space),
            _ => None,
        };
        if let Some(key) = key {
            world.resource_mut::<ButtonInput<KeyCode>>().press(key);
        }
        match frame {
            2 => {
                assert_eq!(*world.resource::<State<Screen>>().get(), Screen::Title);
                review.player = Some(player);
                world.write_message(MenuAction::Play);
            }
            180 => {
                let car = world
                    .resource::<Driving>()
                    .vehicle
                    .expect("entered vehicle");
                assert!(world.get::<CharacterController>(player).is_none());
                review.before = world.get::<Transform>(car).unwrap().translation;
                if matches!(*world.resource::<StartAt>(), StartAt::Bike) {
                    assert_eq!(
                        world
                            .get::<earth_two_client::vehicle::Drivable>(car)
                            .unwrap()
                            .name,
                        "bike"
                    );
                    assert!(
                        world
                            .query::<&earth_two_client::presentation::pose::PoseFit>()
                            .iter(world)
                            .any(|p| p.riding && p.frames > 20),
                        "live bike pose was not applied"
                    );

                }
                info!("Game smoke: entered vehicle");
            }
            280 => {
                use bevy::render::view::window::screenshot::{Screenshot,save_to_disk};
                world.spawn(Screenshot::primary_window()).observe(save_to_disk("build/migration-baseline/step-5-particles/native-wheel.png"));
            }
            300 => {
                let car = world.resource::<Driving>().vehicle.unwrap();
                assert!(
                    world
                        .get::<Transform>(car)
                        .unwrap()
                        .translation
                        .distance(review.before)
                        > 2.0
                );
                info!("Game smoke: drove vehicle");
            }
            600 => {
                assert!(!world.resource::<Driving>().active(), "exit after braking");
                assert!(world.get::<CharacterController>(player).is_some());
                review.before = world.get::<Transform>(player).unwrap().translation;
            }
            720 => {
                assert!(
                    world
                        .get::<Transform>(player)
                        .unwrap()
                        .translation
                        .distance(review.before)
                        > 0.5
                );
                world.write_message(MenuAction::Pause);
                assert!(
                    world
                        .query::<&earth_two_client::presentation::pose::PoseFit>()
                        .iter(world)
                        .all(|p| !p.riding),
                    "riding pose survived exit"
                );
                info!("Game smoke: exited and walked");
            }
            725 => review.parked = world.get::<Transform>(player).unwrap().translation,
            780 => {
                assert_eq!(*world.resource::<State<Screen>>().get(), Screen::Paused);
                assert_eq!(
                    world.get::<Transform>(player).unwrap().translation,
                    review.parked
                );
                world.write_message(MenuAction::MainMenu);
            }
            840 => {
                assert_eq!(*world.resource::<State<Screen>>().get(), Screen::Title);
                world.write_message(MenuAction::Play);
            }
            900 => {
                assert_eq!(*world.resource::<State<Screen>>().get(), Screen::Playing);
                assert_eq!(Some(player), review.player);
                assert_eq!(
                    world
                        .query_filtered::<Entity, With<Player>>()
                        .iter(world)
                        .count(),
                    1
                );
                assert!(world.query::<&earth_two_client::game::sound::output::LoopVoice>().iter(world).all(|v| v.0 < 8), "vehicle loops survived exit");
                let heard = world.resource::<Heard>();
                let engine = if matches!(*world.resource::<StartAt>(),StartAt::Bike) { "cloth" } else { "door" };
                assert!(heard.cues.contains(engine), "entry cue missing: {:?}", heard.cues);
                assert!(heard.cues.iter().any(|n| n.starts_with("step_")), "no rendered foot plants: {:?}",heard.cues);
                assert!(heard.cues.contains("cloth"), "jump/body cue missing");
                assert!(heard.cues.contains("ui_stamp") && heard.cues.contains("ui_page"), "menu cues missing");
                assert!(heard.engine_advanced, "no advancing engine sink");
                assert!(heard.hit_advanced, "no advancing one-shot sink");
                info!("Cue smoke passed: {:?}; engine and one-shot sinks advanced, vehicle loops released",heard.cues);
                let seen=world.resource::<ParticlesSeen>();
                assert!(seen.foot && seen.wheel && seen.ring,"missing emission: foot={} wheel={} ring={}",seen.foot,seen.wheel,seen.ring);
                info!("Particle gameplay passed: rendered foot, wheel and landing dust; max {} live",seen.peak);
                let camera=*world.query_filtered::<&Transform,With<earth_two_client::game::camera::GameCamera>>().single(world).unwrap();
                earth_two_client::game::effects::crash(&mut world.resource_mut::<earth_two_client::sky::Effects>(),camera.translation+*camera.forward()*8.,-*camera.right(),14.5,false);
            }
            905 => {
                use bevy::render::view::window::screenshot::{Screenshot,save_to_disk};
                world.spawn(Screenshot::primary_window()).observe(save_to_disk("build/migration-baseline/step-5-particles/native-sparks.png"));
            }
            1150 => {
                assert!(world.resource::<earth_two_client::sky::Effects>().ps.is_empty(),"particles did not expire");
                let meshes:Vec<_>=world.query_filtered::<(&Mesh3d,&Visibility),With<earth_two_client::sky::render::EffectsMesh>>().iter(world).map(|(m,v)|(m.clone(),*v)).collect();
                assert_eq!(meshes.len(),1);assert_eq!(meshes[0].1,Visibility::Hidden);
                let mesh=world.resource::<Assets<Mesh>>().get(&meshes[0].0.0).unwrap();
                assert_eq!(mesh.count_vertices(),0);assert_eq!(mesh.indices().unwrap().len(),0);
                info!("Particle smoke passed: live emissions, single mesh, shader rendering, expiry and geometry cleanup");
                world.write_message(AppExit::Success);
            }
            _ => {}
        }
    });
}

#[derive(Resource, Default)]
struct Heard {
    cues: std::collections::HashSet<&'static str>,
    engine_advanced: bool,
    hit_advanced: bool,
}
fn hear(
    mut heard: ResMut<Heard>,
    mut cues: MessageReader<earth_two_client::game::cues::Cue>,
    loops: Query<(
        &earth_two_client::game::sound::output::LoopVoice,
        &AudioSink,
    )>,
    hits: Query<(
        &earth_two_client::game::sound::output::hits::HitVoice,
        &AudioSink,
    )>,
) {
    for cue in cues.read() {
        heard.cues.insert(cue.name);
    }
    heard.engine_advanced |= loops.iter().any(|(v, s)| {
        (8..13).contains(&v.0) && s.position().as_secs_f32() > 0.1 && s.volume().to_linear() > 0.01
    });
    heard.hit_advanced |= hits.iter().any(|(_, s)| s.position().as_secs_f32() > 0.01);
}

#[derive(Resource, Default)]
struct ParticlesSeen {
    foot: bool,
    wheel: bool,
    ring: bool,
    peak: usize,
}
fn see_particles(fx: Res<earth_two_client::sky::Effects>, mut seen: ResMut<ParticlesSeen>) {
    seen.peak = seen.peak.max(fx.ps.len());
    assert!(fx.ps.len() <= earth_two_client::sky::effects::MAX_PARTICLES);
    for p in &fx.ps {
        seen.foot |= p.size0 == 0.1;
        seen.wheel |= p.size0 == 0.25;
        seen.ring |= p.size0 == 0.15;
    }
}
