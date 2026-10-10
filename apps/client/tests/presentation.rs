//! Headless tests of the presentation systems in a Bevy app: the ports of
//! the Go tests that ran a whole illusion app.

use bevy::animation::AnimationPlayer as BevyPlayer;
use bevy::animation::graph::AnimationGraphHandle;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use earth_two_client::character::{
    Anim, Body, CAPSULE_HEIGHT, Character, CharacterController, Controls, Intent, State, Traversal,
};
use earth_two_client::presentation::{
    AnimationPlayer, Clip, ClipLibrary, Libraries, MotionSamples, PresentationPlugin, Roster, Skin,
    Wardrobe,
    anim::WALL_KICK_TIME,
    bones::{BoneAttachment, PoseClip, PoseClips, PoseSource, Skeleton},
    cloth::{Clothed, Rigid},
    distant::Distant,
    graph::{Animated, Graphs},
    mesh::{ModelMeshes, ModelStore, SkinnedMeshData},
    outfit::{BodyWardrobe, Garment, Item, ModelParts, ModelPath, Outfit, Slot},
    verlet::Cloth,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const TICK: Duration = Duration::from_nanos(16_666_667);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        bevy::asset::AssetPlugin::default(),
        PresentationPlugin::default(),
    ))
    .insert_resource(TimeUpdateStrategy::ManualDuration(TICK))
    .add_systems(
        Update,
        earth_two_client::character::locomotion::face
            .in_set(earth_two_client::character::CharacterSystems::Act),
    );
    app.finish();
    app.cleanup();
    app
}

fn tick(app: &mut App, n: u32) {
    for _ in 0..n {
        app.update();
    }
}

// character/animate_test.go TestWallKickAnimationSelectionAndRecovery.
#[test]
fn wall_kick_animation_selection_and_recovery() {
    let mut app = app();
    let clips = [
        ("Traversal_WallKick", 37),
        ("idle", 61),
        ("jump", 61),
        ("Traversal_WallKickFall", 37),
        ("Traversal_WallLand", 28),
    ];
    app.insert_resource(Libraries {
        by_model: HashMap::from([(
            "m".to_string(),
            ClipLibrary::keyframed(clips.iter().map(|(n, k)| (n.to_string(), *k))),
        )]),
    });
    app.insert_resource(Roster {
        skins: vec![Skin::new(
            "m",
            HashMap::from([
                (Anim::Idle, Clip::named("idle")),
                (Anim::Jump, Clip::named("jump")),
                (Anim::WallKick, Clip::named("Traversal_WallKick")),
                (Anim::WallFall, Clip::named("Traversal_WallKickFall")),
                (Anim::WallLand, Clip::named("Traversal_WallLand")),
            ]),
            1.0,
        )],
    });
    let root = app
        .world_mut()
        .spawn((
            Character::default(),
            Intent::default(),
            CharacterController::default(),
            Traversal {
                kick: WALL_KICK_TIME,
                wall_normal: Vec3::Z,
                ..Default::default()
            },
            MotionSamples::default(),
            Transform::IDENTITY,
        ))
        .id();
    let body = app
        .world_mut()
        .spawn((
            Body,
            State {
                current: Anim::Jump,
                ..Default::default()
            },
            AnimationPlayer::new("m"),
            Transform::IDENTITY,
            ChildOf(root),
        ))
        .id();
    let state = |app: &App| *app.world().get::<State>(body).unwrap();
    let player = |app: &App| app.world().get::<AnimationPlayer>(body).unwrap().clone();

    tick(&mut app, 1);
    assert!(
        state(&app).current == Anim::WallKick
            && player(&app).clip() == "Traversal_WallKick"
            && player(&app).manual_time,
        "kick did not select dedicated clip: {:?}",
        state(&app)
    );
    app.world_mut().get_mut::<Traversal>(root).unwrap().kick = WALL_KICK_TIME / 2.0;
    tick(&mut app, 1);
    let t = player(&app).time();
    assert!((0.25..=0.31).contains(&t), "kick progress={t}");

    app.world_mut().resource_mut::<Controls>().enabled = false;
    let stamp = player(&app).time();
    let rotation = app.world().get::<Transform>(body).unwrap().rotation;
    app.insert_resource(TimeUpdateStrategy::ManualDuration(TICK * 2));
    tick(&mut app, 1);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(TICK));
    assert!(
        player(&app).time() == stamp
            && app.world().get::<Transform>(body).unwrap().rotation == rotation
            && player(&app).paused,
        "pause advanced kick pose/facing"
    );

    app.world_mut().resource_mut::<Controls>().enabled = true;
    app.world_mut().get_mut::<Traversal>(root).unwrap().kick = WALL_KICK_TIME; // another wall
    tick(&mut app, 1);
    assert!(
        player(&app).time() <= 0.03 && !player(&app).paused,
        "chained bounce did not restart pose"
    );

    app.world_mut().get_mut::<Traversal>(root).unwrap().kick = 0.0; // over, still in the air
    tick(&mut app, 1);
    let p = player(&app);
    assert!(
        state(&app).current == Anim::WallFall
            && p.clip() == "Traversal_WallKickFall"
            && !p.manual_time
            && !p.paused,
        "kick did not go on into its fall: {} {:?}",
        state(&app).current,
        p.clip()
    );
    app.world_mut()
        .get_mut::<CharacterController>(root)
        .unwrap()
        .grounded = true;
    tick(&mut app, 1);
    let p = player(&app);
    assert!(
        state(&app).current == Anim::WallLand && p.clip() == "Traversal_WallLand" && !p.manual_time,
        "landing did not play: {} {:?}",
        state(&app).current,
        p.clip()
    );
}

