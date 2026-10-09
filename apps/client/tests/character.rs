//! The character tests from `character/locomotion_test.go`,
//! `character/traversal_test.go` and `character/contacts_test.go`, run
//! headless against the Avian controller: the Go scenarios, feet, facings,
//! intents, tick counts and expectations as they are.

use avian3d::prelude::*;
use bevy::{
    input::{ButtonInput, keyboard::KeyCode},
    prelude::*,
    time::TimeUpdateStrategy,
};
use earth_two_client::{
    character::{
        self, Anim, Body, BoneInfo, BoneTransform, CAPSULE_HEIGHT, Character, CharacterController,
        CharacterPhysics, ContactPlane, ContactPoint, ContactRig, Controls, Intent, LimbContact,
        Player, State, Traversal, TraversalConfig, View,
        contacts::FitContext,
        locomotion::{
            AIR_STEER, BRAKE, CRAWL, SLIP, airborne, approach, ease, player_input, relative,
            stride, turn_toward,
        },
        traversal::{LADDER_FOOT, ROLL_RELEASE, RungSpacing, SLIDE_RELEASE, SLIDE_STEER, stand_up},
        wrap_angle, yaw_of,
    },
    physics::{EarthPhysicsPlugin, FIXED_HZ, set_paused},
    world::Ladder,
};
use std::{f32::consts::PI, time::Duration};

fn horizontal(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

fn direction(v: Vec3) -> Vec3 {
    if v.length_squared() < 0.0001 {
        Vec3::ZERO
    } else {
        v.normalize()
    }
}

/// The Go harness: a 40×1×40 floor at y -0.5, a character standing at
/// `feet`, and whatever `setup` adds, stepped at 60 Hz.
struct Harness {
    app: App,
    root: Entity,
    body: Entity,
}

fn static_box(world: &mut World, pos: Vec3, size: Vec3) -> Entity {
    world
        .spawn((
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            Transform::from_translation(pos),
        ))
        .id()
}

/// Physics at 60 Hz with the fixed-step character systems only, as the
/// Go harness adds `traverse` and `locomote` and no `face`: the body
/// faces where the test turns it.
fn physics_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, EarthPhysicsPlugin))
        .init_resource::<Controls>()
        .init_resource::<View>()
        .add_systems(
            FixedUpdate,
            (
                character::controller::prepare_characters,
                character::traversal::traverse,
                character::locomotion::locomote,
                character::controller::step_characters,
            )
                .chain(),
        )
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / FIXED_HZ,
        )));
    app
}

fn spawn_character(world: &mut World, feet: Vec3) -> (Entity, Entity) {
    let mut body = Entity::PLACEHOLDER;
    let root = world
        .spawn((
            Character::default(),
            Intent::default(),
            Traversal::default(),
            TraversalConfig::default(),
            CharacterController::standing(0.3, 1.8, 0.3),
            Transform::from_translation(feet + Vec3::Y * 0.9),
        ))
        .with_children(|parent| {
            body = parent
                .spawn((Body, State::default(), Transform::from_xyz(0.0, -0.9, 0.0)))
                .id();
        })
        .id();
    (root, body)
}

impl Harness {
    fn new(feet: Vec3, setup: impl FnOnce(&mut World)) -> Harness {
        let mut app = physics_app();
        let world = app.world_mut();
        static_box(world, Vec3::new(0.0, -0.5, 0.0), Vec3::new(40.0, 1.0, 40.0));
        let (root, body) = spawn_character(world, feet);
        setup(world);
        app.finish();
        app.cleanup();
        let mut h = Harness { app, root, body };
        h.tick(2);
        h.tick(15);
        h
    }

    fn tick(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
        }
    }

    fn intent(&mut self) -> Mut<'_, Intent> {
        self.app.world_mut().get_mut::<Intent>(self.root).unwrap()
    }

    fn cc(&mut self) -> Mut<'_, CharacterController> {
        self.app
            .world_mut()
            .get_mut::<CharacterController>(self.root)
            .unwrap()
    }

    fn s(&mut self) -> Mut<'_, Traversal> {
        self.app
            .world_mut()
            .get_mut::<Traversal>(self.root)
            .unwrap()
    }

    fn tr(&mut self) -> Mut<'_, Transform> {
        self.app
            .world_mut()
            .get_mut::<Transform>(self.root)
            .unwrap()
    }

    fn mode(&mut self) -> Anim {
        self.s().mode
    }

    fn translation(&mut self) -> Vec3 {
        self.tr().translation
    }

    /// Turns the body to face dir, as face would in time.
    fn face_to(&mut self, dir: Vec3) {
        self.app
            .world_mut()
            .get_mut::<Transform>(self.body)
            .unwrap()
            .rotation = Quat::from_axis_angle(Vec3::Y, dir.x.atan2(dir.z));
    }

    fn controls(&mut self, enabled: bool) {
        self.app.world_mut().resource_mut::<Controls>().enabled = enabled;
    }

    fn pause(&mut self, paused: bool) {
        set_paused(
            &mut self.app.world_mut().resource_mut::<Time<Physics>>(),
            paused,
        );
    }
}

fn ladder(world: &mut World, ladder: Ladder) -> Entity {
    world.spawn(ladder).id()
}

// --- locomotion_test.go ---

#[test]
fn input_requests_survive_high_refresh_frames() {
    #[derive(Resource, Default)]
    struct Consumed(usize);
    fn consume(mut q: Query<&mut Intent>, mut consumed: ResMut<Consumed>) {
        for mut intent in &mut q {
            if intent.jump && intent.roll && intent.slide {
                consumed.0 += 1;
            }
            intent.jump = false;
            intent.roll = false;
            intent.slide = false;
        }
    }
    let frame = Duration::from_secs(1) / 240;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(Time::<Fixed>::from_hz(FIXED_HZ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(frame))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<Controls>()
        .init_resource::<View>()
        .init_resource::<Consumed>()
        .add_systems(Update, player_input)
        .add_systems(FixedUpdate, consume);
    app.world_mut().spawn((Player, Intent::default()));
    // Bevy's first update has no delta; Go's Tick counts from the first.
    app.update();
    let keys = [KeyCode::Space, KeyCode::KeyR, KeyCode::ControlLeft];
    for key in keys {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
    }
    app.update();
    for key in keys {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(key);
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<Consumed>().0,
        0,
        "physics advanced before its fixed timestep"
    );
    let intent = *app
        .world_mut()
        .query::<&Intent>()
        .single(app.world())
        .unwrap();
    assert!(
        intent.jump && intent.roll && intent.slide,
        "render frames lost a pending physics input"
    );
    // Include the integer-nanosecond remainder of the fixed interval.
    app.insert_resource(TimeUpdateStrategy::ManualDuration(
        Duration::from_secs_f64(1.0 / FIXED_HZ) - 3 * frame,
    ));
    app.update();
    assert_eq!(
        app.world().resource::<Consumed>().0,
        1,
        "pending inputs were not consumed at the physics step"
    );
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / FIXED_HZ,
    )));
    app.update();
    assert_eq!(
        app.world().resource::<Consumed>().0,
        1,
        "released inputs repeated"
    );
}

