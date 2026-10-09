//! Finite native review: real keyboard navigation, menus and each vehicle camera.
use bevy::{
    input::InputSystems,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use earth_two_client::{
    game::{
        GamePlugin, Screen, Session, StartAt,
        camera::{GameCamera, Orbit},
        menu::Menu,
    },
    vehicle::Driving,
};
const EVIDENCE: &str = "build/migration-baseline/step-6-menus";
fn main() {
    std::fs::create_dir_all(EVIDENCE).unwrap();
    let name = std::env::args()
        .find_map(|a| a.strip_prefix("--vehicle=").map(str::to_owned))
        .unwrap_or("hauler".into());
    let start = match name.as_str() {
        "bike" => StartAt::Bike,
        "trike" => StartAt::Trike,
        "buggy" => StartAt::Buggy,
        "rover" => StartAt::Rover,
        "hauler_tanker" => StartAt::HaulerTanker,
        _ => StartAt::Hauler,
    };
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: earth_two_client::physics::source_assets(),
            ..default()
        }))
        .insert_resource(start)
        .add_plugins(GamePlugin)
        .insert_resource(earth_two_client::sky::Clock::held(11.))
        .insert_resource(earth_two_client::sky::Weather::from_setting(Some("0")))
        .insert_resource(Review {
            name,
            frame: 0,
            parked: Vec3::ZERO,
        })
        .add_systems(PreUpdate, review.after(InputSystems))
        .add_systems(PostUpdate, release_pointer)
        .run();
}
#[derive(Resource)]
struct Review {
    name: String,
    frame: usize,
    parked: Vec3,
}
fn release_pointer(mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut c in &mut windows {
        c.grab_mode = CursorGrabMode::None;
    }
}
fn capture(world: &mut World, name: &str) {
    use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!("{EVIDENCE}/native-{name}.png")));
}
fn review(world: &mut World) {
    if !world.resource::<Session>().ready
        || *world.resource::<State<Screen>>().get() == Screen::Loading
    {
        return;
    }
    world.resource_scope(|world,mut r:Mut<Review>| {
        r.frame+=1;
        for mut w in world.query_filtered::<&mut Window,With<PrimaryWindow>>().iter_mut(world) {w.focused=true;}
        world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        let key=match r.frame {
            80|100|440|460=>Some(KeyCode::ArrowDown),
            120|280|480|1340=>Some(KeyCode::Enter),
            200|380|550|580|1290=>Some(KeyCode::Escape),
            240|260|1320=>Some(KeyCode::ArrowUp),
            640=>Some(KeyCode::KeyE),
            800..1080=>Some(KeyCode::KeyW),
            1080..1240=>Some(KeyCode::Space),
            _=>None,
        };
        if let Some(key)=key {world.resource_mut::<ButtonInput<KeyCode>>().press(key);}
        match r.frame {
            60=>{
                let titles:Vec<_>=world.query::<(&Text,&InheritedVisibility,&ComputedNode)>().iter(world)
                    .filter(|(t,_,_)|t.0=="Earth Two" || t.0=="Play").collect();
                assert_eq!(titles.len(),2);
                assert!(titles.iter().all(|(_,v,n)|v.get() && n.size().x>0.));
                capture(world,&format!("{}-title",r.name));
            },
            180|520=>{assert_eq!(*world.resource::<State<Screen>>().get(),Screen::Controls);capture(world,&format!("{}-controls-{}",r.name,r.frame));},
            220=>{assert_eq!(*world.resource::<State<Screen>>().get(),Screen::Title);assert_eq!(world.resource::<Menu>().focus(),2);},
            300=>capture(world,&format!("{}-admitted",r.name)),
            420=>{assert_eq!(*world.resource::<State<Screen>>().get(),Screen::Paused);capture(world,&format!("{}-pause",r.name));},
            600=>{
                assert_eq!(*world.resource::<State<Screen>>().get(),Screen::Playing);
                // Move this review rig to open terrain so nearby parked vehicles
                // and structures do not obstruct the driving check;
                // ordinary --at routes retain the original world placements.
                let player=world.resource::<Session>().player.unwrap();
                let at=world.get::<Transform>(player).unwrap().translation;
                let car=world.query::<(Entity,&earth_two_client::vehicle::Drivable,&Transform)>().iter(world)
                    .filter(|(_,d,_)|d.name==r.name).min_by(|(_,_,a),(_,_,b)|a.translation.distance_squared(at).total_cmp(&b.translation.distance_squared(at)))
                    .unwrap().0;
                let old=world.get::<Transform>(car).unwrap().translation;
                let delta=Vec3::new(100.,earth_two_world::terrain::walk_height(100.,100.)+0.1,100.)-old;
                for e in [car,player] {
                    world.get_mut::<Transform>(e).unwrap().translation+=delta;
                    if let Some(mut p)=world.get_mut::<avian3d::prelude::Position>(e) {p.0+=delta;}
                }
            },
            700=>{
                let d=world.resource::<Driving>();
                assert_eq!(d.name,r.name,"entered expected vehicle");
                let car=d.vehicle.expect("entered vehicle");
                r.parked=world.get::<Transform>(car).unwrap().translation;
                // Start behind the chassis; later motion exercises automatic chase.
                let turn=world.get::<Transform>(car).unwrap().rotation*Vec3::Z;
                let mut orbit=world.resource_mut::<Orbit>();
                orbit.yaw=turn.x.atan2(turn.z)+std::f32::consts::PI;
                orbit.pitch=12f32.to_radians();
                orbit.target=None;
            },
            760|1240=>{
                let player=world.resource::<Session>().player.unwrap();
                let p=world.get::<Transform>(player).unwrap().translation;
                let camera=world.query_filtered::<&Transform,With<GameCamera>>().single(world).unwrap();
                assert!(camera.translation.is_finite());
                assert!(camera.translation.distance(p)>2.,"{} camera collapsed into vehicle: {:?} vs {p:?}",r.name,camera.translation);
                capture(world,&format!("{}-camera-{}",r.name,r.frame));
            },
            1090=>{
                let d=world.resource::<Driving>();
                let at=world.get::<Transform>(d.vehicle.unwrap()).unwrap().translation;
                info!("Driving review {}: displacement {} speed {} parked {:?} at {:?}",r.name,at.distance(r.parked),d.speed,r.parked,at);
                assert!(at.distance(r.parked)>1.,"{} did not drive",r.name);
            },
            1330=>{assert!(matches!(world.resource::<Menu>().press(),Some(earth_two_client::game::MenuAction::Quit)));info!("Menus/camera smoke passed for {}: title, controls, back, play, pause, controls, resume, enter, drive, chase, Quit",r.name);},
            1390=>panic!("native Quit did not close application"),
            _=>{}
        }
    });
}
