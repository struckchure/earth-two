//! The vehicle tests from `vehicle/vehicle_test.go`, `real_test.go`,
//! `headlamps_test.go`, `impacts_test.go` and `pedestrians_test.go`, run
//! headless: a vehicle parked on a floor 400 m across, and a driver who
//! gets in, as the Go rig had it. Getting in and out is the character's;
//! here the rig seats the driver through [`enter`] and takes the
//! [`LeftVehicle`] message as standing them up.

use avian3d::prelude::*;
use bevy::{asset::AssetPlugin, ecs::system::RunSystemOnce, prelude::*, time::TimeUpdateStrategy};
use earth_two_client::{
    character::{
        Character, CharacterController, CharacterPlugin, Health, Intent, LifeState, Player,
        Traversal, TraversalConfig,
    },
    physics::{EarthPhysicsPlugin, FIXED_HZ, set_paused, source_assets},
    vehicle::{
        Controls, Drivable, Driving, EnteredVehicle, Headlamp, HeadlampBeam, LeftVehicle,
        LightCycle, Prompt, Seats, Spec, Tyre, VehicleInput, VehiclePlugin, VehicleState, enter,
        spawn_parked,
        spec::{ChassisBox, SeatSpec, WheelSpec, bounds, corners},
    },
    world::WorldPlugin,
};
use earth_two_world::kit::Kit;
use std::{collections::BTreeMap, time::Duration};

/// A buggy-sized four-wheel-drive car.
fn test_buggy() -> Spec {
    let mut s = Spec {
        handling: "buggy".into(),
        chassis: vec![ChassisBox {
            center: [0.0, 0.95, 0.0],
            size: [1.6, 0.7, 3.6],
            rotation: [0.0; 4],
        }],
        seats: vec![SeatSpec {
            at: [0.35, 0.55, 0.0],
            pose: "drive".into(),
            exits: vec![[1.8, 0.0, 0.0], [-1.8, 0.0, 0.0], [0.0, 0.0, -3.0]],
            ..default()
        }],
        ..default()
    };
    for z in [1.3, -1.3] {
        for x in [0.85, -0.85] {
            s.wheels.push(WheelSpec {
                piece: "buggy_wheel".into(),
                at: [x, 0.38, z],
                radius: 0.38,
                width: 0.28,
                left: x > 0.0,
                steer: z > 0.0,
                drive: true,
                hand_brake: z < 0.0,
            });
        }
    }
    s
}

/// The drivable vehicles as world.json builds them.
fn real_specs() -> BTreeMap<String, Spec> {
    let kit = Kit::load(source_assets(), "world/world.json").unwrap();
    kit.pieces
        .into_iter()
        .filter_map(|(name, p)| p.vehicle.map(|v| (name, v)))
        .collect()
}

#[derive(Resource, Default)]
struct Left(Vec<LeftVehicle>);

fn collect_left(mut reader: MessageReader<LeftVehicle>, mut left: ResMut<Left>) {
    left.0.extend(reader.read().copied());
}

struct Rig {
    app: App,
    car: Entity,
    player: Entity,
}

fn static_box(world: &mut World, at: Vec3, size: Vec3) -> Entity {
    world
        .spawn((
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            Transform::from_translation(at),
        ))
        .id()
}

/// A buggy parked on a floor 400 m across, and the player standing beside
/// its driving seat, without a window.
fn new_rig() -> Rig {
    new_rig_with("buggy", &test_buggy())
}

/// Parks `name`, built as `spec`, with the player beside its seat.
fn new_rig_with(name: &str, spec: &Spec) -> Rig {
    new_rig_with_warmup(name, spec, 30)
}

fn new_rig_with_warmup(name: &str, spec: &Spec, warmup: usize) -> Rig {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        bevy::scene::ScenePlugin,
        EarthPhysicsPlugin,
        WorldPlugin,
        CharacterPlugin,
        VehiclePlugin,
    ))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / FIXED_HZ,
    )))
    .init_resource::<Left>()
    .add_systems(Update, collect_left);
    app.finish();
    app.cleanup();
    let world = app.world_mut();
    world.spawn((
        RigidBody::Static,
        Collider::cuboid(400.0, 1.0, 400.0),
        Friction::new(0.9),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
    let mut commands = world.commands();
    let car = spawn_parked(&mut commands, name, spec, Vec3::ZERO, Quat::IDENTITY);
    world.flush();
    let seat = spec.seats[0].at;
    let player = world
        .spawn((
            Player,
            Health::default(),
            Transform::from_xyz(seat[0] + 1.6, 0.9, seat[2]),
        ))
        .id();
    let mut g = Rig { app, car, player };
    g.tick(warmup);
    assert!(
        g.app.world().get::<Drivable>(car).is_some(),
        "the parked {name} should be made drivable"
    );
    g
}

impl Rig {
    fn tick(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
        }
    }

    fn get<T: Component + Clone>(&self, e: Entity) -> T {
        self.app.world().get::<T>(e).unwrap().clone()
    }

    fn translation(&self, e: Entity) -> Vec3 {
        self.get::<Transform>(e).translation
    }

    fn rotation(&self, e: Entity) -> Quat {
        self.get::<Transform>(e).rotation
    }

    fn up_y(&self) -> f32 {
        (self.rotation(self.car) * Vec3::Y).y
    }

    fn state(&self) -> VehicleState {
        self.get::<VehicleState>(self.car)
    }

    fn body(&self, e: Entity) -> RigidBody {
        *self.app.world().get::<RigidBody>(e).unwrap()
    }

    fn driving(&self) -> Driving {
        self.app.world().resource::<Driving>().clone()
    }

    fn prompt(&self) -> Prompt {
        self.app.world().resource::<Prompt>().clone()
    }

    fn keys(&mut self, f: impl FnOnce(&mut Controls)) {
        let mut controls = self.app.world_mut().get_mut::<Controls>(self.car).unwrap();
        f(&mut controls);
    }

    /// Presses and lets go of a key over a frame.
    fn tap(&mut self, f: impl FnOnce(&mut Controls)) {
        self.keys(f);
        self.tick(1);
        self.keys(|k| {
            k.exit = false;
            k.headlamps = false;
            k.right = false;
        });
        self.tick(1);
    }

    /// E beside the seat: the player gets in.
    fn get_in(&mut self) {
        let (car, player) = (self.car, self.player);
        self.app
            .world_mut()
            .run_system_once(
                move |mut commands: Commands,
                      mut cars: Query<(&mut Drivable, &Seats, &Transform)>,
                      mut driving: ResMut<Driving>,
                      mut entered: MessageWriter<EnteredVehicle>| {
                    let (mut d, seats, tr) = cars.get_mut(car).unwrap();
                    enter(
                        &mut commands,
                        car,
                        &mut d,
                        seats,
                        tr,
                        player,
                        &mut driving,
                        &mut entered,
                    );
                },
            )
            .unwrap();
        self.tick(2);
    }

    /// E in the seat: the player tries to get out.
    fn get_out(&mut self) {
        self.tap(|k| k.exit = true);
    }

    fn seated(&self) -> bool {
        self.get::<Drivable>(self.car).driver.is_some()
    }

    fn set_velocity(&mut self, v: Vec3) {
        self.app
            .world_mut()
            .get_mut::<LinearVelocity>(self.car)
            .unwrap()
            .0 = v;
    }

    fn velocity(&self) -> Vec3 {
        self.app.world().get::<LinearVelocity>(self.car).unwrap().0
    }

    /// A pedestrian standing at `feet`, stepped every `every` ticks.
    fn pedestrian(&mut self, feet: Vec3, every: u32) -> Entity {
        let mut cc = CharacterController::standing(0.3, 1.8, 0.3);
        cc.every = every;
        let e = self
            .app
            .world_mut()
            .spawn((
                Character::default(),
                Intent::default(),
                Traversal::default(),
                TraversalConfig::default(),
                Health::default(),
                cc,
                Transform::from_translation(feet + Vec3::Y * 0.9),
            ))
            .id();
        self.tick(30);
        e
    }
}