/// Turns from 0 toward target at Default's walking turn, 60 times a
/// second, returning how long it takes to get there and the fastest it went.
fn turn_about(target: f32) -> (f32, f32) {
    let c = Character::default();
    let dt = 1.0 / 60.0;
    let (mut yaw, mut rate) = (0.0f32, 0.0f32);
    let mut fastest = 0.0f32;
    for i in 1..=600 {
        let prev = yaw;
        (yaw, rate) = turn_toward(yaw, rate, target, c.turn_speed, c.turn_accel, dt);
        fastest = fastest.max(rate.abs());
        let d = wrap_angle(yaw - prev).abs();
        assert!(
            d <= c.turn_speed * dt + 1e-4,
            "turned {d:.3} rad in one step, faster than turn_speed"
        );
        if yaw == target && rate == 0.0 {
            return (i as f32 * dt, fastest);
        }
    }
    panic!("never got to {target:.2}: at {yaw:.2} turning {rate:.2}");
}

#[test]
fn turn_about_takes_a_moment() {
    let (secs, fastest) = turn_about(PI - 0.01);
    assert!(
        (0.5..=0.9).contains(&secs),
        "turning about took {secs:.2}s, want a walker's 0.5-0.9s"
    );
    assert!(
        fastest <= Character::default().turn_speed + 1e-3,
        "turned at {fastest:.2} rad/s, over turn_speed"
    );
    let (small, _) = turn_about(0.3);
    assert!(
        small < secs / 2.0,
        "a slight turn took {small:.2}s, near a full turn's {secs:.2}s"
    );
}

#[test]
fn turn_toward_takes_the_short_way() {
    let (yaw, rate) = turn_toward(3.0, 0.0, -3.0, 7.0, 30.0, 1.0 / 60.0);
    assert!(
        rate > 0.0 && yaw > 3.0,
        "from 3 to -3 rad: yaw {yaw:.3} rate {rate:.2}, want turning up through π"
    );
}

#[test]
fn turn_toward_keeps_turning_way_around() {
    // Straight behind is as far either way; mid-turn, it keeps its way.
    let (yaw, rate) = turn_toward(0.0, -2.0, PI, 7.0, 30.0, 1.0 / 60.0);
    assert!(
        rate < 0.0 && yaw < 0.0,
        "yaw {yaw:.3} rate {rate:.2}, want still turning the negative way"
    );
}

#[test]
fn stride_slips_and_crawls() {
    let ahead = Vec3::Z;
    let (dir, share) = stride(0.0, ahead);
    assert!(
        share == 1.0 && dir.distance(ahead) <= 1e-5,
        "heading where it faces: {dir} {share:.2}, want straight on at full speed"
    );
    let (dir, share) = stride(0.0, Vec3::NEG_Z);
    assert!(
        (share - CRAWL).abs() <= 1e-5,
        "heading straight back: share {share:.2}, want {CRAWL:.2}"
    );
    let a = dir.dot(ahead).acos();
    assert!(
        a <= SLIP + 1e-4,
        "heading straight back steps {a:.2} rad off its facing, more than slip"
    );
    let (_, side) = stride(0.0, Vec3::X);
    assert!(
        side > CRAWL && side < 1.0,
        "heading sideways: share {side:.2}, want between crawl and 1"
    );
}

#[test]
fn yaw_of_round_trips() {
    for yaw in [0.0f32, 1.0, -2.0, 3.0] {
        let got = yaw_of(Quat::from_axis_angle(Vec3::Y, yaw));
        assert!((got - yaw).abs() <= 1e-4, "yaw_of({yaw:.2}) = {got:.4}");
    }
}

#[test]
fn approach_steps_toward() {
    let got = approach(Vec3::ZERO, Vec3::new(3.0, 0.0, 4.0), 1.0);
    assert!(
        got.distance(Vec3::new(0.6, 0.0, 0.8)) <= 1e-5,
        "approach = {got}, want one unit toward (3, 0, 4)"
    );
    let got = approach(Vec3::X, Vec3::new(1.5, 0.0, 0.0), 1.0);
    assert_eq!(got.x, 1.5, "approach within a step = {got}, want there");
}

#[test]
fn airborne_keeps_to_the_jump() {
    let launch = Vec3::new(0.0, 0.0, 4.0);
    let angle = |v: Vec3| v.x.atan2(v.z);
    let side = airborne(launch, Vec3::new(4.0, 0.0, 0.0));
    let a = angle(side).abs();
    assert!(
        (AIR_STEER - 1e-4..=AIR_STEER + 1e-4).contains(&a),
        "steering sideways veered {:.1}°, want {:.1}°",
        a * 180.0 / PI,
        AIR_STEER * 180.0 / PI
    );
    let l = airborne(launch, Vec3::new(0.0, 0.0, 9.0)).length();
    assert!(l <= 4.0 + 1e-4, "sped up in the air to {l:.2}");
    let got = airborne(launch, Vec3::ZERO);
    assert_eq!(
        got, launch,
        "letting go: {got}, want to keep going {launch}"
    );
    let back = airborne(launch, Vec3::new(0.0, 0.0, -4.0));
    assert!(
        back.z > 0.0 && (back.z - 2.0).abs() <= 1e-4 && back.x.abs() <= 1e-4,
        "pulling back: {back}, want still forward at half speed"
    );
    let got = airborne(Vec3::ZERO, Vec3::new(4.0, 0.0, 0.0));
    assert_eq!(got, Vec3::ZERO, "a standing jump drifted: {got}");
}

/// Eases from speed from to to at Default's rates (braking twice as
/// quick), 60 times a second, returning how long it takes to get within 5%
/// of the change, and the hardest push in any one step.
fn speed_up(from: f32, to: f32) -> (f32, f32) {
    let c = Character::default();
    let dt = 1.0 / 60.0;
    let k = if to < from { BRAKE } else { 1.0 };
    let mut v = Vec3::new(from, 0.0, 0.0);
    let mut hardest = 0.0f32;
    for i in 1..=600 {
        let next = ease(v, Vec3::new(to, 0.0, 0.0), k * c.ease, k * c.accel, dt);
        hardest = hardest.max((next.x - v.x).abs() / dt);
        v = next;
        if (to - v.x).abs() <= 0.05 * (to - from).abs() {
            return (i as f32 * dt, hardest);
        }
    }
    panic!("never got from {from:.1} to {to:.1}: at {:.2}", v.x);
}

#[test]
fn ease_walk_to_run_and_back() {
    let c = Character::default();
    let (up, push) = speed_up(c.walk_speed, c.run_speed);
    assert!(
        (0.4..=1.0).contains(&up),
        "walk to run took {up:.2}s, want 0.4-1s"
    );
    assert!(push <= c.accel + 1e-3, "sped up at {push:.1}, over accel");
    let (down, _) = speed_up(c.run_speed, c.walk_speed);
    assert!(
        down < up,
        "run to walk took {down:.2}s, no quicker than walk to run's {up:.2}s"
    );
    let (start, _) = speed_up(0.0, c.walk_speed);
    assert!(
        (0.3..=0.9).contains(&start),
        "setting off took {start:.2}s, want 0.3-0.9s"
    );
}

