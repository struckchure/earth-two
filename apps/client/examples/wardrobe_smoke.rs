//! Finite live wardrobe check with the real keyboard, outfits and renderer.
use bevy::{
    input::InputSystems,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use earth_two_client::{
    character::{Body, Player},
    game::{GamePlugin, Screen, Session, menu::Menu},
    presentation::{
        Outfit, Slot,
        outfit::{Garment, ModelPath},
        viewer::MeshEntities,
    },
};
fn main() {
    std::fs::create_dir_all("build/migration-baseline/step-6-wardrobe").unwrap();
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: earth_two_client::physics::source_assets(),
            ..default()
        }))
        .add_plugins(GamePlugin)
        .insert_resource(earth_two_client::sky::Clock::held(11.))
        .insert_resource(earth_two_client::sky::Weather::from_setting(Some("0")))
        .init_resource::<Review>()
        .add_systems(PreUpdate, review.after(InputSystems))
        .add_systems(PostUpdate, release_pointer)
        .run();
}
#[derive(Resource, Default)]
struct Review {
    frame: usize,
    player: Option<Entity>,
    at: Vec3,
    outfit: Outfit,
}
fn release_pointer(mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut c in &mut windows {
        c.grab_mode = CursorGrabMode::None;
        c.visible = true;
    }
}
fn key(world: &mut World, key: KeyCode) {
    world.resource_mut::<ButtonInput<KeyCode>>().press(key);
}
fn capture(world: &mut World, name: &str) {
    use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!(
            "build/migration-baseline/step-6-wardrobe/native-{name}.png"
        )));
}
fn check_clothes(world: &mut World, player: Entity) {
    let outfit = *world.get::<Outfit>(player).unwrap();
    let body = world
        .query_filtered::<(Entity, &ChildOf), With<Body>>()
        .iter(world)
        .find(|(_, p)| p.parent() == player)
        .unwrap()
        .0;
    assert_eq!(
        world
            .get::<earth_two_client::character::State>(body)
            .unwrap()
            .skin,
        outfit.body as usize
    );
    assert!(world.get::<MeshEntities>(body).is_some());
    let garments: Vec<_> = world
        .query::<(Entity, &ChildOf, &Garment)>()
        .iter(world)
        .filter(|(_, p, _)| p.parent() == body)
        .map(|(e, _, g)| (e, g.slot))
        .collect();
    assert_eq!(
        garments.len(),
        Slot::ALL
            .iter()
            .filter(|s| outfit.item(**s).is_some())
            .count()
    );
    for (entity, slot) in garments {
        assert!(
            world.get::<MeshEntities>(entity).is_some(),
            "unloaded {slot}"
        );
        let expected = &world
            .resource::<earth_two_client::presentation::Wardrobe>()
            .bodies[outfit.body as usize]
            .items(slot)[outfit.item(slot).unwrap()]
        .model;
        assert_eq!(&world.get::<ModelPath>(entity).unwrap().0, expected);
    }
}
fn review(world: &mut World) {
    if !world.resource::<Session>().ready
        || *world.resource::<State<Screen>>().get() == Screen::Loading
    {
        return;
    }
    world.resource_scope(|world,mut r:Mut<Review>| {
        r.frame+=1;world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        for mut w in world.query_filtered::<&mut Window,With<PrimaryWindow>>().iter_mut(world) {w.focused=true;}
        let player=world.resource::<Session>().player.unwrap();
        match r.frame {
            1=>{r.player=Some(player);r.at=world.get::<Transform>(player).unwrap().translation;key(world,KeyCode::ArrowDown);}
            3=>key(world,KeyCode::Enter),
            30=>{assert_eq!(*world.resource::<State<Screen>>().get(),Screen::Dressing);key(world,KeyCode::ArrowDown);}
            32=>key(world,KeyCode::ArrowRight),
            34=>key(world,KeyCode::ArrowDown),
            36=>key(world,KeyCode::Enter),
            160=>{check_clothes(world,player);capture(world,"man");assert_eq!(world.get::<Transform>(player).unwrap().translation,r.at);key(world,KeyCode::ArrowUp);}
            162=>key(world,KeyCode::ArrowUp),
            164=>key(world,KeyCode::ArrowRight),
            166|168=>key(world,KeyCode::ArrowDown),
            170=>key(world,KeyCode::Space),
            320=>{assert_eq!(world.get::<Outfit>(player).unwrap().body,1);check_clothes(world,player);capture(world,"woman");r.outfit = *world.get::<Outfit>(player).unwrap();}
            340=>key(world,KeyCode::Escape),
            342=>{assert_eq!(*world.resource::<State<Screen>>().get(),Screen::Title);assert_eq!(world.resource::<Menu>().focus(),1);key(world,KeyCode::ArrowUp);}
            344=>key(world,KeyCode::Space),
            370..=470=>{key(world,KeyCode::KeyW);}
            480=>{assert_eq!(*world.resource::<State<Screen>>().get(),Screen::Playing);assert!(world.get::<Transform>(player).unwrap().translation.distance(r.at)>1.);check_clothes(world,player);key(world,KeyCode::Escape);}
            482=>key(world,KeyCode::ArrowDown),
            484=>key(world,KeyCode::Enter),
            540=>{assert_eq!(*world.resource::<State<Screen>>().get(),Screen::Dressing);assert!(!world.resource::<Menu>().on_title());assert_eq!(*world.get::<Outfit>(player).unwrap(),r.outfit);capture(world,"paused");key(world,KeyCode::ArrowUp);}
            542=>{assert_eq!(world.resource::<Menu>().focus(),12);key(world,KeyCode::Enter);}
            544=>{assert_eq!(*world.resource::<State<Screen>>().get(),Screen::Paused);assert_eq!(world.resource::<Menu>().focus(),1);key(world,KeyCode::Backspace);}
            570=>{
                assert_eq!(*world.resource::<State<Screen>>().get(),Screen::Playing);assert_eq!(r.player,Some(player));
                assert_eq!(world.query_filtered::<Entity,With<Player>>().iter(world).count(),1);
                info!("Wardrobe smoke passed: title/pause return and focus, body/tone/look swaps, rendered garments, frozen preview, walking in changed clothes, one player");world.write_message(AppExit::Success);
            }
            _=>{}
        }
    });
}