/// `TestGetInDriveGetOutAndPark`, without the character: the seat and the
/// stand are the messages the character port acts on.
#[test]
fn get_in_drive_get_out_and_park() {
    let mut g = new_rig();
    assert_eq!(
        g.body(g.car),
        RigidBody::Static,
        "a parked vehicle should be Static"
    );

    g.get_in();
    assert!(g.seated(), "E should seat the player");
    assert!(
        g.body(g.car) == RigidBody::Dynamic && g.driving().active() && g.driving().name == "buggy",
        "getting in should wake the buggy, and say the player's driving it"
    );
    g.tick(60);
    let st = g.state();
    assert_eq!(
        st.touching, 4,
        "it should stand on its 4 wheels, {} touch",
        st.touching
    );
    let rest = g.translation(g.car);
    assert!(
        rest.y.abs() <= 0.03,
        "woken, it should settle where it was parked (its suspension at rest), not at y={}",
        rest.y
    );

    g.keys(|k| k.forward = 1.0);
    g.tick(180);
    g.keys(|k| k.forward = 0.0);
    let car = g.translation(g.car);
    assert!(car.z >= 8.0, "W should drive it forward, it's at {car}");
    let pose = g.driving().pose.0;
    assert!(
        pose.distance(car) <= 2.0,
        "the driver should go with it: pose {pose}, buggy at {car}"
    );

    // Too fast to get out.
    g.get_out();
    assert!(
        g.prompt().noting() == "Slow down to get out" && g.seated(),
        "at speed, E shouldn't let the player out: note {:?}, seated {}, speed {}",
        g.prompt().noting(),
        g.seated(),
        g.state().speed
    );

    g.keys(|k| k.forward = -1.0);
    for _ in 0..600 {
        g.tick(1);
        if g.state().speed.abs() < 0.5 {
            break;
        }
    }
    g.keys(|k| k.forward = 0.0);
    let s = g.state().speed;
    assert!(
        s.abs() <= 0.5,
        "S should brake it to a stop, still at {s} m/s"
    );

    g.get_out();
    assert!(
        !g.seated(),
        "stopped, E should let the player out ({})",
        g.prompt().noting()
    );
    let left = g.app.world().resource::<Left>().0.clone();
    assert_eq!(left.len(), 1, "getting out should stand the player up");
    let car = g.translation(g.car);
    assert!(
        left[0].feet.y.abs() < 0.1 && left[0].feet.distance(car) >= 1.2,
        "the player should stand on the ground beside it: at {}, buggy at {car}",
        left[0].feet
    );
    assert!(!g.driving().active(), "on foot, the player isn't driving");

    g.tick(150);
    assert_eq!(
        g.body(g.car),
        RigidBody::Static,
        "left at rest, it should be parked Static"
    );
    let parked = g.translation(g.car);
    g.tick(60);
    assert_eq!(g.translation(g.car), parked, "parked, it should stay put");
}

/// `TestReverseFromAStop`.
#[test]
fn reverse_from_a_stop() {
    let mut g = new_rig();
    g.get_in();
    g.tick(30);
    g.keys(|k| k.forward = -1.0);
    g.tick(60);
    let gear = g.state().gear;
    assert!(gear < 0, "it should be in reverse, in gear {gear}");
    g.tick(90);
    let z = g.translation(g.car).z;
    assert!(z <= -2.0, "S from a stop should reverse, it's at z={z}");
}

/// `TestSteering`.
#[test]
fn steering() {
    let mut g = new_rig();
    g.get_in();
    g.tick(30);
    g.keys(|k| {
        k.forward = 1.0;
        k.steer = 1.0;
    });
    g.tick(150);
    let car = g.translation(g.car);
    // Facing +Z, its right is -X.
    assert!(
        car.x <= -2.0,
        "D should turn it right (toward -X), it's at {car}"
    );
    let steer = g.state().wheels[0].steer;
    assert!(
        steer < 0.0,
        "its front wheels should be turned right (negative steer), got {steer}"
    );
}

