//! `world/block_test.go`: the Hull test block's colliders and ladders,
//! placed from the real manifests, with a character in it, run through
//! physics and traversal without drawing anything.

use avian3d::prelude::*;
use bevy::{asset::AssetPlugin, prelude::*, time::TimeUpdateStrategy};
use earth_two_client::{
    character::{
        Anim, Body, Character, CharacterController, CharacterPlugin, Intent, State, Traversal,
        TraversalConfig,
    },
    physics::{EarthPhysicsPlugin, FIXED_HZ, source_assets},
    world::{LayoutRoot, SpawnLayout, WorldPlugin},
};
use std::time::Duration;

struct Block {
    app: App,
    root: Entity,
    body: Entity,
}

impl Block {
    fn new(feet: Vec3, facing: Vec3) -> Block {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin {
                file_path: source_assets(),
                ..default()
            },
            bevy::scene::ScenePlugin,
            EarthPhysicsPlugin,
            WorldPlugin,
            CharacterPlugin,
        ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / FIXED_HZ,
        )));
        app.finish();
        app.cleanup();
        let assets = app.world().resource::<AssetServer>().clone();
        let world = app.world_mut();
        // The ground under the deck, as the game has it.
        world.spawn((
            RigidBody::Static,
            Collider::cuboid(40.0, 1.0, 40.0),
            Transform::from_xyz(0.0, -0.5, 0.0),
        ));
        world.spawn(SpawnLayout::load(
            &assets,
            "world/world.json",
            "world/hull_block.json",
        ));
        // The files load on the IO pool; wait for the layout root, then let
        // the scene, transforms and physics settle.
        for _ in 0..1200 {
            app.update();
            if app
                .world_mut()
                .query::<&LayoutRoot>()
                .iter(app.world())
                .next()
                .is_some()
            {
                break;
            }
        }
        for _ in 0..10 {
            app.update();
        }
        let mut body = Entity::PLACEHOLDER;
        let root = app
            .world_mut()
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
        let mut b = Block { app, root, body };
        b.tick(2);
        b.tick(15);
        b.face(facing);
        b
    }

    fn tick(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
        }
    }

    fn face(&mut self, dir: Vec3) {
        self.app
            .world_mut()
            .get_mut::<Transform>(self.body)
            .unwrap()
            .rotation = Quat::from_axis_angle(Vec3::Y, dir.x.atan2(dir.z));
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

    fn mode(&mut self) -> Anim {
        self.s().mode
    }

    fn feet(&mut self) -> Vec3 {
        let height = self.cc().height;
        self.app
            .world()
            .get::<Transform>(self.root)
            .unwrap()
            .translation
            - Vec3::Y * (height / 2.0)
    }

    /// Ticks until `done` says so, for at most `n` ticks, and says whether
    /// it did.
    fn until(&mut self, n: usize, mut done: impl FnMut(&mut Block) -> bool) -> bool {
        for _ in 0..n {
            self.tick(1);
            if done(self) {
                return true;
            }
        }
        false
    }
}

// The positions below are the layout's (tools/world/hull_block.py) in the
// game's frame: Blender's (x, y) is the game's (x, -z).

#[test]
fn block_vaults_the_crate() {
    // The crate at (1, 1), its long side to the south.
    let mut b = Block::new(Vec3::new(1.0, 0.0, 1.95), Vec3::NEG_Z);
    b.intent().move_dir = Vec3::NEG_Z;
    b.intent().jump = true;
    // It turns to face the crate first, if it has to.
    assert!(
        b.until(45, |b| b.mode() == Anim::Vault),
        "mode {}, want a vault (hint {:?})",
        b.mode(),
        b.s().hint
    );
    b.intent().move_dir = Vec3::ZERO;
    b.tick(90);
    let f = b.feet();
    assert!(f.z <= 0.5 && f.y <= 0.1, "ended at {f}, not over the crate");
}