#[test]
fn ease_eases_in() {
    // Well within the push cap, each step closes the same share of what's
    // left: the steps shrink as it gets close.
    let (mut v, want) = (Vec3::ZERO, Vec3::X);
    let mut last = f32::MAX;
    for _ in 0..10 {
        let next = ease(v, want, 5.0, 100.0, 1.0 / 60.0);
        let step = next.x - v.x;
        assert!(
            step < last && step > 0.0,
            "step {step:.4} after {last:.4}: want shrinking steps toward the target"
        );
        last = step;
        v = next;
    }
    let got = ease(Vec3::new(0.99, 0.0, 0.0), want, 5.0, 100.0, 1.0 / 60.0);
    assert_eq!(got, want, "within snap: {got}, want there");
}

#[test]
fn relative_movement() {
    let near = |a: Vec3, b: Vec3| a.distance(b) < 1e-5;
    let (ahead, left) = (Vec3::NEG_Z, Vec3::NEG_X);
    // Unset, and looking down -Z, the keys go as they say.
    for forward in [Vec3::ZERO, Vec3::NEG_Z] {
        let got = relative(ahead, forward);
        assert!(near(got, ahead), "W looking {forward} goes {got}");
    }
    // Looking down +X, W goes +X and A goes -Z.
    let got = relative(ahead, Vec3::X);
    assert!(near(got, Vec3::X), "W looking +X goes {got}");
    let got = relative(left, Vec3::X);
    assert!(near(got, Vec3::NEG_Z), "A looking +X goes {got}");
    // Looking down at a slant only its heading counts.
    let got = relative(ahead, Vec3::new(1.0, -2.0, 0.0));
    assert!(near(got, Vec3::X), "W looking down at +X goes {got}");
}

// --- traversal_test.go ---

#[test]
fn slide_roll_and_jump() {
    let mut h = Harness::new(Vec3::ZERO, |_| {});
    h.intent().slide = true;
    h.tick(1);
    assert!(
        h.mode() == Anim::Idle && !h.intent().slide,
        "slow slide should be consumed and rejected"
    );
    h.cc().walk = Vec3::new(0.0, 0.0, 4.0);
    h.cc().velocity.z = 4.0;
    h.intent().run = true;
    h.intent().move_dir = Vec3::Z;
    h.intent().slide = true;
    h.tick(1);
    let height = h.cc().height;
    assert!(
        h.mode() == Anim::Slide && height == 0.9,
        "slide mode={} height={height}",
        h.mode()
    );
    h.intent().jump = true;
    h.tick(1);
    assert!(
        !h.s().active() && h.cc().velocity.y > 0.0 && !h.intent().jump,
        "slide jump failed"
    );
    h.intent().move_dir = Vec3::ZERO;
    h.tick(90);
    let start = h.translation();
    h.intent().roll = true;
    h.tick(1);
    assert_eq!(h.mode(), Anim::Roll, "roll mode");
    h.tick(55);
    let distance = horizontal(start).distance(horizontal(h.translation()));
    assert!((1.8..=2.2).contains(&distance), "roll distance={distance}");
    h.intent().roll = true;
    h.tick(1);
    assert_eq!(h.mode(), Anim::Roll, "completed roll cannot chain");
}

#[test]
fn sprinting_roll_runs_on() {
    let mut h = Harness::new(Vec3::ZERO, |_| {});
    {
        let mut intent = h.intent();
        intent.run = true;
        intent.move_dir = Vec3::Z;
        intent.roll = true;
    }
    h.tick(1);
    assert_eq!(h.mode(), Anim::Roll, "roll mode");
    let mut steps = 0;
    while h.mode() == Anim::Roll && steps < 60 {
        h.tick(1);
        steps += 1;
    }
    let height = h.cc().height;
    assert!(
        !h.s().active() && height == CAPSULE_HEIGHT,
        "sprinting roll ended in mode {}, height {height}",
        h.mode()
    );
    let limit = (TraversalConfig::default().roll_time * ROLL_RELEASE * 60.0) as i32 + 2;
    assert!(
        steps <= limit,
        "sprinting roll took {steps} steps, want it to run on after {limit}"
    );
    let v = horizontal(h.cc().walk).length();
    assert!(
        v >= Character::default().walk_speed * 2.0,
        "ran on out of the roll at {v}, want it to keep its speed"
    );
    h.tick(30);
    let v = horizontal(h.cc().velocity).length();
    assert!(
        v >= Character::default().run_speed * 0.9,
        "not running after the roll: {v}"
    );
}

#[test]
fn sprinting_slide_runs_on() {
    let mut h = Harness::new(Vec3::ZERO, |_| {});
    h.cc().walk = Vec3::new(0.0, 0.0, 4.6);
    h.cc().velocity.z = 4.6;
    {
        let mut intent = h.intent();
        intent.run = true;
        intent.move_dir = Vec3::Z;
        intent.slide = true;
    }
    h.tick(1);
    assert_eq!(h.mode(), Anim::Slide, "slide mode");
    let mut steps = 0;
    while h.mode() == Anim::Slide && steps < 60 {
        h.tick(1);
        steps += 1;
    }
    let height = h.cc().height;
    assert!(
        !h.s().active() && height == CAPSULE_HEIGHT,
        "sprinting slide ended in mode {}, height {height}",
        h.mode()
    );
    let limit = (TraversalConfig::default().slide_time * SLIDE_RELEASE * 60.0) as i32 + 2;
    assert!(
        steps <= limit,
        "sprinting slide took {steps} steps, want it to run on after {limit}"
    );
    let v = horizontal(h.cc().walk).length();
    assert!(
        v >= Character::default().run_speed,
        "ran on out of the slide at {v}, want at least running speed"
    );
}

#[test]
fn slide_cannot_turn_round() {
    for (name, mv) in [
        ("pulling back", Vec3::NEG_Z),
        (
            "pulling back and to the side",
            Vec3::new(1.0, 0.0, -1.0).normalize(),
        ),
        ("steering hard to the side", Vec3::X),
    ] {
        let mut h = Harness::new(Vec3::ZERO, |_| {});
        h.cc().walk = Vec3::new(0.0, 0.0, 4.6);
        h.cc().velocity.z = 4.6;
        {
            let mut intent = h.intent();
            intent.run = true;
            intent.move_dir = mv;
            intent.slide = true;
        }
        h.tick(1);
        assert_eq!(h.mode(), Anim::Slide, "{name}: slide mode");
        let d = h.s().direction;
        assert!(
            d.z >= 0.99,
            "{name}: slide started going {d}, want the way it was running"
        );
        while h.mode() == Anim::Slide {
            let walk = h.cc().walk;
            assert!(walk.z >= 0.0, "{name}: slide went backwards: {walk}");
            let d = h.s().direction;
            let a = d.x.atan2(d.z);
            assert!(
                a.abs() <= SLIDE_STEER + 1e-3,
                "{name}: slide steered {}°, past {}°",
                a * 180.0 / PI,
                SLIDE_STEER * 180.0 / PI
            );
            h.tick(1);
        }
    }
}