/// `TestExitBlocked`.
#[test]
fn exit_blocked() {
    let mut g = new_rig();
    g.get_in();
    g.tick(30);
    // Walls along both sides and behind: nowhere to stand.
    let world = g.app.world_mut();
    static_box(world, Vec3::new(1.8, 1.5, 0.0), Vec3::new(1.0, 3.0, 8.0));
    static_box(world, Vec3::new(-1.8, 1.5, 0.0), Vec3::new(1.0, 3.0, 8.0));
    static_box(world, Vec3::new(0.0, 1.5, -3.0), Vec3::new(4.0, 3.0, 1.0));
    g.tick(10);
    g.get_out();
    assert!(
        g.seated() && g.prompt().noting() == "No room to get out here",
        "hemmed in, the player should stay in, got note {:?}",
        g.prompt().noting()
    );
}

/// A rough stand-in for a kind of vehicle: wheels at (x, z) of radius r,
/// all driven, the front ones steering, under a chassis box.
fn stand_in(
    handling: &str,
    r: f32,
    width: f32,
    wheels: &[[f32; 2]],
    chassis: ChassisBox,
    seat: [f32; 3],
) -> Spec {
    let mut s = Spec {
        handling: handling.into(),
        chassis: vec![chassis],
        seats: vec![SeatSpec {
            at: seat,
            exits: vec![[seat[0] + 2.5, 0.0, seat[2]]],
            ..default()
        }],
        ..default()
    };
    for w in wheels {
        s.wheels.push(WheelSpec {
            at: [w[0], r, w[1]],
            radius: r,
            width,
            left: w[0] > 0.0,
            steer: w[1] > 0.0,
            drive: true,
            hand_brake: w[1] < 0.0,
            ..default()
        });
    }
    s
}

fn aligned(center: [f32; 3], size: [f32; 3]) -> ChassisBox {
    ChassisBox {
        center,
        size,
        rotation: [0.0; 4],
    }
}

/// `TestEveryKindDrivesAndStaysUp`.
#[test]
fn every_kind_drives_and_stays_up() {
    let kinds = [
        (
            "trike",
            stand_in(
                "trike",
                0.34,
                0.22,
                &[[0.44, 0.85], [-0.44, 0.85], [0.0, -0.95]],
                aligned([0.0, 0.75, -0.05], [0.9, 0.6, 2.1]),
                [0.0, 0.62, -0.25],
            ),
        ),
        (
            "bike",
            stand_in(
                "bike",
                0.33,
                0.12,
                &[[0.0, 0.74], [0.0, -0.74]],
                aligned([0.0, 0.85, 0.0], [0.35, 0.6, 1.6]),
                [0.0, 0.7, -0.15],
            ),
        ),
        (
            "rover",
            stand_in(
                "rover",
                0.55,
                0.4,
                &[
                    [1.4, 1.8],
                    [-1.4, 1.8],
                    [1.4, 0.0],
                    [-1.4, 0.0],
                    [1.4, -1.8],
                    [-1.4, -1.8],
                ],
                aligned([0.0, 1.75, 0.0], [2.4, 1.8, 5.0]),
                [0.5, 1.2, 1.4],
            ),
        ),
        (
            "truck",
            stand_in(
                "truck",
                0.6,
                0.45,
                &[
                    [1.1, 2.5],
                    [-1.1, 2.5],
                    [1.1, -1.3],
                    [-1.1, -1.3],
                    [1.1, -2.7],
                    [-1.1, -2.7],
                ],
                aligned([0.0, 2.2, 0.0], [2.4, 2.6, 7.4]),
                [0.6, 1.5, 2.6],
            ),
        ),
    ];
    for (name, spec) in kinds {
        let mut g = new_rig_with(name, &spec);
        g.get_in();
        assert!(
            g.driving().active(),
            "{name}: couldn't get in: prompt {:?}",
            g.prompt()
        );
        g.tick(60);
        let start = g.translation(g.car);
        assert!(
            start.y.abs() <= 0.03,
            "{name}: woken, it should settle where it was parked, not at y={}",
            start.y
        );
        g.keys(|k| k.forward = 1.0);
        g.tick(240);
        g.keys(|k| k.steer = 1.0);
        g.tick(180);
        let up = g.rotation(g.car) * Vec3::Y;
        assert!(up.y >= 0.7, "{name}: it went over: up is {up}");
        let d = g.translation(g.car).distance(start);
        assert!(d >= 12.0, "{name}: it only went {d} m");
        assert!(g.state().touching > 0, "{name}: it's off the ground");
    }
}

#[derive(Resource, Default)]
struct RayHit(Option<Entity>);

fn cast_along_wheels(spatial: SpatialQuery, tyres: Query<&Tyre>, mut hit: ResMut<RayHit>) {
    // Along the buggy's left side at hub height, front to back.
    hit.0 = spatial
        .cast_ray(
            Vec3::new(0.85, 0.38, 3.0),
            Dir3::NEG_Z,
            6.0,
            true,
            &SpatialQueryFilter::default(),
        )
        .filter(|h| tyres.contains(h.entity))
        .map(|h| h.entity);
}

/// `TestParkedTyresAreSolid`: a ray along a parked vehicle's side, through
/// the wheels, hits a tyre. Driven, they're not in the way of its wheels.
#[test]
fn parked_tyres_are_solid() {
    let mut g = new_rig();
    g.app.init_resource::<RayHit>();
    let hit = |g: &mut Rig| {
        g.app
            .world_mut()
            .run_system_once(cast_along_wheels)
            .unwrap();
        g.app.world().resource::<RayHit>().0
    };
    assert!(
        hit(&mut g).is_some(),
        "parked, a ray along the wheels should hit a tyre"
    );
    g.get_in();
    g.tick(10);
    assert!(
        hit(&mut g).is_none(),
        "driven, the tyres shouldn't be there"
    );
    let touching = g.state().touching;
    assert_eq!(
        touching, 4,
        "its wheels should find the ground, not tyres: {touching} touch"
    );
}

