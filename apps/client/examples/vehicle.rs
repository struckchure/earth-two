//! Vehicle review scene; the integrated game remains the next migration phase.
//! cargo run --locked -p earth-two-client --example vehicle -- bike
use avian3d::prelude::*;
use bevy::{asset::AssetPlugin, prelude::*};
use earth_two_client::{
    character::{CharacterPlugin, Health, Player},
    physics::{EarthPhysicsPlugin, source_assets},
    vehicle::{
        self, Controls, Drivable, Driving, EnteredVehicle, Seats, VehiclePlugin, VehicleState,
        VehicleSystems, WheelOf,
    },
    world::{Wheel, WorldPlugin},
};
use earth_two_world::kit::Kit;

#[derive(Resource)]
struct Review {
    kit: Kit,
    name: String,
    driver: Option<Entity>,
    started: bool,
}
#[derive(Component)]
struct FollowCamera;
#[derive(Component)]
struct Hud;

fn main() {
    let kit = Kit::load(source_assets(), "world/world.json").unwrap();
    let name = std::env::args().nth(1).unwrap_or_else(|| "bike".into());
    if !kit.pieces.get(&name).is_some_and(|p| p.vehicle.is_some()) {
        eprintln!("Choose bike, buggy, trike, rover, hauler, or hauler_tanker");
        std::process::exit(2);
    }
    App::new()
        .insert_resource(Review {
            kit,
            name,
            driver: None,
            started: false,
        })
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: source_assets(),
            ..default()
        }))
        .add_plugins((
            EarthPhysicsPlugin,
            WorldPlugin,
            CharacterPlugin,
            VehiclePlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                input.before(VehicleSystems::Drive),
                (start_driving, models).after(VehicleSystems::Drive),
                camera_and_hud.after(VehicleSystems::Drive),
                capture,
            ),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut review: ResMut<Review>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(400.0, 1.0, 400.0),
        Friction::new(0.9),
        Mesh3d(meshes.add(Cuboid::new(400.0, 1.0, 400.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.45, 0.28, 0.20))),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
    let spec = review.kit.pieces[&review.name].vehicle.as_ref().unwrap();
    let car = vehicle::spawn_parked(
        &mut commands,
        &review.name,
        spec,
        Vec3::ZERO,
        Quat::IDENTITY,
    );
    commands.entity(car).insert(Visibility::default());
    review.driver = Some(
        commands
            .spawn((Player, Health::default(), Transform::default()))
            .id(),
    );
    commands.spawn((
        Camera3d::default(),
        FollowCamera,
        Transform::from_xyz(6.0, 3.5, -8.0).looking_at(Vec3::Y, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 15000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 10.0, -4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Text::new("Loading vehicle…"),
        Hud,
        TextFont {
            font_size: px(20).into(),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: px(20),
            top: px(20),
            ..default()
        },
    ));
}

fn start_driving(
    mut commands: Commands,
    mut review: ResMut<Review>,
    mut cars: Query<(Entity, &mut Drivable, &Seats, &Transform)>,
    mut driving: ResMut<Driving>,
    mut entered: MessageWriter<EnteredVehicle>,
) {
    if review.started {
        return;
    }
    let Ok((car, mut d, seats, tr)) = cars.single_mut() else {
        return;
    };
    vehicle::enter(
        &mut commands,
        car,
        &mut d,
        seats,
        tr,
        review.driver.unwrap(),
        &mut driving,
        &mut entered,
    );
    review.started = true;
}

fn input(
    keys: Res<ButtonInput<KeyCode>>,
    mut cars: Query<&mut Controls>,
    mut physics: ResMut<Time<Physics>>,
) {
    // Automated model captures must not consume typing in other desktop apps.
    if std::env::var_os("EARTH_TWO_CAPTURE").is_some() {
        return;
    }
    for mut controls in &mut cars {
        controls.forward =
            f32::from(keys.pressed(KeyCode::KeyW)) - f32::from(keys.pressed(KeyCode::KeyS));
        controls.steer =
            f32::from(keys.pressed(KeyCode::KeyD)) - f32::from(keys.pressed(KeyCode::KeyA));
        controls.hand_brake = keys.pressed(KeyCode::Space);
        controls.right = keys.pressed(KeyCode::KeyR);
        controls.headlamps = keys.just_pressed(KeyCode::KeyH);
    }
    if keys.just_pressed(KeyCode::Escape) {
        let paused = !physics.is_paused();
        earth_two_client::physics::set_paused(&mut physics, paused);
    }
}

#[allow(clippy::type_complexity)]
fn models(
    mut commands: Commands,
    review: Res<Review>,
    assets: Res<AssetServer>,
    cars: Query<(Entity, &Drivable), Added<Drivable>>,
    wheels: Query<(Entity, &Name), (With<Wheel>, Added<WheelOf>)>,
) {
    for (e, car) in &cars {
        let scene: Handle<WorldAsset> =
            assets.load(format!("{}#Scene0", review.kit.pieces[&car.name].model));
        commands.entity(e).with_child(WorldAssetRoot(scene));
    }
    for (e, name) in &wheels {
        if let Some(piece) = review.kit.pieces.get(name.as_str()) {
            let scene: Handle<WorldAsset> = assets.load(format!("{}#Scene0", piece.model));
            commands
                .entity(e)
                .insert(Visibility::default())
                .with_child(WorldAssetRoot(scene));
        }
    }
}

#[allow(clippy::type_complexity)]
fn camera_and_hud(
    cars: Query<(&Transform, &Drivable, &VehicleState), Without<FollowCamera>>,
    mut cameras: Query<&mut Transform, With<FollowCamera>>,
    mut hud: Query<&mut Text, With<Hud>>,
    physics: Res<Time<Physics>>,
) {
    let Ok((tr, car, state)) = cars.single() else {
        return;
    };
    for mut camera in &mut cameras {
        let forward = tr.rotation * Vec3::Z;
        let flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::Z);
        let (low, high) = vehicle::spec::bounds(&car.spec);
        let distance = ((high - low).length() * 1.6).max(8.0);
        let at = tr.translation + tr.rotation * ((low + high) * 0.5);
        *camera = Transform::from_translation(at - flat * distance + Vec3::Y * distance * 0.4)
            .looking_at(at, Vec3::Y);
    }
    for mut text in &mut hud {
        text.0 = format!(
            "Vehicle review: {} | {:.1} km/h | gear {} | {} wheels touching\nW/S: throttle/brake/reverse   A/D: steer   Space: handbrake\nH: headlamps   Hold R when tipped: recover   Esc: pause/resume {}\nPhysics review only; default materials, no rider or game flow",
            car.name,
            state.speed * 3.6,
            state.gear,
            state.touching,
            if physics.is_paused() { "[paused]" } else { "" }
        );
    }
}

fn capture(
    mut commands: Commands,
    assets: Res<AssetServer>,
    roots: Query<&WorldAssetRoot>,
    time: Res<Time<Real>>,
    mut requested: Local<bool>,
) {
    let Ok(path) = std::env::var("EARTH_TWO_CAPTURE") else {
        return;
    };
    if *requested
        || time.elapsed_secs() < 5.0
        || roots.is_empty()
        || roots
            .iter()
            .any(|r| !assets.is_loaded_with_dependencies(r.0.id()))
    {
        return;
    }
    use bevy::render::view::window::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
    *requested = true;
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
}