#[test]
fn slide_steers_to_the_side() {
    let mut h = Harness::new(Vec3::ZERO, |_| {});
    h.cc().walk = Vec3::new(0.0, 0.0, 4.6);
    h.cc().velocity.z = 4.6;
    {
        let mut intent = h.intent();
        intent.run = true;
        intent.move_dir = Vec3::Z;
        intent.slide = true;
    }
    h.tick(1);
    h.intent().move_dir = Vec3::new(1.0, 0.0, 1.0).normalize();
    h.tick(20);
    let d = h.s().direction;
    assert!(
        h.mode() == Anim::Slide && d.x > 0.2,
        "slide did not steer right: {}, going {d}",
        h.mode()
    );
}

#[test]
fn low_ceiling_and_blocked_roll() {
    let mut h = Harness::new(Vec3::ZERO, |w| {
        static_box(w, Vec3::new(0.0, 1.2, 1.0), Vec3::new(3.0, 0.2, 1.4));
        static_box(w, Vec3::new(0.0, 1.0, 2.3), Vec3::new(3.0, 2.0, 0.1));
    });
    h.intent().move_dir = Vec3::Z;
    h.intent().roll = true;
    h.tick(65);
    assert!(h.translation().z <= 2.0, "roll passed through wall");
    let height = h.cc().height;
    assert!(
        h.mode() == Anim::Crouch && height == 0.9,
        "stood through ceiling: {} height={height}",
        h.mode()
    );
    h.intent().move_dir = Vec3::X;
    h.tick(180);
    assert_eq!(h.cc().height, 1.8, "did not stand in open space");
}

#[test]
fn vault_mantle_and_blocked_route() {
    for height in [0.75f32, 1.6] {
        let depth = if height > 1.0 { 2.0 } else { 0.5 };
        let mut h = Harness::new(Vec3::ZERO, |w| {
            static_box(
                w,
                Vec3::new(0.0, height / 2.0, -0.5 - depth / 2.0),
                Vec3::new(3.0, height, depth),
            );
        });
        h.face_to(Vec3::NEG_Z);
        h.intent().move_dir = Vec3::NEG_Z;
        h.intent().jump = true;
        h.tick(1);
        let want = if height < 1.0 {
            Anim::Vault
        } else {
            Anim::Mantle
        };
        let hint = h.s().hint.clone();
        assert_eq!(h.mode(), want, "mode want {want} hint={hint:?}");
        h.intent().move_dir = Vec3::ZERO;
        h.tick(75);
        assert!(!h.s().active(), "route did not complete");
        assert!(h.translation().z <= -0.5, "route did not cross obstacle");
        if height > 1.0 {
            assert!(
                h.translation().y >= height + 0.85,
                "mantle did not land on top"
            );
        }
    }
    let mut h = Harness::new(Vec3::ZERO, |w| {
        static_box(w, Vec3::new(0.0, 0.375, -0.75), Vec3::new(3.0, 0.75, 0.5));
        static_box(w, Vec3::new(0.0, 2.1, -0.8), Vec3::new(3.0, 0.2, 2.0));
    });
    h.intent().move_dir = Vec3::NEG_Z;
    h.intent().jump = true;
    h.tick(1);
    assert!(
        h.mode() != Anim::Vault && h.mode() != Anim::Mantle,
        "accepted route through ceiling"
    );
}

#[test]
fn ladder_climb_hold_detach_and_pause() {
    let mut h = Harness::new(Vec3::ZERO, |w| {
        ladder(
            w,
            Ladder {
                bottom: Vec3::new(0.0, 0.0, -0.2),
                top: Vec3::new(0.0, 5.0, -0.2),
                facing: Vec3::NEG_Z,
                bottom_exit: Vec3::new(0.0, 0.0, 0.4),
                top_exit: Vec3::new(0.0, 5.0, -0.2),
                width: 1.0,
            },
        );
    });
    h.intent().move_dir = Vec3::NEG_Z;
    h.tick(1);
    assert_eq!(h.mode(), Anim::LadderEnter, "ladder did not attach");
    h.tick(50);
    let y = h.translation().y;
    h.intent().move_dir = Vec3::ZERO;
    h.tick(20);
    assert!((h.translation().y - y).abs() <= 0.01, "ladder hold drifted");
    h.controls(false);
    h.pause(true);
    let elapsed = h.s().elapsed;
    h.tick(20);
    assert!(
        h.s().elapsed == elapsed && (h.translation().y - y).abs() <= 0.01,
        "pause advanced traversal"
    );
    h.controls(true);
    h.pause(false);
    h.intent().jump = true;
    h.tick(1);
    assert!(
        !h.s().active() && !h.cc().controlled && h.cc().velocity.y > 0.0 && h.s().detach > 0.0,
        "ladder jump failed"
    );
}

#[test]
fn wall_kick_guards_and_launch() {
    let mut h = Harness::new(Vec3::Y, |w| {
        static_box(w, Vec3::new(0.65, 2.0, 0.0), Vec3::new(0.1, 4.0, 4.0));
    });
    h.cc().grounded = false;
    h.intent().move_dir = Vec3::X;
    h.intent().jump = true;
    h.cc().walk = Vec3::new(2.0, 0.0, 0.0);
    h.tick(1);
    assert_eq!(h.s().kick, 0.0, "kicked off a wall without a sprint");
    h.intent().jump = true;
    h.cc().walk = Vec3::new(4.0, 0.0, 3.0);
    h.tick(1);
    // Like a ball: out the way it came in, reflected, and it can't steer.
    let (walk, vy, kick) = (h.cc().walk, h.cc().velocity.y, h.s().kick);
    assert!(
        kick != 0.0
            && vy > 0.0
            && (walk.x + 0.8 * 3.2).abs() <= 0.01
            && (walk.z - 0.6 * 3.2).abs() <= 0.01,
        "kick failed: {:?} {:?}",
        h.s().clone(),
        h.cc().clone()
    );
    h.intent().move_dir = Vec3::NEG_Z;
    h.tick(5);
    let walk = h.cc().walk;
    assert!(
        (walk.x + 0.8 * 3.2).abs() <= 0.01 && (walk.z - 0.6 * 3.2).abs() <= 0.01,
        "steered after a wall kick: {walk}"
    );
    h.intent().move_dir = Vec3::X;
    assert!(
        h.s().kick != 0.0 && h.cc().walk.x < 0.0 && h.cc().velocity.y > 0.0,
        "kick failed: {:?} {:?}",
        h.s().clone(),
        h.cc().clone()
    );
    let speed = h.cc().walk.x;
    h.intent().jump = true;
    h.tick(1);
    assert!(h.cc().walk.x >= speed - 0.1, "repeated kick bypassed guard");
    assert!(!h.intent().jump, "airborne jump request retained");
}

#[test]
fn wall_kick_uses_the_foot_nearer_the_wall() {
    for along in [4.0f32, -4.0] {
        let mut h = Harness::new(Vec3::Y, |w| {
            static_box(w, Vec3::new(0.65, 2.0, 0.0), Vec3::new(0.1, 4.0, 4.0));
        });
        h.cc().grounded = false;
        h.cc().walk = Vec3::new(3.0, 0.0, along);
        h.intent().move_dir = Vec3::X;
        h.intent().jump = true;
        h.tick(1);
        // Going +Z, a wall at +X is on the left.
        let s = h.s().clone();
        assert!(
            s.kick != 0.0 && s.kick_right == (along < 0.0),
            "going {along} along the wall: {s:?}"
        );
    }
}