/// `TestPausedVehicleHoldsStill`: paused while driving, the vehicle holds
/// still where it was last simulated; the fixed clock running on doesn't
/// rock it between its last two steps.
#[test]
fn paused_vehicle_holds_still() {
    let mut g = new_rig();
    g.get_in();
    g.keys(|k| k.forward = 1.0);
    g.tick(90);
    set_paused(&mut g.app.world_mut().resource_mut::<Time<Physics>>(), true);
    g.tick(1);
    let held = g.driving().pose.0;
    let held_rotation = g.rotation(g.car);
    // Frames that don't divide the physics step, so the share of one
    // they've run past keeps changing.
    g.app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 97.0,
        )));
    for i in 0..40 {
        g.tick(1);
        let at = g.driving().pose.0;
        assert_eq!(
            at,
            held,
            "paused, the buggy moved from {held} to {at}, {} frames on",
            i + 1
        );
        assert_eq!(g.translation(g.car), held);
        assert_eq!(g.rotation(g.car), held_rotation);
    }
    set_paused(
        &mut g.app.world_mut().resource_mut::<Time<Physics>>(),
        false,
    );
    g.tick(40);
    assert!(g.translation(g.car).distance(held) > 0.1);
}

/// `TestRealVehiclesStayUpright`: every vehicle, as built, drives off,
/// turns hard at speed and stays on its wheels.
fn assert_real_vehicle_stays_upright(name: &str) {
    let spec = real_specs().remove(name).unwrap();
    let mut g = new_rig_with(name, &spec);
    g.get_in();
    assert!(
        g.driving().active(),
        "{name}: couldn't get in: {:?}",
        g.prompt()
    );
    g.tick(60);
    g.keys(|k| k.forward = 1.0);
    let mut worst = 1.0f32;
    for i in 0..600 {
        if i == 240 {
            g.keys(|k| k.steer = 1.0);
        }
        if i == 420 {
            g.keys(|k| k.steer = -1.0);
        }
        g.tick(1);
        worst = worst.min(g.up_y());
    }
    let st = g.state();
    println!("{name}: speed {:.1} m/s, worst up.Y {worst:.2}", st.speed);
    assert!(
        worst >= 0.8,
        "{name}: it went over (up.Y down to {worst:.2})"
    );
}

/// `TestBikeStaysUpSlow`: sat on at a standstill, and ridden slowly, the
/// bike stays up.
#[test]
fn bike_stays_up_slow() {
    let mut g = new_rig_with("bike", &real_specs()["bike"]);
    g.get_in();
    let mut worst = 1.0f32;
    let mut track = |g: &mut Rig, n: usize| {
        for _ in 0..n {
            g.tick(1);
            worst = worst.min(g.up_y());
        }
    };
    track(&mut g, 300);
    g.keys(|k| k.forward = 1.0);
    track(&mut g, 40);
    g.keys(|k| k.forward = 0.0);
    track(&mut g, 240);
    println!(
        "crawling: worst up.Y {worst:.2}, speed {:.1}",
        g.state().speed
    );
    assert!(worst >= 0.9, "it fell over (up.Y down to {worst:.2})");
}

fn assert_headlamps(g: &mut Rig, want: bool) {
    let got = g.get::<Drivable>(g.car).headlamps;
    assert_eq!(got, want, "headlamps powered {got}, want {want}");
    if g.driving().active() {
        assert_eq!(g.driving().headlamps, want, "HUD headlamps");
    }
    let mut count = 0;
    let mut beams = g.app.world_mut().query::<(&Headlamp, &HeadlampBeam)>();
    for (_, beam) in beams.iter(g.app.world()) {
        assert_eq!(
            beam.enabled, want,
            "beam enabled {}, want {want}",
            beam.enabled
        );
        count += 1;
    }
    assert!(count > 0, "vehicle has no headlamps");
}

/// `TestHeadlampsAutomaticAndManual`.
#[test]
fn headlamps_automatic_and_manual() {
    let mut g = new_rig();
    let night = |g: &mut Rig, on: bool| {
        g.app.world_mut().resource_mut::<LightCycle>().night = on;
        g.tick(1);
    };
    assert_headlamps(&mut g, false);
    g.tap(|k| k.headlamps = true); // On foot: no vehicle controls.
    assert_headlamps(&mut g, false);
    night(&mut g, true);
    assert_headlamps(&mut g, false); // Parked vehicles do not automatically light up.
    g.get_in();
    assert!(g.driving().active(), "could not enter vehicle");
    assert_headlamps(&mut g, true);
    night(&mut g, false);
    assert_headlamps(&mut g, false); // Dawn switches automatic lights off.
    night(&mut g, true);
    assert_headlamps(&mut g, true);
    g.get_out();
    assert_headlamps(&mut g, false); // Automatic lamps switch off when exiting.
    g.get_in();
    assert_headlamps(&mut g, true);
    g.tap(|k| k.headlamps = true);
    g.tick(120);
    assert_headlamps(&mut g, false); // Automation must not undo a manual off.
    g.get_out();
    g.get_in();
    assert_headlamps(&mut g, true); // A new drive returns to the automatic default.
    g.tap(|k| k.headlamps = true);
    g.tap(|k| k.headlamps = true); // Explicitly switch on.
    assert_headlamps(&mut g, true);
    g.get_out();
    g.tick(120);
    assert_headlamps(&mut g, true); // Explicit on stays on after exiting.
    night(&mut g, false);
    assert_headlamps(&mut g, true); // The manual on remains authoritative by day.
    g.tap(|k| k.headlamps = true);
    assert_headlamps(&mut g, true); // On-foot H does not alter a parked vehicle.
    g.get_in();
    g.tap(|k| k.headlamps = true);
    assert_headlamps(&mut g, false);
    g.get_out();
    assert_headlamps(&mut g, false);
    g.get_in();
    assert_headlamps(&mut g, false); // Entering by day leaves automatic lights off.
    // H can turn lights on by day, and holding H only toggles once.
    g.keys(|k| k.headlamps = true);
    g.tick(1);
    g.tick(60);
    assert_headlamps(&mut g, true);
    g.app
        .world_mut()
        .resource_mut::<earth_two_client::character::Controls>()
        .enabled = false;
    g.tap(|k| k.headlamps = true);
    assert_headlamps(&mut g, true);
}