/// A walking character's player keeps pace with the ground speed, and a
/// standing one idles.
#[test]
fn walk_speed_follows_the_ground() {
    let mut app = app();
    app.insert_resource(Libraries {
        by_model: HashMap::from([(
            "m".to_string(),
            ClipLibrary::keyframed([("idle".to_string(), 61), ("walk".to_string(), 61)]),
        )]),
    });
    app.insert_resource(Roster {
        skins: vec![Skin::new(
            "m",
            HashMap::from([
                (Anim::Idle, Clip::named("idle")),
                (Anim::Walk, Clip::named("walk")),
            ]),
            1.0,
        )],
    });
    let center = Vec3::new(0.0, CAPSULE_HEIGHT / 2.0, 0.0);
    let root = app
        .world_mut()
        .spawn((
            Character::default(),
            Intent {
                move_dir: Vec3::Z,
                ..Default::default()
            },
            CharacterController {
                grounded: true,
                velocity: Vec3::new(0.0, 0.0, 1.6),
                ground_normal: Vec3::Y,
                height: CAPSULE_HEIGHT,
                ..Default::default()
            },
            MotionSamples::new(center, center + Vec3::new(0.0, 0.0, 1.6 / 60.0)),
            Traversal::default(),
            Transform::from_translation(center),
        ))
        .id();
    let body = app
        .world_mut()
        .spawn((
            Body,
            State::default(),
            AnimationPlayer::new("m"),
            Transform::IDENTITY,
            ChildOf(root),
        ))
        .id();
    tick(&mut app, 2);
    let st = app.world().get::<State>(body).unwrap();
    let p = app.world().get::<AnimationPlayer>(body).unwrap();
    assert_eq!(st.current, Anim::Walk);
    assert_eq!(p.clip(), "walk");
    assert!((p.speed - 1.0).abs() < 1e-3, "walk speed {}", p.speed);
}