#[test]
fn traversal_animations_are_not_stationary_actions() {
    for a in [
        Anim::Slide,
        Anim::Roll,
        Anim::LadderClimb,
        Anim::Vault,
        Anim::Mantle,
        Anim::WallKick,
        Anim::WallKickRight,
        Anim::Crouch,
    ] {
        assert!(!a.one_shot(), "{a} incorrectly locks stationary action");
    }
}

#[test]
fn thin_vault_barrier() {
    let mut h = Harness::new(Vec3::ZERO, |w| {
        static_box(w, Vec3::new(0.0, 0.375, -0.6), Vec3::new(3.0, 0.75, 0.15));
    });
    h.face_to(Vec3::NEG_Z);
    h.intent().move_dir = Vec3::NEG_Z;
    h.intent().jump = true;
    h.tick(1);
    assert_eq!(h.mode(), Anim::Vault, "thin barrier mode");
}

#[test]
fn ladder_automatic_exits() {
    for top in [false, true] {
        let name = if top { "top" } else { "bottom" };
        let mut h = Harness::new(Vec3::ZERO, |w| {
            static_box(w, Vec3::new(0.0, 0.5, -1.5), Vec3::new(3.0, 1.0, 1.0));
            ladder(
                w,
                Ladder {
                    bottom: Vec3::new(0.0, 0.0, -0.2),
                    top: Vec3::new(0.0, 1.04, -0.2),
                    facing: Vec3::NEG_Z,
                    bottom_exit: Vec3::new(0.0, 0.0, 0.4),
                    top_exit: Vec3::new(0.0, 1.04, -1.5),
                    width: 1.0,
                },
            );
        });
        h.intent().move_dir = Vec3::NEG_Z;
        h.tick(24);
        if !top {
            h.intent().move_dir.z = 1.0;
        }
        let mut entered_exit = false;
        for _ in 0..220 {
            let before = h.translation();
            h.tick(1);
            if h.mode() == Anim::LadderExit {
                entered_exit = true;
            }
            assert!(
                before.distance(h.translation()) <= 0.06,
                "{name}: ladder exit teleported"
            );
            if entered_exit && !h.s().active() {
                break;
            }
        }
        assert!(entered_exit, "{name}: missing ladder exit phase");
        let vy = h.cc().velocity.y;
        assert!(vy <= 0.01, "{name}: exit launched upward: {vy}");
        assert!(
            !h.s().active() && !h.cc().controlled && h.s().detach > 0.0,
            "{name}: did not exit ladder: {:?}",
            h.s().clone()
        );
        if top {
            let at = h.translation();
            assert!(at.y >= 1.8, "top exit too low {at}");
        }
    }
}

#[test]
fn wall_kicks_can_chain_between_walls() {
    let mut h = Harness::new(Vec3::Y, |w| {
        for x in [-0.65f32, 0.65] {
            static_box(w, Vec3::new(x, 2.0, 0.0), Vec3::new(0.1, 4.0, 4.0));
        }
    });
    h.intent().move_dir = Vec3::X;
    h.intent().jump = true;
    h.cc().walk = Vec3::new(4.6, 0.0, 0.0);
    h.tick(1);
    let first = h.s().last_wall;
    h.intent().move_dir = Vec3::NEG_X;
    h.tick(11);
    h.intent().jump = true;
    h.tick(1);
    assert!(
        h.s().last_wall != first && h.cc().walk.x > 0.0,
        "second wall kick did not chain: cc={:?} position={} state={:?} first={first:?}",
        h.cc().clone(),
        h.translation(),
        h.s().clone()
    );
}

#[test]
fn slide_gets_up_by_itself() {
    let mut h = Harness::new(Vec3::ZERO, |_| {});
    h.cc().velocity.z = 4.0;
    {
        let mut intent = h.intent();
        intent.move_dir = Vec3::Z;
        intent.run = true;
        intent.slide = true;
    }
    h.tick(1);
    h.intent().move_dir = Vec3::ZERO;
    h.tick(40);
    assert_eq!(h.mode(), Anim::Slide, "slide ended early");
    h.tick(10);
    // Its clip gets up out of it: no stand-up after.
    let height = h.cc().height;
    assert!(
        !h.s().active() && height == CAPSULE_HEIGHT,
        "slide did not end standing: {}, height {height}",
        h.mode()
    );
}

#[test]
fn ladder_phase_follows_distance_and_holds() {
    let mut h = Harness::new(Vec3::ZERO, |w| {
        let e = ladder(
            w,
            Ladder {
                bottom: Vec3::new(0.0, 0.0, -0.2),
                top: Vec3::new(0.0, 5.0, -0.2),
                facing: Vec3::NEG_Z,
                width: 1.0,
                ..Default::default()
            },
        );
        w.entity_mut(e).insert(RungSpacing(0.25));
    });
    h.intent().move_dir.z = -1.0;
    h.tick(80);
    assert_eq!(h.mode(), Anim::LadderClimb, "not climbing");
    let want = (h.translation().y - h.cc().height / 2.0 - h.cc().walk.y / 60.0 - LADDER_FOOT) / 0.5;
    let phase = h.s().phase;
    assert!(
        (phase - want).abs() <= 0.005,
        "phase={phase} distance phase={want}"
    );
    h.intent().move_dir = Vec3::ZERO;
    h.tick(1);
    let phase = h.s().phase;
    h.tick(20);
    assert!(
        (h.s().phase - phase).abs() <= 0.001,
        "hold advanced contact cycle"
    );
    h.intent().move_dir.z = 1.0;
    h.tick(15);
    assert!(
        h.s().phase < phase,
        "descending did not reverse contact cycle"
    );
}

#[test]
fn ladder_blocked_exit_holds_without_launching() {
    let mut h = Harness::new(Vec3::ZERO, |w| {
        ladder(
            w,
            Ladder {
                bottom: Vec3::new(0.0, 0.0, -0.2),
                top: Vec3::new(0.0, 2.84, -0.2),
                facing: Vec3::NEG_Z,
                top_exit: Vec3::new(0.0, 2.84, -1.5),
                width: 1.0,
                ..Default::default()
            },
        );
        static_box(w, Vec3::new(0.0, 4.0, -1.5), Vec3::new(2.0, 1.0, 1.0));
    });
    h.intent().move_dir.z = -1.0;
    h.tick(260);
    assert!(
        h.mode() == Anim::LadderClimb && h.s().hint == "Exit blocked · Space jump off",
        "blocked exit state: {:?}",
        h.s().clone()
    );
    let y = h.translation().y;
    h.tick(30);
    assert!(
        (h.translation().y - y).abs() <= 0.01 && h.cc().velocity.y.abs() <= 0.01,
        "blocked ladder exit drifted"
    );
    h.intent().jump = true;
    h.tick(1);
    assert!(
        !h.s().active() && h.cc().velocity.y > 0.0,
        "blocked exit trapped character"
    );
}