/// `TestEveryRealVehicleHasForwardHeadlamps`.
#[test]
fn every_real_vehicle_has_forward_headlamps() {
    for (name, spec) in real_specs() {
        let mut g = new_rig_with(&name, &spec);
        let at = Vec3::new(12.0, 4.0, -7.0);
        *g.app.world_mut().get_mut::<Transform>(g.car).unwrap() = Transform::from_translation(at)
            .with_rotation(Quat::from_axis_angle(Vec3::Y, 90f32.to_radians()));
        g.tick(1);
        let want = if spec.handling == "bike" { 1 } else { 2 };
        let mut count = 0;
        let mut lamps = g
            .app
            .world_mut()
            .query::<(&Headlamp, &HeadlampBeam, &GlobalTransform)>();
        for (_, beam, p) in lamps.iter(g.app.world()) {
            let forward = p.forward();
            assert!(
                forward.x >= 0.98 && forward.y < 0.0,
                "{name}: beam does not follow the nose and dip toward the road: {forward:?}"
            );
            assert!(
                p.translation().x > at.x && beam.range >= 30.0,
                "{name}: lamp isn't ahead of the chassis or has too little reach: {:?}, {beam:?}",
                p.translation()
            );
            count += 1;
        }
        assert_eq!(count, want, "{name}: {count} headlamps, want {want}");
    }
}

/// `TestHOnlyChangesTheDrivenVehicle`.
#[test]
fn h_only_changes_the_driven_vehicle() {
    let mut g = new_rig();
    let world = g.app.world_mut();
    let mut commands = world.commands();
    let other = spawn_parked(
        &mut commands,
        "other buggy",
        &test_buggy(),
        Vec3::X * 20.0,
        Quat::IDENTITY,
    );
    world.flush();
    g.tick(2);
    g.app.world_mut().resource_mut::<LightCycle>().night = true;
    g.app
        .world_mut()
        .get_mut::<Drivable>(other)
        .unwrap()
        .switch_headlamps(true);
    g.tick(1);
    g.get_in();
    assert!(g.driving().active(), "could not enter vehicle");
    g.tap(|k| k.headlamps = true);
    assert!(
        !g.get::<Drivable>(g.car).headlamps,
        "H didn't switch the driven car's lamps off"
    );
    assert!(
        g.get::<Drivable>(other).headlamps,
        "H switched off another vehicle's lamps"
    );
}

/// `TestVehiclesRunOverCharacters`.
#[test]
fn vehicles_run_over_characters() {
    for (name, speed, state, player) in [
        ("slow bump", 2.0, LifeState::Healthy, false),
        ("critical", 4.0, LifeState::Critical, false),
        ("fatal", 12.0, LifeState::Dead, false),
        ("fatal reverse", -12.0, LifeState::Dead, false),
        ("player critical", 4.0, LifeState::Critical, true),
        ("player fatal", 12.0, LifeState::Dead, true),
    ] {
        let mut g = new_rig();
        g.get_in();
        g.tick(60);
        let direction = if speed < 0.0 { -1.0 } else { 1.0 };
        let person = g.pedestrian(Vec3::Z * (direction * 2.35), 6);
        if player {
            let world = g.app.world_mut();
            world.entity_mut(g.player).remove::<Player>();
            world.entity_mut(person).insert(Player);
            g.tick(1);
        }
        g.set_velocity(Vec3::Z * speed);
        for _ in 0..25 {
            g.tick(1);
            if g.get::<Health>(person).state != LifeState::Healthy {
                break;
            }
        }
        let health = g.get::<Health>(person);
        assert_eq!(
            health.state,
            state,
            "{name}: impact {:.1} km/h: health={health:?}, want state {state:?}",
            speed * 3.6
        );
        if state == LifeState::Healthy {
            continue;
        }
        assert_eq!(
            health.vehicle,
            Some(g.car),
            "{name}: impact must retain its source vehicle"
        );
        assert!(
            g.app.world().get::<CharacterController>(person).is_none(),
            "{name}: an incapacitated character still has an immovable standing capsule"
        );
        assert!(
            g.body(person) == RigidBody::Dynamic && g.get::<Mass>(person).0 == 70.0,
            "{name}: victim must become a finite-mass fallen body"
        );
        assert_eq!(
            g.get::<Intent>(person).move_dir,
            Vec3::ZERO,
            "{name}: incapacitated character still walks"
        );
        assert_eq!(
            g.get::<Health>(g.player).state,
            LifeState::Healthy,
            "{name}: driver injured by their own vehicle"
        );
        g.tick(60);
        let at = g.translation(g.car);
        assert!(
            at.z * direction >= 3.0,
            "{name}: vehicle stopped on the victim: {at}"
        );
        assert!(
            g.velocity().z * direction >= speed * direction * 0.3,
            "{name}: impact removed the vehicle's momentum"
        );
    }
}

/// `TestPassingVehicleDoesNotDamageNearbyCharacter`.
#[test]
fn passing_vehicle_does_not_damage_nearby_character() {
    let mut g = new_rig();
    g.get_in();
    g.tick(60);
    let person = g.pedestrian(Vec3::new(2.0, 0.0, 2.35), 6);
    g.set_velocity(Vec3::Z * 12.0);
    g.tick(60);
    let health = g.get::<Health>(person);
    assert_eq!(
        health.state,
        LifeState::Healthy,
        "a nearby character was injured without chassis contact: {health:?}"
    );
}

/// `TestWallProtectsCharacterFromVehicle`.
#[test]
fn wall_protects_character_from_vehicle() {
    let mut g = new_rig();
    g.get_in();
    let person = g.pedestrian(Vec3::Z * 8.0, 6);
    static_box(
        g.app.world_mut(),
        Vec3::new(0.0, 2.0, 4.0),
        Vec3::new(8.0, 4.0, 1.0),
    );
    g.tick(2);
    g.set_velocity(Vec3::Z * 12.0);
    g.tick(90);
    assert_eq!(
        g.get::<Health>(person).state,
        LifeState::Healthy,
        "vehicle damaged a character through a wall"
    );
    assert!(
        g.translation(g.car).z <= 4.0,
        "vehicle drove through the wall"
    );
}