// render/bones_test.go, in an app: attachments follow the bone and an
// unknown bone leaves the transform alone.
#[test]
fn bone_attachments_follow_bones() {
    let mut app = app();
    let skeleton = Arc::new(Skeleton {
        names: vec!["Root".into(), "Tip".into()],
        parents: vec![-1, 0],
        bind: vec![Transform::IDENTITY, Transform::from_xyz(0.0, 1.0, 0.0)],
    });
    let bend = PoseClip {
        name: "Bend".into(),
        keyframes: [0.0f32, 90.0, 0.0]
            .iter()
            .map(|d| {
                vec![
                    Transform::IDENTITY,
                    Transform::from_xyz(0.0, 1.0, 0.0)
                        .with_rotation(Quat::from_rotation_z(d.to_radians())),
                ]
            })
            .collect(),
    };
    let clips = Arc::new(PoseClips {
        clips: vec![bend],
        frame_rate: 60.0,
    });
    app.insert_resource(Libraries {
        by_model: HashMap::from([("m".to_string(), clips.library())]),
    });
    let source = PoseSource {
        skeleton,
        clips,
        transform: Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)),
    };
    let mut player = AnimationPlayer::new("m");
    player.play("Bend");
    let model = app
        .world_mut()
        .spawn((source.clone(), player, Transform::IDENTITY))
        .id();
    let tip = app
        .world_mut()
        .spawn((
            Transform::IDENTITY,
            BoneAttachment {
                bone: "Tip".into(),
                offset: Transform::from_xyz(0.0, 0.5, 0.0),
            },
            ChildOf(model),
        ))
        .id();
    let still = app.world_mut().spawn((source, Transform::IDENTITY)).id();
    let tail = app
        .world_mut()
        .spawn((
            Transform::IDENTITY,
            BoneAttachment {
                bone: "Tail".into(),
                offset: Transform::IDENTITY,
            },
            ChildOf(still),
        ))
        .id();
    let bind = app
        .world_mut()
        .spawn((
            Transform::IDENTITY,
            BoneAttachment {
                bone: "Tip".into(),
                offset: Transform::IDENTITY,
            },
            ChildOf(still),
        ))
        .id();
    tick(&mut app, 2); // the first frame has no delta; then keyframe 1: the tip turned 90° about Z
    let tr = app.world().get::<Transform>(tip).unwrap();
    assert!(
        tr.translation.distance(Vec3::new(1.5, 1.0, 0.0)) < 1e-3,
        "{tr:?}"
    );
    assert_eq!(
        *app.world().get::<Transform>(tail).unwrap(),
        Transform::IDENTITY
    );
    let tr = app.world().get::<Transform>(bind).unwrap();
    assert!(
        tr.translation.distance(Vec3::new(2.0, 1.0, 0.0)) < 1e-3,
        "{tr:?}"
    );
}

