//! Real game/audio integration: Hull, menu duck, resume, outside, and return.
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use earth_two_client::{
    game::{GamePlugin, MenuAction, Screen, Session, StartAt, sound::output::LoopVoice},
    sky::{Clock, Weather},
};
#[derive(Resource, Default)]
struct Review {
    stage: u8,
    since: f32,
    voice: Option<Entity>,
}
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: earth_two_client::physics::source_assets(),
            ..default()
        }))
        .insert_resource(StartAt::Hull)
        .add_plugins(GamePlugin)
        .insert_resource(Clock::held(11.))
        .insert_resource(Weather::from_setting(Some("0")))
        .init_resource::<Review>()
        .add_systems(Update, review)
        .run();
}
fn review(world: &mut World) {
    for mut c in world
        .query_filtered::<&mut CursorOptions, With<PrimaryWindow>>()
        .iter_mut(world)
    {
        c.grab_mode = CursorGrabMode::None;
        c.visible = true;
    }
    for mut w in world
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .iter_mut(world)
    {
        w.focused = true;
    }
    if !world.resource::<Session>().ready
        || *world.resource::<State<Screen>>().get() == Screen::Loading
    {
        return;
    }
    world.resource_scope(|world, mut r: Mut<Review>| {
        let now = world.resource::<Time<Real>>().elapsed_secs();
        let screen = *world.resource::<State<Screen>>().get();
        let sounds: Vec<_> = world.query::<(Entity, &LoopVoice, &AudioSink)>().iter(world).map(|(e, voice, sink)| (e, voice.0, sink.volume().to_linear(), sink.position().as_secs_f32())).collect();
        assert!(sounds.len() <= 8, "duplicate ambience voices");
        let hull = sounds.iter().find(|s| s.1 == 2).copied();
        if r.stage == 1 && now - r.since > 4. {
            let player = world.resource::<Session>().player.unwrap();
            info!("Audio review player {:?}", world.get::<Transform>(player));
            for t in world.query_filtered::<&Transform, With<earth_two_client::game::camera::GameCamera>>().iter(world) {
                info!("Audio review camera {:?}, zones {:?}", t.translation, earth_two_client::landfall::zones::places_at(t.translation));
            }
        }
        match r.stage {
            0 if screen == Screen::Title => { world.write_message(MenuAction::Play); r.stage = 1; r.since = now; }
            1 if now - r.since > 4. => {
                let (e, _, volume, position) = hull.expect("Hull sound has no device sink");
                assert!(volume > 0.52 && position > 1., "Hull voice not playing: {volume}, {position}");
                r.voice = Some(e); info!("Ambience smoke: Hull audible volume={volume} position={position}");
                world.write_message(MenuAction::Pause); r.stage = 2; r.since = now;
            }
            2 if now - r.since > 3. => {
                assert_eq!(screen, Screen::Paused);
                let (e, _, volume, _) = hull.unwrap(); assert_eq!(Some(e), r.voice);
                assert!((volume - 0.22).abs() < 0.015, "menu duck {volume}");
                info!("Ambience smoke: same loop ducked to {volume}");
                world.write_message(MenuAction::Resume); r.stage = 3; r.since = now;
            }
            3 if now - r.since > 3. => {
                let (e, _, volume, _) = hull.unwrap(); assert_eq!(Some(e), r.voice); assert!(volume > 0.52);
                info!("Ambience smoke: resumed same loop at {volume}");
                move_player(world, Vec3::new(100., 0., 100.)); r.stage = 4; r.since = now;
            }
            4 if now - r.since > 5. => {
                assert!(hull.is_none(), "Hull loop was not released outside");
                let wind = sounds.iter().find(|s| s.1 == 0).expect("outside wind sink");
                assert!(wind.2 > 0.39 && wind.3 > 1.);
                info!("Ambience smoke: outside wind replaces Hull loop");
                move_player(world, Vec3::new(-15., 0., 6.)); r.stage = 5; r.since = now;
            }
            5 if now - r.since > 5. => {
                let (e, _, volume, position) = hull.expect("Hull loop not recreated");
                assert_ne!(Some(e), r.voice); assert!(volume > 0.52 && position > 1.);
                info!("Ambience smoke passed: playing sinks, menu duck/resume, zone fade/despawn/recreation");
                world.write_message(AppExit::Success); r.stage = 6;
            }
            _ => {}
        }
        assert!(now - r.since < 30., "ambience smoke timeout stage {}", r.stage);
    });
}
fn move_player(world: &mut World, at: Vec3) {
    let e = world.resource::<Session>().player.unwrap();
    let y = earth_two_world::terrain::walk_height(at.x, at.z) + 0.95;
    world.get_mut::<Transform>(e).unwrap().translation = Vec3::new(at.x, y, at.z);
    if let Some(mut cc) = world.get_mut::<earth_two_client::character::CharacterController>(e) {
        cc.velocity = Vec3::ZERO;
    }
}