#[test]
fn slide_stops_before_its_leading_foot_reaches_wall() {
    let mut h = Harness::new(Vec3::ZERO, |w| {
        static_box(w, Vec3::new(0.0, 1.0, 2.0), Vec3::new(4.0, 2.0, 0.2));
    });
    h.cc().walk = Vec3::new(0.0, 0.0, 5.0);
    h.cc().velocity.z = 5.0;
    {
        let mut intent = h.intent();
        intent.move_dir = Vec3::Z;
        intent.run = true;
        intent.slide = true;
    }
    h.tick(1);
    for _ in 0..70 {
        h.tick(1);
        if h.mode() != Anim::Slide {
            break;
        }
    }
    assert_ne!(h.mode(), Anim::Slide, "slide never stopped");
    let z = h.translation().z;
    assert!(
        z <= 0.3 + 1.0,
        "capsule stopped too close for leading foot: {z}"
    );
}

#[test]
fn slide_recovery_allows_movement_jump_and_roll() {
    for next in ["walk", "jump", "roll"] {
        let mut h = Harness::new(Vec3::ZERO, |_| {});
        {
            let world = h.app.world_mut();
            let mut cc = *world.get::<CharacterController>(h.root).unwrap();
            let mut s = world.get_mut::<Traversal>(h.root).unwrap();
            stand_up(&mut s, &mut cc);
            *world.get_mut::<CharacterController>(h.root).unwrap() = cc;
        }
        h.intent().move_dir = Vec3::Z;
        if next == "jump" {
            h.intent().jump = true;
        }
        if next == "roll" {
            h.intent().roll = true;
        }
        let start = h.translation();
        h.tick(1);
        if next == "jump" {
            assert!(
                h.cc().velocity.y > 0.0 && !h.s().active(),
                "recovery swallowed jump"
            );
        }
        if next == "roll" {
            assert_eq!(h.mode(), Anim::Roll, "recovery swallowed chained roll");
        }
        if next == "walk" {
            h.tick(16);
            assert!(
                h.translation().z > start.z + 0.05,
                "recovery locked movement"
            );
        }
    }
}

#[test]
fn obstacles_and_walls_need_the_body_near() {
    let mut h = Harness::new(Vec3::ZERO, |w| {
        static_box(w, Vec3::new(0.0, 0.375, -1.15), Vec3::new(3.0, 0.75, 0.5));
    });
    h.intent().move_dir = Vec3::NEG_Z;
    h.intent().jump = true;
    h.tick(1);
    let hint = h.s().hint.clone();
    assert!(
        h.mode() != Anim::Vault && hint.is_empty(),
        "vaulted from out of reach: {} hint={hint:?}",
        h.mode()
    );
    let mut h = Harness::new(Vec3::ZERO, |w| {
        ladder(
            w,
            Ladder {
                bottom: Vec3::new(0.0, 0.0, -0.4),
                top: Vec3::new(0.0, 5.0, -0.4),
                facing: Vec3::NEG_Z,
                width: 1.0,
                ..Default::default()
            },
        );
    });
    h.intent().move_dir = Vec3::NEG_Z;
    h.tick(1);
    assert!(!h.s().active(), "ladder attached from out of reach");
    let mut h = Harness::new(Vec3::Y, |w| {
        static_box(w, Vec3::new(0.75, 2.0, 0.0), Vec3::new(0.1, 4.0, 4.0));
    });
    h.cc().grounded = false;
    h.intent().move_dir = Vec3::X;
    h.intent().jump = true;
    h.tick(1);
    assert_eq!(h.s().kick, 0.0, "kicked a wall out of reach");
}

#[test]
fn wall_kick_landing_keeps_a_sprint_going() {
    for run in [false, true] {
        let mut h = Harness::new(Vec3::ZERO, |_| {});
        h.s().bounced = true;
        h.cc().grounded = true;
        h.cc().walk = Vec3::new(0.0, 0.0, 3.2); // the way the body faces
        h.intent().move_dir = Vec3::Z;
        h.intent().run = run;
        h.tick(1);
        let want = if run {
            0.0
        } else {
            TraversalConfig::default().land_delay
        };
        assert!(
            !h.s().bounced && (h.s().land - want).abs() <= 0.001,
            "run {run}: landing {:?}",
            h.s().clone()
        );
        h.tick(6);
        // Walking it pulls up for the landing; sprinting it runs straight on.
        let speed = h.cc().walk.z;
        assert!(
            run == (speed > 3.2),
            "run {run}: going {speed} through the landing"
        );
    }
}

#[test]
fn vault_waits_to_face_the_obstacle() {
    let barrier = |w: &mut World| {
        static_box(w, Vec3::new(0.0, 0.375, -0.75), Vec3::new(3.0, 0.75, 0.5));
    };
    let mut h = Harness::new(Vec3::ZERO, barrier);
    // Its back to the barrier: asked to vault, it has to turn round first.
    h.intent().move_dir = Vec3::NEG_Z;
    h.intent().jump = true;
    h.tick(1);
    let (mode, jump, grounded) = (h.mode(), h.intent().jump, h.cc().grounded);
    assert!(
        mode == Anim::Idle && !jump && grounded,
        "facing away: mode={mode} jump={jump} grounded={grounded}, want it waiting on the ground"
    );
    h.face_to(Vec3::NEG_Z);
    h.tick(1);
    assert_eq!(h.mode(), Anim::Vault, "turned to face it: want vault");

    // Asked, but still facing away when the wait runs out: it stays put.
    let mut h = Harness::new(Vec3::ZERO, barrier);
    h.intent().move_dir = Vec3::NEG_Z;
    h.intent().jump = true;
    h.tick(60);
    h.face_to(Vec3::NEG_Z);
    h.tick(1);
    assert_eq!(h.mode(), Anim::Idle, "after the wait: want idle");
}

#[test]
fn mantle_from_the_air() {
    let mut h = Harness::new(Vec3::ZERO, |w| {
        static_box(w, Vec3::new(2.0, 2.0, 0.0), Vec3::new(3.0, 4.0, 16.0));
    });
    // In the air beside the block, its top 1.5 m above the feet, facing
    // along it and heading into it at an angle, as off a wall kick.
    h.tr().translation = Vec3::new(0.1, 2.5 + 0.9, 0.0);
    h.cc().grounded = false;
    h.intent().move_dir = direction(Vec3::new(1.0, 0.0, 1.0));
    h.intent().jump = true;
    h.tick(1);
    let (mode, dir, hint) = (h.mode(), h.s().direction, h.s().hint.clone());
    assert!(
        mode == Anim::Mantle && dir == Vec3::X,
        "mode={mode} direction={dir} hint={hint:?}, want a mantle square on to the block"
    );
    h.tick(60);
    let at = h.translation();
    assert!(
        !h.s().active() && h.cc().grounded && at.y >= 4.8 && at.x >= 0.5,
        "after the mantle: mode={} grounded={} at {at}, want standing on the block",
        h.mode(),
        h.cc().grounded
    );
}

