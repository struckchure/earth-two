//! Finite desktop review with disposable keys and an explicitly local test database.
use bevy::{
    input::{
        ButtonState, InputSystems,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use earth_two_client::{
    game::{GamePlugin, GameSet, MenuAction, Screen, Session, menu::Menu},
    identity::IdentityPanel,
};
const EVIDENCE: &str = "build/migration-baseline/step-7-identity";
const PASSPHRASE: &str = "disposable identity review";
fn main() {
    let directory =
        std::path::Path::new(EVIDENCE).join(format!("native-profile-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    // SAFETY: set before Bevy starts any threads. This example never opens a player's profile.
    unsafe {
        std::env::set_var("EARTH_TWO_IDENTITY_PATH", directory.join("pk"));
    }
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        file_path: earth_two_client::physics::source_assets(),
        ..default()
    }))
    .add_plugins(GamePlugin)
    .insert_resource(earth_two_client::sky::Clock::held(11.))
    .insert_resource(earth_two_client::sky::Weather::from_setting(Some("0")))
    .init_resource::<Review>()
    .add_systems(PreUpdate, review.after(InputSystems))
    .add_systems(PostUpdate, release_pointer.after(GameSet::Menu));
    {
        let mut panel = app.world_mut().resource_mut::<IdentityPanel>();
        panel.host = "http://127.0.0.1:3017".into();
        panel.database = "earth-two-identity-review".into();
    }
    app.run();
}
#[derive(Resource, Default)]
struct Review {
    step: usize,
    age: usize,
    id: String,
}
fn release_pointer(mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut c in &mut windows {
        c.grab_mode = CursorGrabMode::None;
    }
}
fn press(world: &mut World, key: KeyCode, text: Option<&str>) {
    world.resource_mut::<ButtonInput<KeyCode>>().press(key);
    world.write_message(KeyboardInput {
        key_code: key,
        logical_key: Key::Character(text.unwrap_or("").into()),
        text: text.map(Into::into),
        state: ButtonState::Pressed,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}
fn capture(world: &mut World, name: &str) {
    use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!("{EVIDENCE}/native-{name}.png")));
}
fn focus(world: &mut World, row: usize) {
    world.write_message(MenuAction::Focus(row));
}
fn review(world: &mut World) {
    if !world.resource::<Session>().ready
        || *world.resource::<State<Screen>>().get() == Screen::Loading
    {
        return;
    }
    for mut window in world
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .iter_mut(world)
    {
        window.focused = true;
    }
    world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    world.resource_scope(|world,mut r:Mut<Review>| {
        r.age+=1;
        if r.age>3600 {panic!("identity operation did not finish");}
        if r.age==1 {match r.step {
            0=>focus(world,3),
            1|4|6|8|11|13|17|21|24|27|29|33|38=>press(world,KeyCode::Enter,None),
            2|9|36=>press(world,KeyCode::KeyX,Some(PASSPHRASE)),
            3=>focus(world,4),
            5=>focus(world,9),
            7|28=>focus(world,7),
            10=>focus(world,8),
            12=>focus(world,6),
            14=>focus(world,2),
            15=>press(world,KeyCode::KeyX,Some("review@example.com")),
            16=>focus(world,10),
            18=>focus(world,3),
            19=>press(world,KeyCode::KeyX,Some("Review Player")),
            20=>focus(world,11),
            22|25|39=>press(world,KeyCode::Escape,None),
            23|30|34=>focus(world,0),
            26=>focus(world,4),
            31=>press(world,KeyCode::KeyX,Some("wrong")),
            32|37=>focus(world,5),
            35=>{world.resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::SuperLeft);press(world,KeyCode::KeyA,None);},
            _=>{}
        }}
        if r.step == 18 && r.age == 10 {
            world.resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::SuperLeft);
            press(world, KeyCode::KeyA, None);
        }
        if r.age<20 || (r.step == 24 && r.age < 120) || world.resource::<IdentityPanel>().busy {return;}
        match r.step {
            1=>{assert_eq!(world.resource::<Menu>().screen(),Screen::Identity);capture(world,"identity");},
            4=>{let p=world.resource::<IdentityPanel>();assert!(!p.failed,"{}",p.message);r.id=p.key.as_ref().unwrap().id();assert!(p.fields[0].is_empty());capture(world,"created");},
            6=>assert!(std::path::Path::new(&world.resource::<IdentityPanel>().fields[1]).is_file()),
            8=>{assert!(world.resource::<IdentityPanel>().key.is_none());focus(world,0);},
            11=>assert_eq!(world.resource::<IdentityPanel>().key.as_ref().unwrap().id(),r.id),
            13=>{let p=world.resource::<IdentityPanel>();assert!(!p.failed,"{}",p.message);assert!(p.session.as_ref().is_some_and(|s|s.is_active()));capture(world,"connected");},
            17=>assert_eq!(world.resource::<IdentityPanel>().session.as_ref().unwrap().details().unwrap().email,"review@example.com"),
            21=>{assert_eq!(world.resource::<IdentityPanel>().session.as_ref().unwrap().details().unwrap().display_name,"Review Player");capture(world,"profile-saved");},
            22=>assert_eq!(world.resource::<Menu>().focus(),3),
            24=>{assert_eq!(world.resource::<Menu>().screen(),Screen::Playing);assert!(world.resource::<IdentityPanel>().session.as_ref().unwrap().is_active());capture(world,"playing-connected");},
            27=>{assert_eq!(world.resource::<Menu>().screen(),Screen::Identity);capture(world,"from-pause");},
            29=>{assert!(world.resource::<IdentityPanel>().key.is_none());assert!(world.resource::<IdentityPanel>().session.is_none());},
            33=>{assert!(world.resource::<IdentityPanel>().failed);assert!(world.resource::<IdentityPanel>().key.is_none());capture(world,"wrong-passphrase");},
            38=>{let p=world.resource::<IdentityPanel>();assert!(!p.failed,"{}",p.message);assert_eq!(p.key.as_ref().unwrap().id(),r.id);},
            39=>{assert_eq!(world.resource::<Menu>().screen(),Screen::Paused);assert_eq!(world.resource::<Menu>().focus(),4);},
            40=>{info!("Identity smoke passed: create/export/lock/import/connect/email/name/play/pause/wrong-passphrase/unlock/back");world.write_message(AppExit::Success);},
            _=>{}
        }
        r.step+=1;r.age=0;
    });
}