/// `TestVehiclePushesPedestrianInsteadOfStopping`.
#[test]
fn vehicle_pushes_pedestrian_instead_of_stopping() {
    for (name, forward, direction) in [("forward", 1.0, 1.0), ("reverse", -1.0, -1.0)] {
        for every in [1, 6, 8] {
            let mut g = new_rig();
            g.get_in();
            g.tick(60);
            let start = g.translation(g.car);
            let person = g.pedestrian(Vec3::new(start.x, 0.0, start.z + 8.0 * direction), every);
            let before = g.translation(person);
            g.keys(|k| k.forward = forward);
            g.tick(240);
            let car = g.translation(g.car);
            let after = g.translation(person);
            let speed = g.state().speed;
            println!("{name}/{every}: car={car} pedestrian={before} -> {after} speed={speed:.2}");
            assert!(
                (car.z - before.z) * direction >= 4.0 && speed * direction >= 2.0,
                "{name}/{every}: a pedestrian stopped the car: car={car} pedestrian={after} speed={speed:.2}"
            );
            assert!(
                before.distance(after) >= 2.0,
                "{name}/{every}: the vehicle passed through the pedestrian without pushing them"
            );
        }
    }
}

/// `TestPedestrianPhysicsKeepsCrowdLODOutsideCarContacts`.
#[test]
fn pedestrian_physics_keeps_crowd_lod_outside_car_contacts() {
    let mut g = new_rig();
    g.get_in();
    let near = g.pedestrian(Vec3::X * 4.0, 8);
    let far = g.pedestrian(Vec3::X * 50.0, 8);
    let mut far_skipped = false;
    for _ in 0..8 {
        g.tick(1);
        let close = g.get::<CharacterController>(near);
        let distant = g.get::<CharacterController>(far);
        assert!(
            close.stepped,
            "pedestrians within chassis reach must step every tick"
        );
        assert!(
            close.every == 8 && distant.every == 8,
            "the temporary override must preserve each pedestrian's configured interval"
        );
        far_skipped = far_skipped || !distant.stepped;
    }
    assert!(
        far_skipped,
        "distant pedestrians should retain their cheaper update rate"
    );
    // Pausing skips simulation, but must still undo the temporary override.
    set_paused(&mut g.app.world_mut().resource_mut::<Time<Physics>>(), true);
    g.tick(1);
    assert_eq!(
        g.get::<CharacterController>(near).every,
        8,
        "pausing leaked the full-frequency override"
    );
}

/// `TestRealVehiclesKeepNormalPedestrianResponseWithCrowdLOD`.
fn assert_real_vehicle_pedestrian_response(name: &str) {
    let spec = real_specs().remove(name).unwrap();
    let run = |every: u32| {
        let mut g = new_rig_with(name, &spec);
        g.get_in();
        assert!(g.driving().active(), "could not enter the vehicle");
        g.tick(60);
        let start = g.translation(g.car);
        let person = g.pedestrian(
            Vec3::new(start.x, 0.0, start.z + bounds(&spec).1.z + 6.0),
            every,
        );
        let before = g.translation(person);
        g.keys(|k| k.forward = 1.0);
        g.tick(300);
        println!(
            "{name}/{every}: health={:?}, controller={}, car={:?}, person={:?}",
            g.get::<Health>(person),
            g.app.world().get::<CharacterController>(person).is_some(),
            g.translation(g.car),
            g.translation(person)
        );
        (g.translation(g.car), before, g.translation(person))
    };
    let (baseline, _, _) = run(1);
    let (car, before, after) = run(6);
    // Crowd update frequency must not change contact response.
    assert!(
        baseline.distance(car) <= 0.5,
        "crowd LOD changed {name}'s collision response: full-rate car={baseline} reduced-rate car={car}"
    );
    assert!(
        before.distance(after) >= 0.5,
        "{name}: the pedestrian was not pushed on impact: car={car} before={before} after={after}"
    );
}

// Separate tests ensure a failing bike never hides the remaining vehicles.
macro_rules! real_vehicle_cases {
    ($($name:ident),+ $(,)?) => { $(
        mod $name {
            #[test]
            fn stays_upright() {
                super::assert_real_vehicle_stays_upright(stringify!($name));
            }
            #[test]
            fn pedestrian_response_with_crowd_lod() {
                super::assert_real_vehicle_pedestrian_response(stringify!($name));
            }
        }
    )+ };
}
real_vehicle_cases!(bike, buggy, hauler, hauler_tanker, rover, trike);

/// `TestVehicleStillStopsAtSolidObstacles`.
#[test]
fn vehicle_still_stops_at_solid_obstacles() {
    for obstacle in ["wall", "parked car"] {
        let mut g = new_rig();
        g.get_in();
        let world = g.app.world_mut();
        if obstacle == "wall" {
            static_box(world, Vec3::new(0.0, 2.0, 8.0), Vec3::new(8.0, 4.0, 1.0));
        } else {
            world.spawn((
                RigidBody::Static,
                Collider::convex_hull(&corners(&test_buggy())).unwrap(),
                Transform::from_xyz(0.0, 0.0, 8.0),
            ));
        }
        g.tick(2);
        g.keys(|k| k.forward = 1.0);
        g.tick(240);
        let at = g.translation(g.car);
        assert!(at.z <= 7.0, "vehicle drove through {obstacle}: {at}");
    }
}

/// `TestPedestrianCannotWalkThroughParkedVehicle`.
#[test]
fn pedestrian_cannot_walk_through_parked_vehicle() {
    let mut g = new_rig();
    let person = g.pedestrian(Vec3::Z * 5.0, 1);
    g.app
        .world_mut()
        .get_mut::<Intent>(person)
        .unwrap()
        .move_dir = Vec3::NEG_Z;
    g.tick(300);
    let at = g.translation(person);
    assert!(
        (1.9..=4.0).contains(&at.z),
        "pedestrian should walk up to the parked chassis and stop: {at}"
    );
}