#[test]
fn block_mantles_the_tall_crate() {
    // The tall crate at (5, 3), from the north.
    let mut b = Block::new(Vec3::new(5.0, 0.0, 2.05), Vec3::Z);
    b.intent().move_dir = Vec3::Z;
    b.intent().jump = true;
    // It turns to face the crate first, if it has to.
    assert!(
        b.until(45, |b| b.mode() == Anim::Mantle),
        "mode {}, want a mantle (hint {:?})",
        b.mode(),
        b.s().hint
    );
    b.intent().move_dir = Vec3::ZERO;
    b.tick(90);
    let f = b.feet();
    assert!(f.y >= 1.55, "ended at {f}, not on top of the crate");
}

#[test]
fn block_slides_under_the_duct() {
    // The duct across the gap at (-3, -1), from the south at a run.
    let mut b = Block::new(Vec3::new(-3.0, 0.0, 0.6), Vec3::NEG_Z);
    b.intent().move_dir = Vec3::NEG_Z;
    b.intent().run = true;
    b.cc().walk = Vec3::new(0.0, 0.0, -6.0);
    b.cc().velocity.z = -6.0;
    b.intent().slide = true;
    b.tick(1);
    assert_eq!(
        b.mode(),
        Anim::Slide,
        "want a slide (hint {:?})",
        b.s().hint
    );
    assert!(
        b.until(120, |b| b.feet().z < -1.6),
        "stuck at {} under the duct",
        b.feet()
    );
}

#[test]
fn block_climbs_the_ladder_to_the_catwalk() {
    // The ladder at (-5, -4.37), climbed facing north.
    let mut b = Block::new(Vec3::new(-5.0, 0.0, -3.4), Vec3::NEG_Z);
    b.intent().move_dir = Vec3::NEG_Z;
    assert!(
        b.until(60, |b| matches!(
            b.mode(),
            Anim::LadderEnter | Anim::LadderClimb
        )),
        "didn't get on the ladder: at {}, mode {}",
        b.feet(),
        b.mode()
    );
    let mut exited = false;
    b.until(1200, |b| {
        if b.mode() == Anim::LadderExit {
            exited = true;
        }
        exited && b.mode() != Anim::LadderExit
    });
    let f = b.feet();
    assert!(
        exited && f.y >= 3.5 && f.z <= -4.6,
        "ended at {f} (mode {}), not on the catwalk",
        b.mode()
    );
}

#[test]
fn block_walks_up_the_stairs() {
    // The stairs at x 7, rising north from z 1.35 to the catwalk.
    let mut b = Block::new(Vec3::new(7.0, 0.0, 2.2), Vec3::NEG_Z);
    b.intent().move_dir = Vec3::NEG_Z;
    assert!(
        b.until(600, |b| {
            let f = b.feet();
            f.y > 3.5 && f.z < -4.8
        }),
        "got to {}, not up on the catwalk",
        b.feet()
    );
}

#[test]
fn block_wall_kicks_up_the_corridor() {
    // The corridor's walls face each other at x 8.15 and 9.85.
    // Dropped in a metre up, it's still in the air, as off a jump; level
    // with the seam between two walls, which mustn't let its probe through.
    let mut b = Block::new(Vec3::new(9.3, 1.0, 0.0), Vec3::X);
    b.intent().move_dir = Vec3::X;
    b.intent().jump = true;
    b.cc().walk = Vec3::new(4.6, 0.0, 0.0);
    b.tick(1);
    let first = b.s().last_wall;
    assert!(
        b.cc().walk.x < 0.0,
        "no kick off the east wall: {:?}",
        b.s().clone()
    );
    b.intent().move_dir = Vec3::NEG_X;
    b.tick(11);
    b.intent().jump = true;
    b.tick(1);
    assert!(
        b.s().last_wall != first && b.cc().walk.x > 0.0,
        "no second kick off the west wall: at {}, {:?}",
        b.feet(),
        b.s().clone()
    );
}