/// Holding C crouches, and walks crouched; letting go stands it up, once
/// there's room.
#[test]
fn hold_c_to_crouch() {
    let mut h = Harness::new(Vec3::ZERO, |w| {
        static_box(w, Vec3::new(5.0, 1.2, 0.0), Vec3::new(2.0, 0.2, 3.0));
    });
    h.face_to(Vec3::X);
    h.intent().crouch = true;
    h.tick(2);
    let height = h.cc().height;
    assert!(
        h.mode() == Anim::Crouch && height == 0.9,
        "holding C should crouch: {}, height {height}",
        h.mode()
    );
    h.tick(60);
    let height = h.cc().height;
    assert!(
        h.mode() == Anim::Crouch && height == 0.9,
        "held, it should stay down in the open: {}, height {height}",
        h.mode()
    );
    h.intent().crouch = false;
    h.tick(30);
    let height = h.cc().height;
    assert!(
        h.mode() != Anim::Crouch && height == 1.8,
        "let go, it should stand: {}, height {height}",
        h.mode()
    );

    // Held, it walks crouched, slowly, in under the slab at x 4 to 6 (its
    // underside 1.1 up); let go there, it stays down until it's out.
    h.intent().crouch = true;
    h.tick(2);
    let start = h.translation();
    h.intent().move_dir = Vec3::X;
    h.tick(60);
    let gone = h.translation().x - start.x;
    assert!(
        (0.5..=1.0).contains(&gone),
        "it should walk crouched, slowly: went {gone} m in a second"
    );
    for _ in 0..600 {
        h.tick(1);
        if h.translation().x > 5.0 {
            break;
        }
    }
    h.intent().crouch = false;
    h.tick(10);
    let (at, height) = (h.translation(), h.cc().height);
    assert!(
        at.x >= 4.5 && h.mode() == Anim::Crouch && height == 0.9,
        "under the slab it can't stand: at {at}, {}, height {height}",
        h.mode()
    );
    for _ in 0..600 {
        h.tick(1);
        if h.translation().x > 7.0 {
            break;
        }
    }
    h.intent().move_dir = Vec3::ZERO;
    h.tick(30);
    let (at, height) = (h.translation(), h.cc().height);
    assert!(
        at.x >= 7.0 && h.mode() != Anim::Crouch && height == 1.8,
        "out from under it, it should stand: at {at}, {}, height {height}",
        h.mode()
    );
}

// --- contacts_test.go ---

fn contact_test_limb() -> (Vec<BoneTransform>, Vec<BoneInfo>, LimbContact) {
    let pose = [
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.65, 0.30),
        Vec3::new(0.0, 0.35, 0.50),
        Vec3::new(0.0, 0.30, 0.64),
    ]
    .into_iter()
    .map(|p| BoneTransform {
        translation: p,
        ..Default::default()
    })
    .collect();
    let bones = [-1, 0, 1, 2]
        .into_iter()
        .map(|parent| BoneInfo {
            name: String::new(),
            parent,
        })
        .collect();
    let limb = LimbContact {
        upper: 0,
        middle: 1,
        end: 2,
        radius: 0.10,
        tips: vec![
            ContactPoint {
                bone: 2,
                radius: 0.075,
            },
            ContactPoint {
                bone: 3,
                radius: 0.10,
            },
        ],
        ..Default::default()
    };
    (pose, bones, limb)
}

#[test]
#[allow(clippy::approx_constant)] // the Go test's angles, as they are
fn limb_contacts_preserve_lengths_and_clear_sole() {
    for angle in [0.0f32, 0.7, 1.57, 3.14] {
        let (mut pose, bones, mut limb) = contact_test_limb();
        let q = Quat::from_axis_angle(Vec3::Y, angle);
        for bone in &mut pose {
            bone.translation = q * bone.translation;
            bone.rotation = q;
        }
        let plane = ContactPlane {
            point: q * Vec3::new(0.0, 0.0, 0.34),
            normal: q * Vec3::NEG_Z,
        };
        let a = pose[0].translation.distance(pose[1].translation);
        let b = pose[1].translation.distance(pose[2].translation);
        assert!(
            character::fit_limb(&mut pose, &bones, &mut limb, &[plane], false, 1.0 / 60.0),
            "penetrating pose wasn't corrected"
        );
        assert!(
            (pose[0].translation.distance(pose[1].translation) - a).abs() <= 0.0001
                && (pose[1].translation.distance(pose[2].translation) - b).abs() <= 0.0001,
            "contact stretched a limb"
        );
        assert!(
            character::plane_distance(pose[1].translation, plane) >= limb.radius - 0.001,
            "knee penetrates: {}",
            pose[1].translation
        );
        for tip in &limb.tips {
            assert!(
                character::plane_distance(pose[tip.bone].translation, plane) >= tip.radius - 0.001,
                "sole penetrates: {pose:?}"
            );
        }
    }
}

#[test]
fn limb_corner_and_release() {
    let (mut pose, bones, mut limb) = contact_test_limb();
    let original = pose.clone();
    let planes = [
        ContactPlane {
            point: Vec3::new(0.0, 0.0, 0.35),
            normal: Vec3::NEG_Z,
        },
        ContactPlane {
            point: Vec3::new(0.16, 0.0, 0.0),
            normal: Vec3::NEG_X,
        },
    ];
    character::fit_limb(&mut pose, &bones, &mut limb, &planes, false, 1.0 / 60.0);
    for p in planes {
        assert!(
            character::plane_distance(pose[1].translation, p) >= limb.radius - 0.001,
            "knee folded through corner"
        );
    }
    let offset = limb.offset.length();
    pose.copy_from_slice(&original);
    character::fit_limb(&mut pose, &bones, &mut limb, &[], false, 1.0 / 60.0);
    let got = limb.offset.length();
    assert!(
        got > 0.0 && got < offset,
        "contact release should blend back"
    );
    for _ in 0..60 {
        pose.copy_from_slice(&original);
        character::fit_limb(&mut pose, &bones, &mut limb, &[], false, 1.0 / 60.0);
    }
    assert!(
        limb.offset.length() <= 0.001,
        "contact offset survived leaving the wall"
    );
}

#[test]
fn rung_foot_stays_planted_while_knee_folds() {
    let (mut pose, bones, mut limb) = contact_test_limb();
    pose[1].translation = Vec3::new(0.0, 0.65, 0.5);
    pose[2].translation = Vec3::new(0.0, 0.35, 0.2);
    pose[3].translation = Vec3::new(0.0, 0.3, 0.3);
    let foot = pose[2].translation;
    let plane = ContactPlane {
        point: Vec3::new(0.0, 0.0, 0.4),
        normal: Vec3::NEG_Z,
    };
    character::fit_limb(&mut pose, &bones, &mut limb, &[plane], true, 1.0 / 60.0);
    assert!(
        pose[2].translation.distance(foot) <= 0.0001,
        "knee correction moved planted foot"
    );
    assert!(
        character::plane_distance(pose[1].translation, plane) >= limb.radius - 0.001,
        "knee passed through ladder backing wall"
    );
}