/// The vehicle's input is what the keys asked, held by the brakes with
/// nobody in it.
#[test]
fn parked_vehicle_holds_its_brakes() {
    let g = new_rig();
    let input = g.get::<VehicleInput>(g.car);
    assert_eq!(input.hand_brake, 1.0);
}

/// Replays the same logical control script as the isolated Go observer.
/// Export is optional; normal tests still verify complete, finite traces.
#[test]
fn migration_vehicle_trace() {
    #[derive(serde::Deserialize)]
    struct Phase {
        ticks: usize,
        forward: i32,
        steer: i32,
    }
    #[derive(serde::Deserialize)]
    struct Script {
        version: u32,
        scenario: String,
        fixed_hz: u32,
        warmup_ticks: usize,
        phases: Vec<Phase>,
    }
    let script: Script =
        serde_json::from_str(include_str!("../../../tools/migration/vehicle-inputs.json")).unwrap();
    assert_eq!(script.version, 1);
    assert_eq!(script.fixed_hz, 60);
    let out = std::env::var_os("EARTH_TWO_RUST_TRACE_OUT").map(std::path::PathBuf::from);
    if let Some(path) = &out {
        std::fs::create_dir_all(path).unwrap();
    }
    for (name, spec) in real_specs() {
        let mut g = new_rig_with(&name, &spec);
        g.get_in();
        assert!(g.driving().active());
        g.tick(script.warmup_ticks);
        let mut samples = Vec::new();
        for phase in &script.phases {
            g.keys(|keys| {
                keys.forward = phase.forward as f32;
                keys.steer = phase.steer as f32;
            });
            for _ in 0..phase.ticks {
                g.tick(1);
                let tr = g.get::<Transform>(g.car);
                let state = g.state();
                let input = g.get::<VehicleInput>(g.car);
                let linear = g.velocity();
                let angular = g.get::<AngularVelocity>(g.car).0;
                assert!(tr.translation.is_finite() && tr.rotation.is_finite());
                assert!(linear.is_finite() && angular.is_finite());
                assert!(state.speed.is_finite() && state.rpm.is_finite());
                let wheels: Vec<_> = state
                    .wheels
                    .iter()
                    .map(|w| {
                        serde_json::json!({
                            "contact": w.contact, "suspension": w.suspension,
                            "spin": w.spin, "steer": w.steer,
                        })
                    })
                    .collect();
                samples.push(serde_json::json!({
                    "tick": samples.len() + 1,
                    "entity": format!("vehicle/{name}"),
                    "requested": [phase.forward, phase.steer],
                    "input": [input.forward, input.right, input.brake, input.hand_brake],
                    "position": tr.translation.to_array(),
                    "rotation_xyzw": tr.rotation.to_array(),
                    "linear_velocity": linear.to_array(),
                    "angular_velocity": angular.to_array(),
                    "up_y": (tr.rotation * Vec3::Y).y,
                    "speed": state.speed, "rpm": state.rpm, "gear": state.gear,
                    "touching": state.touching, "wheels": wheels,
                    "diagnostics": g.get::<earth_two_client::vehicle::sim::VehicleRuntime>(g.car).diagnostics(),
                    "mass": g.get::<ComputedMass>(g.car).value(),
                    "inertia_local_columns": g.get::<ComputedAngularInertia>(g.car).value().to_mat3().to_cols_array(),
                    "center_of_mass": g.get::<ComputedCenterOfMass>(g.car).0.to_array(),
                }));
            }
        }
        assert_eq!(
            samples.len(),
            script.phases.iter().map(|p| p.ticks).sum::<usize>()
        );
        if let Some(path) = &out {
            let trace = serde_json::json!({
                "version": 1, "engine": "rust-avian", "scenario": script.scenario,
                "fixed_hz": 60,
                "sample_phase": "after complete harness tick; root transform and last physics writeback",
                "samples": samples,
            });
            std::fs::write(
                path.join(format!("{name}.json")),
                serde_json::to_vec(&trace).unwrap(),
            )
            .unwrap();
        }
    }
}

#[test]
fn waking_a_bike_does_not_hit_its_parked_tyres() {
    let mut g = new_rig_with("bike", &real_specs()["bike"]);
    g.get_in();
    for _ in 0..60 {
        g.tick(1);
        let p = g.get::<Position>(g.car).0;
        assert!(
            Vec2::new(p.x, p.z).length() < 0.03,
            "unpowered bike launched itself: {p}"
        );
        assert!(
            g.velocity().length() < 0.5,
            "unpowered bike acquired speed: {:?}",
            g.velocity()
        );
    }
}

#[test]
fn ordinary_dynamic_props_still_exchange_momentum_with_vehicles() {
    let run = |with_prop: bool| {
        let mut g = new_rig();
        g.get_in();
        g.tick(60);
        let prop = with_prop.then(|| {
            g.app
                .world_mut()
                .spawn((
                    RigidBody::Dynamic,
                    Collider::cuboid(2.0, 1.8, 1.0),
                    Mass(950.0),
                    Friction::new(0.4),
                    Transform::from_xyz(0.0, 0.9, 8.0),
                ))
                .id()
        });
        g.tick(2);
        g.set_velocity(Vec3::Z * 12.0);
        g.tick(60);
        (g.translation(g.car), prop.map(|e| g.translation(e)))
    };
    let (clear, _) = run(false);
    let (car, prop) = run(true);
    assert!(prop.unwrap().z > 8.2, "the prop must receive momentum");
    assert!(
        clear.z - car.z > 0.5,
        "ordinary props must slow the car: clear={clear} contact={car}"
    );
}

#[derive(Resource, Default)]
struct FixedTrace(Vec<(Vec3, Quat, Vec3)>);