fn test_wardrobe() -> Wardrobe {
    let mut man = BodyWardrobe {
        name: "man".into(),
        skin_meshes: vec![0, 1, 2],
        regions: HashMap::from([
            ("torso".to_string(), vec![0]),
            ("hips".to_string(), vec![1]),
        ]),
        underwear: HashMap::from([("hips".to_string(), vec![3])]),
        tones: vec![earth_two_client::presentation::outfit::Tone {
            name: "Fair".into(),
            texture: "skins/fair.jpg".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    *man.items_mut(Slot::Top) = vec![Item {
        name: "Polo".into(),
        model: "man/top/polo.glb".into(),
        hides: vec!["torso".into()],
        skin_meshes: vec![1],
        ..Default::default()
    }];
    *man.items_mut(Slot::Bottom) = vec![Item {
        name: "Cargo".into(),
        model: "man/bottom/cargo.glb".into(),
        hides: vec!["hips".into()],
        covers: vec!["hips".into()],
        ..Default::default()
    }];
    *man.items_mut(Slot::Shoes) = vec![Item {
        name: "Boots".into(),
        model: "man/shoes/boots.glb".into(),
        ..Default::default()
    }];
    Wardrobe { bodies: vec![man] }
}

fn spawn_dressed(app: &mut App, outfit: Outfit) -> (Entity, Entity) {
    let roster = Roster {
        skins: vec![Skin::new(
            "man.glb",
            HashMap::from([(Anim::Idle, Clip::named("idle"))]),
            1.0,
        )],
    };
    app.insert_resource(roster.clone());
    app.insert_resource(test_wardrobe());
    let mut commands = app.world_mut().commands();
    let root = roster.spawn(&mut commands, 0, Vec3::ZERO, 0.0);
    commands.entity(root).insert(outfit);
    app.world_mut().flush();
    let body = app.world().get::<Children>(root).unwrap()[0];
    (root, body)
}

fn garments(app: &mut App, body: Entity) -> Vec<(Slot, ModelPath, ModelParts, Option<String>)> {
    let mut out = vec![];
    let mut q = app
        .world_mut()
        .query::<(&ChildOf, &Garment, &ModelPath, &ModelParts)>();
    for (parent, g, m, parts) in q.iter(app.world()) {
        if parent.parent() == body {
            out.push((g.slot, m.clone(), parts.clone(), g.footwear.clone()));
        }
    }
    out.sort_by_key(|g| g.0);
    out
}

/// dress puts a Garment under the body for each thing worn, hides the
/// skin under it, tones the skin, and replaces them when the outfit
/// changes.
#[test]
fn dress_puts_garments_on_the_body() {
    let mut app = app();
    let mut outfit = Outfit::default();
    outfit.put(Slot::Top, 0);
    outfit.put(Slot::Bottom, 0);
    outfit.put(Slot::Shoes, 0);
    let (root, body) = spawn_dressed(&mut app, outfit);
    tick(&mut app, 1);
    let parts = app.world().get::<ModelParts>(body).unwrap().clone();
    assert_eq!(parts.hidden, [0, 1, 3].into_iter().collect());
    assert_eq!(parts.texture[&2], "skins/fair.jpg");
    assert_eq!(app.world().get::<ModelPath>(body).unwrap().0, "man.glb");
    let worn = garments(&mut app, body);
    assert_eq!(worn.len(), 3);
    assert_eq!(worn[0].1.0, "man/top/polo.glb");
    assert_eq!(
        worn[0].2.texture[&1], "skins/fair.jpg",
        "the top's skin patch in the body's tone"
    );
    assert_eq!(
        worn[1].3.as_deref(),
        Some("man/shoes/boots.glb"),
        "trousers know the shoes under them"
    );
    assert_eq!(worn[2].3, None);

    // Taking the top off replaces the garments.
    app.world_mut()
        .get_mut::<Outfit>(root)
        .unwrap()
        .remove(Slot::Top);
    tick(&mut app, 2);
    let worn = garments(&mut app, body);
    assert_eq!(worn.len(), 2);
    assert!(worn.iter().all(|g| g.0 != Slot::Top));
    let parts = app.world().get::<ModelParts>(body).unwrap().clone();
    assert_eq!(parts.hidden, [1, 3].into_iter().collect());
}

/// Garments mirror the body's player, paused.
#[test]
fn garments_mirror_the_body_pose() {
    let mut app = app();
    let mut outfit = Outfit::default();
    outfit.put(Slot::Top, 0);
    let (_, body) = spawn_dressed(&mut app, outfit);
    app.insert_resource(Libraries {
        by_model: HashMap::from([(
            "man.glb".to_string(),
            ClipLibrary::keyframed([("idle".to_string(), 61)]),
        )]),
    });
    tick(&mut app, 3);
    let bp = app.world().get::<AnimationPlayer>(body).unwrap().clone();
    let mut q = app
        .world_mut()
        .query_filtered::<&AnimationPlayer, With<Garment>>();
    let gp = q.single(app.world()).unwrap().clone();
    assert_eq!(gp.clip(), "idle");
    assert!(gp.paused);
    assert!((gp.time() - bp.time()).abs() < 1e-6 && bp.time() > 0.0);
}

/// The clip clock's pose reaches Bevy's player as weighted, seeked nodes.
#[test]
fn clip_clock_drives_bevy_player() {
    let mut app = app();
    let (walk, jump) = {
        let mut clips = app.world_mut().resource_mut::<Assets<AnimationClip>>();
        let mut walk = AnimationClip::default();
        walk.set_duration(1.0);
        let mut jump = AnimationClip::default();
        jump.set_duration(0.5);
        (clips.add(walk), clips.add(jump))
    };
    app.insert_resource(Libraries {
        by_model: HashMap::from([(
            "m".to_string(),
            ClipLibrary::new([("Walk".to_string(), 1.0), ("Jump".to_string(), 0.5)]),
        )]),
    });
    let world = app.world_mut();
    world.resource_scope(|world, mut graphs: Mut<Graphs>| {
        let mut assets = world.resource_mut::<Assets<AnimationGraph>>();
        graphs.build(
            "m",
            [("Walk".to_string(), walk), ("Jump".to_string(), jump)],
            &mut assets,
        );
    });
    let armature = app.world_mut().spawn(BevyPlayer::default()).id();
    let mut clock = AnimationPlayer::new("m");
    clock.play("Walk");
    let e = app.world_mut().spawn((clock, Animated(armature))).id();
    tick(&mut app, 30);
    app.world_mut()
        .get_mut::<AnimationPlayer>(e)
        .unwrap()
        .play_once("Jump")
        .fade_in(0.5);
    tick(&mut app, 15); // a quarter of a second into the half-second fade
    let graph = app.world().resource::<Graphs>().get("m").unwrap().clone();
    assert!(app.world().get::<AnimationGraphHandle>(armature).is_some());
    let player = app.world().get::<BevyPlayer>(armature).unwrap();
    let walk = player
        .animation(graph.nodes["Walk"])
        .expect("Walk still fading out");
    let jump = player.animation(graph.nodes["Jump"]).expect("Jump playing");
    assert!(
        (walk.weight() - 0.5).abs() < 0.05,
        "walk weight {}",
        walk.weight()
    );
    assert!(
        (jump.weight() - 0.5).abs() < 0.05,
        "jump weight {}",
        jump.weight()
    );
    assert!(
        (jump.seek_time() - 0.25).abs() < 0.02,
        "jump at {}",
        jump.seek_time()
    );
    assert!(
        (walk.seek_time() - 0.75).abs() < 0.02,
        "walk at {}",
        walk.seek_time()
    );
    assert!(
        walk.is_paused() && jump.is_paused(),
        "Bevy's clocks are the clip clock's"
    );
    tick(&mut app, 30);
    let player = app.world().get::<BevyPlayer>(armature).unwrap();
    assert!(
        player.animation(graph.nodes["Walk"]).is_none(),
        "the fade is over"
    );
}

/// strip is a hanging ribbon skinned to one bone.
fn strip(rows: usize, x: f32, y: f32) -> SkinnedMeshData {
    let mut m = SkinnedMeshData::default();
    for r in 0..=rows {
        let yy = y - r as f32 * 0.1;
        m.positions.push(Vec3::new(x, yy, 0.0));
        m.positions.push(Vec3::new(x + 0.1, yy, 0.0));
        m.normals.push(Vec3::Z);
        m.normals.push(Vec3::Z);
    }
    for r in 0..rows {
        let a = 2 * r as u32;
        m.indices.extend([a, a + 2, a + 1, a + 1, a + 2, a + 3]);
    }
    m.joints = vec![[0; 4]; m.positions.len()];
    m.weights = vec![[1.0, 0.0, 0.0, 0.0]; m.positions.len()];
    m
}

fn one_bone() -> Skeleton {
    Skeleton {
        names: vec!["pelvis".into()],
        parents: vec![-1],
        bind: vec![Transform::IDENTITY],
    }
}

/// clothe fits a Cloth to a garment hanging off the body, and leaves a
/// Rigid character's clothes to the skeleton.
#[test]
fn clothe_fits_hanging_garments() {
    let mut app = app();
    let mut store = ModelStore::default();
    // The body: a short strip the garment's top row touches.
    store.insert(
        "body.glb",
        ModelMeshes {
            meshes: vec![strip(1, 0.0, 1.0)],
            skeleton: one_bone(),
        },
    );
    // The garment hangs half a metre below the body's bottom edge.
    store.insert(
        "top.glb",
        ModelMeshes {
            meshes: vec![strip(5, 0.0, 0.9)],
            skeleton: one_bone(),
        },
    );
    app.insert_resource(store);
    let spawn = |app: &mut App, rigid: bool| {
        let mut root = app.world_mut().spawn(Transform::IDENTITY);
        if rigid {
            root.insert(Rigid);
        }
        let root = root.id();
        let body = app
            .world_mut()
            .spawn((ModelPath("body.glb".into()), ChildOf(root)))
            .id();
        app.world_mut()
            .spawn((
                Garment {
                    slot: Slot::Top,
                    skin: vec![],
                    footwear: None,
                },
                ModelPath("top.glb".into()),
                ChildOf(body),
            ))
            .id()
    };
    let loose = spawn(&mut app, false);
    let still = spawn(&mut app, true);
    tick(&mut app, 2);
    let cloth = app
        .world()
        .get::<Cloth>(loose)
        .expect("a loose garment gets a Cloth");
    assert!(app.world().get::<Clothed>(loose).is_some());
    let spec = &cloth.meshes[&0];
    assert_eq!(spec.freedom[0], 0.0, "the top row touches the body");
    assert!(spec.freedom[11] > 0.0, "the hem hangs free");
    assert!((cloth.stiffness - 0.045).abs() < 1e-6);
    assert!(
        app.world().get::<Cloth>(still).is_none() && app.world().get::<Clothed>(still).is_some()
    );
}

/// Distant bodies are posed at one of a few moments of the clock, with no
/// crossfade.
#[test]
fn distant_bodies_keep_lockstep() {
    let mut app = app();
    app.insert_resource(Libraries {
        by_model: HashMap::from([(
            "m".to_string(),
            ClipLibrary::keyframed([("idle".to_string(), 61)]),
        )]),
    });
    let mut p = AnimationPlayer::new("m");
    p.play("idle");
    let e = app
        .world_mut()
        .spawn((Body, State::default(), p, Distant))
        .id();
    tick(&mut app, 5);
    let p = app.world().get::<AnimationPlayer>(e).unwrap();
    let elapsed = app.world().resource::<Time>().elapsed_secs();
    let expect = earth_two_client::presentation::distant::lockstep_time(elapsed, e);
    assert!(
        (p.time() - expect % 1.0).abs() < 1e-3,
        "time {} want {}",
        p.time(),
        expect % 1.0
    );
}

#[test]
fn pausing_holds_body_and_garment_clip_time_blend_and_state_then_resumes() {
    for mode in [Anim::Fix, Anim::Walk, Anim::Interact] {
        let mut a = app();
        a.insert_resource(Libraries {
            by_model: HashMap::from([(
                "m".into(),
                ClipLibrary::keyframed([("idle".into(), 61), ("pose".into(), 121)]),
            )]),
        });
        a.insert_resource(Roster {
            skins: vec![Skin::new(
                "m",
                HashMap::from([
                    (Anim::Idle, Clip::named("idle")),
                    (mode, Clip::named("pose")),
                ]),
                1.,
            )],
        });
        let root = a
            .world_mut()
            .spawn((
                Character::default(),
                Intent::default(),
                CharacterController {
                    grounded: true,
                    ..default()
                },
                Traversal::default(),
                MotionSamples::default(),
                Transform::default(),
            ))
            .id();
        let mut p = AnimationPlayer::new("m");
        p.play("idle");
        p.play("pose").fade_in(0.3);
        p.seek(0.4);
        let body = a
            .world_mut()
            .spawn((
                Body,
                State {
                    current: mode,
                    ..default()
                },
                p,
                Transform::default(),
                ChildOf(root),
            ))
            .id();
        let garment = a
            .world_mut()
            .spawn((
                Garment {
                    slot: Slot::Top,
                    skin: vec![],
                    footwear: None,
                },
                AnimationPlayer::new("m"),
                ChildOf(body),
            ))
            .id();
        a.world_mut().resource_mut::<Controls>().enabled = false;
        let mut before = a.world().get::<AnimationPlayer>(body).unwrap().clone();
        before.paused = true;
        tick(&mut a, 60);
        assert_eq!(a.world().get::<State>(body).unwrap().current, mode);
        assert_eq!(
            a.world().get::<AnimationPlayer>(body).unwrap().clone(),
            before
        );
        assert_eq!(
            a.world().get::<AnimationPlayer>(garment).unwrap().clone(),
            before
        );
        a.world_mut().get_mut::<Intent>(root).unwrap().hold = mode;
        a.world_mut().resource_mut::<Controls>().enabled = true;
        tick(&mut a, 2);
        assert!(!a.world().get::<AnimationPlayer>(body).unwrap().paused);
        assert_ne!(
            a.world().get::<AnimationPlayer>(body).unwrap().clone(),
            before
        );
    }
}