/// A held pose fitted to the world each frame, as the animation module
/// will do with the sampled one: the pose, its bones, and whether the last
/// fit corrected anything.
#[derive(Resource)]
struct Fitting {
    pose: Vec<BoneTransform>,
    held: Vec<BoneTransform>,
    bones: Vec<BoneInfo>,
    rig: ContactRig,
    corrected: bool,
}

fn fit_held_pose(
    mut fitting: ResMut<Fitting>,
    roots: Query<(Entity, &Transform, &Traversal), Without<Body>>,
    bodies: Query<(&ChildOf, &Transform), With<Body>>,
    ladders: Query<&Ladder>,
    physics: CharacterPhysics,
) {
    let (parent, body) = bodies.single().unwrap();
    let (root, root_tr, traversal) = roots.get(parent.parent()).unwrap();
    let matrix = root_tr.to_matrix() * body.to_matrix();
    let fitting = &mut *fitting;
    fitting.pose.copy_from_slice(&fitting.held);
    let climbing = matches!(traversal.mode, Anim::LadderClimb | Anim::LadderEnter);
    let context = FitContext {
        climbing,
        ladder: climbing
            .then(|| traversal.ladder.and_then(|l| ladders.get(l).ok()))
            .flatten()
            .map(|l| (l.bottom, l.facing)),
        exiting: traversal.mode == Anim::LadderExit,
    };
    let cast =
        |origin: Vec3, dir: Vec3, max: f32| physics.cast_ray_excluding(origin, dir, max, root);
    let solid =
        |hit: &character::RayHit| physics.static_surface(hit.entity) && hit.normal.y.abs() <= 0.2;
    fitting.corrected = character::fit_pose(
        &mut fitting.rig,
        &mut fitting.pose,
        &fitting.bones,
        matrix,
        root_tr.translation,
        context,
        &cast,
        &solid,
        1.0 / 60.0,
    );
}

fn bone(name: &str, parent: i32) -> BoneInfo {
    BoneInfo {
        name: name.to_string(),
        parent,
    }
}

fn held(points: &[Vec3]) -> Vec<BoneTransform> {
    points
        .iter()
        .map(|p| BoneTransform {
            translation: *p,
            ..Default::default()
        })
        .collect()
}

/// A standing character with a held pose, no gravity, in a world of
/// `setup`'s making, fitted every frame.
fn fitting_app(
    center: Vec3,
    bones: Vec<BoneInfo>,
    pose: Vec<BoneTransform>,
    setup: impl FnOnce(&mut World),
) -> (App, Entity) {
    let mut app = physics_app();
    let world = app.world_mut();
    setup(world);
    let root = world
        .spawn((
            Transform::from_translation(center),
            CharacterController {
                radius: 0.3,
                height: 1.8,
                controlled: true,
                ..Default::default()
            },
            Traversal::default(),
        ))
        .with_children(|parent| {
            parent.spawn((Body, State::default(), Transform::from_xyz(0.0, -0.9, 0.0)));
        })
        .id();
    let rig = character::make_contact_rig(&bones);
    app.insert_resource(Fitting {
        pose: pose.clone(),
        held: pose,
        bones,
        rig,
        corrected: false,
    })
    .add_systems(
        PostUpdate,
        fit_held_pose.before(TransformSystems::Propagate),
    );
    app.finish();
    app.cleanup();
    (app, root)
}

#[test]
fn held_air_pose_still_responds_to_world_contacts() {
    let bones = vec![bone("pelvis", -1), bone("head", 0)];
    let pose = held(&[Vec3::new(0.0, 0.9, 0.0), Vec3::new(0.0, 1.5, 0.3)]);
    let (mut app, root) = fitting_app(Vec3::new(0.0, 0.9, 0.0), bones, pose, |w| {
        static_box(w, Vec3::new(0.0, 1.5, 0.4), Vec3::new(4.0, 3.0, 0.1));
    });
    app.update();
    let fit = app.world().resource::<Fitting>();
    assert!(
        fit.corrected && fit.pose[1].translation.z <= 0.165,
        "held head pose penetrates wall"
    );
    let before = fit.pose[1].translation.z;
    app.world_mut()
        .get_mut::<Transform>(root)
        .unwrap()
        .translation
        .z = -0.2;
    app.update();
    let fit = app.world().resource::<Fitting>();
    assert!(
        fit.corrected && fit.pose[1].translation.z > before,
        "holding an animation froze wall contact correction"
    );
    for _ in 0..40 {
        app.update();
    }
    assert!(
        !app.world().resource::<Fitting>().corrected,
        "wall clearance survived moving away"
    );
}

/// The authored climb puts the knees between the rails, level with the
/// rungs. Only the backing wall may bend them; the ladder itself must not.
#[test]
fn ladder_geometry_leaves_climb_pose_alone() {
    let names = [
        "thigh_l", "calf_l", "foot_l", "ball_l", "thigh_r", "calf_r", "foot_r", "ball_r",
    ];
    // Traversal_Ladder at phase 0 on the man, facing +Z.
    let points = [
        Vec3::new(0.115, 0.975, 0.0),
        Vec3::new(0.192, 0.741, 0.344),
        Vec3::new(0.14, 0.27, 0.285),
        Vec3::new(0.183, 0.201, 0.411),
        Vec3::new(-0.113, 0.979, 0.0),
        Vec3::new(-0.236, 0.973, 0.404),
        Vec3::new(-0.14, 0.52, 0.285),
        Vec3::new(-0.183, 0.451, 0.411),
    ];
    let bones: Vec<BoneInfo> = names
        .iter()
        .enumerate()
        .map(|(i, name)| bone(name, if i % 4 == 0 { -1 } else { i as i32 - 1 }))
        .collect();
    let mut ladder_entity = Entity::PLACEHOLDER;
    let (mut app, root) = fitting_app(Vec3::new(0.0, 0.92, 0.0), bones, held(&points), |w| {
        // The traversal course's ladder: rails and rungs 37 cm ahead, wall
        // at 65 cm.
        static_box(w, Vec3::new(0.0, 1.4, 1.65), Vec3::new(3.0, 2.8, 2.0));
        for x in [-0.32f32, 0.32] {
            static_box(w, Vec3::new(x, 1.45, 0.37), Vec3::new(0.07, 2.9, 0.07));
        }
        let mut y = 0.2f32;
        while y < 2.9 {
            static_box(w, Vec3::new(0.0, y, 0.37), Vec3::new(0.71, 0.045, 0.06));
            y += 0.25;
        }
        ladder_entity = w
            .spawn((
                Ladder {
                    top: Vec3::new(0.0, 2.84, 0.0),
                    facing: Vec3::Z,
                    width: 0.8,
                    ..Default::default()
                },
                RungSpacing(0.25),
            ))
            .id();
    });
    app.update();
    // The exit starts from the same pose, still on the rungs.
    for mode in [Anim::LadderClimb, Anim::LadderExit] {
        {
            let mut s = app.world_mut().get_mut::<Traversal>(root).unwrap();
            s.mode = mode;
            s.ladder = Some(ladder_entity);
        }
        app.update();
        let fit = app.world().resource::<Fitting>();
        assert!(
            !fit.corrected,
            "{mode}: ladder splayed the knees: {} {}",
            fit.pose[1].translation, fit.pose[5].translation
        );
    }
}