fn scripted_fixed_controls(mut cars: Query<&mut VehicleInput>, trace: Res<FixedTrace>) {
    for mut input in &mut cars {
        input.forward = 1.0;
        input.right = if trace.0.len() >= 120 { 0.3 } else { 0.0 };
        input.hand_brake = 0.0;
    }
}

fn record_fixed_motion(
    cars: Query<(&Position, &Rotation, &LinearVelocity), With<Drivable>>,
    mut trace: ResMut<FixedTrace>,
) {
    let (p, r, v) = cars.single().unwrap();
    trace.0.push((p.0, r.0, v.0));
}

#[test]
fn fixed_vehicle_inputs_are_independent_of_render_cadence() {
    let run = |cadence: &[f64]| {
        let mut g = new_rig();
        g.get_in();
        g.tick(60);
        // Test the simulation boundary: render input sampling is supplied by
        // the application, and these consumed inputs change on fixed ticks.
        g.app
            .world_mut()
            .resource_mut::<earth_two_client::character::Controls>()
            .enabled = false;
        g.app
            .init_resource::<FixedTrace>()
            .add_systems(FixedUpdate, scripted_fixed_controls)
            .add_systems(
                FixedPostUpdate,
                record_fixed_motion.after(PhysicsSystems::Writeback),
            );
        let mut frame = 0;
        while g.app.world().resource::<FixedTrace>().0.len() < 240 {
            g.app
                .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                    cadence[frame % cadence.len()],
                )));
            g.tick(1);
            frame += 1;
        }
        g.app.world_mut().resource_mut::<FixedTrace>().0[..240].to_vec()
    };
    let reference = run(&[1.0 / 60.0]);
    for cadence in [&[1.0 / 30.0][..], &[1.0 / 97.0], &[1.0 / 20.0, 1.0 / 144.0]] {
        for (tick, (expected, actual)) in reference.iter().zip(run(cadence)).enumerate() {
            assert!(
                expected.0.distance(actual.0) < 1e-5,
                "position at tick {tick}: {expected:?} != {actual:?}"
            );
            assert!(
                expected.1.angle_between(actual.1) < 1e-3,
                "rotation at tick {tick}"
            );
            assert!(
                expected.2.distance(actual.2) < 1e-5,
                "velocity at tick {tick}"
            );
        }
    }
}

#[test]
fn vehicle_can_be_entered_immediately_after_assembly() {
    for (name, spec) in real_specs() {
        let mut g = new_rig_with_warmup(&name, &spec, 1);
        g.get_in();
        g.tick(120);
        assert!(g.velocity().is_finite(), "{name}");
        assert!(g.translation(g.car).is_finite(), "{name}");
        assert!(g.velocity().length() < 0.5, "{name}: {:?}", g.velocity());
    }
}

#[test]
fn lost_vehicle_returns_home_without_interpolating_through_the_world() {
    let mut g = new_rig();
    g.get_in();
    g.tick(30);
    let lost = Vec3::new(20.0, -45.0, 30.0);
    g.app.world_mut().get_mut::<Position>(g.car).unwrap().0 = lost;
    g.app
        .world_mut()
        .get_mut::<Transform>(g.car)
        .unwrap()
        .translation = lost;
    g.set_velocity(Vec3::new(3.0, -10.0, 4.0));
    g.tick(1);
    let home = g.get::<Drivable>(g.car).home.0 + Vec3::Y;
    assert_eq!(g.get::<Position>(g.car).0, home);
    assert_eq!(g.velocity(), Vec3::ZERO);
    assert_eq!(g.get::<AngularVelocity>(g.car).0, Vec3::ZERO);
    g.app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 144.0,
        )));
    for _ in 0..6 {
        g.tick(1);
        assert!(
            g.translation(g.car).distance(home) < 0.1,
            "stale interpolation after recovery: {:?}",
            g.translation(g.car)
        );
    }
}

// A camera cast must ignore every collider belonging to the driven body,
// while still seeing other bodies. Exercise the authored chassis, not a box
// approximation, including both haulers and all orbit directions.
#[test]
fn camera_sweeps_ignore_every_real_vehicle_chassis_but_keep_world_obstacles() {
    for (name, spec) in real_specs() {
        let mut g = new_rig_with(&name, &spec);
        g.get_in();
        let car = g.car;
        let tr = g.get::<Transform>(car);
        let pivot = tr.translation
            + tr.rotation
                * (Vec3::from(spec.seats[0].at)
                    + Vec3::Y * (earth_two_client::character::CAPSULE_HEIGHT / 2.0 + 0.7));
        let distance = earth_two_client::vehicle::spec::chase_distance(&spec);
        let label = name.clone();
        g.app.world_mut().run_system_once(move |physics: earth_two_client::character::CharacterPhysics| {
            for pitch in [0.0_f32, 12.0, 35.0] {
                for angle in 0..24 {
                    let yaw = angle as f32 * std::f32::consts::TAU / 24.0;
                    let pitch = pitch.to_radians();
                    let offset = Vec3::new(yaw.sin()*pitch.cos(), pitch.sin(), yaw.cos()*pitch.cos()) * distance;
                    let hit = physics.sweep_capsule_excluding(pivot, offset, 0.2, 0.4, car);
                    assert!(hit.is_none(), "{label}: camera hit its own chassis at yaw {yaw}, pitch {pitch}: {hit:?}");
                }
            }
        }).unwrap();
        let wall = static_box(
            g.app.world_mut(),
            pivot + Vec3::Z * (distance * 0.75),
            Vec3::new(30., 20., 0.5),
        );
        g.tick(2);
        g.app
            .world_mut()
            .run_system_once(
                move |physics: earth_two_client::character::CharacterPhysics| {
                    let hit = physics
                        .sweep_capsule_excluding(pivot, Vec3::Z * distance, 0.2, 0.4, car)
                        .expect("wall must still shorten camera arm");
                    assert_eq!(
                        hit.entity, wall,
                        "{name}: camera must see wall, not chassis"
                    );
                    assert!(hit.distance > 0.0 && hit.distance < distance);
                },
            )
            .unwrap();
    }
}
