//! The Hull test block, placed from the real manifests through BSN, without
//! a window: the hierarchy lands each collider where the Go client's flat
//! `Kit.Place` puts it, Avian collides with it, and despawning the layout
//! root removes everything.

use avian3d::prelude::*;
use bevy::{asset::AssetPlugin, prelude::*};
use earth_two_client::{
    physics::{headless_app, source_assets},
    world::{Ladder, Lamp, LayoutRoot, PieceCollider, PieceModel, Placed, SpawnLayout},
};
use earth_two_world::kit::{Kit, Layout};

const LAYOUT: &str = "world/hull_block.json";

fn block_app() -> App {
    let mut app = headless_app(AssetPlugin {
        file_path: source_assets(),
        ..default()
    });
    let assets = app.world().resource::<AssetServer>().clone();
    app.world_mut()
        .spawn(SpawnLayout::load(&assets, "world/world.json", LAYOUT));
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
    app
}

/// Quaternions equal to single precision, either sign; `angle_between`
/// loses that through `acos` near 1.
fn same_rotation(a: Quat, b: Quat) -> bool {
    a.dot(b).abs() > 1.0 - 1e-6
}

fn reference() -> (Kit, Layout) {
    let root = source_assets();
    (
        Kit::load(&root, "world/world.json").unwrap(),
        Layout::load(&root, LAYOUT).unwrap(),
    )
}

#[test]
fn layout_places_every_piece_with_its_parts_where_the_manifest_says() {
    let mut app = block_app();
    let (kit, layout) = reference();
    assert!(!layout.pieces.is_empty());

    let mut roots: Vec<(Entity, Placed, Transform, Vec<Entity>)> = app
        .world_mut()
        .query::<(Entity, &Placed, &Transform, Option<&Children>)>()
        .iter(app.world())
        .map(|(e, p, t, c)| {
            (
                e,
                p.clone(),
                *t,
                c.map(|c| c.iter().collect()).unwrap_or_default(),
            )
        })
        .collect();
    roots.sort_by_key(|(_, placed, _, _)| placed.index);
    assert_eq!(roots.len(), layout.pieces.len(), "one root per placement");

    let mut colliders = 0;
    let mut ladders = 0;
    let mut lamps = 0;
    for (entity, placed, transform, children) in &roots {
        let placement = &layout.pieces[placed.index];
        assert_eq!(placed.piece, placement.piece);
        let piece = kit.piece(&placed.piece).unwrap();
        assert_eq!(
            app.world().get::<PieceModel>(*entity).unwrap().0,
            piece.model,
            "{}: the model path is the manifest's",
            placed.piece
        );
        let at = placement.at();
        let turn = placement.turn();
        // Avian writes the body's transform back from its own pose.
        assert!(
            transform.translation.distance(at) < 1e-4,
            "{}: at {}",
            placed.piece,
            transform.translation
        );
        assert!(
            same_rotation(transform.rotation, turn),
            "{}: turned {:?}, want {turn:?}",
            placed.piece,
            transform.rotation
        );
        assert_eq!(
            app.world().get::<RigidBody>(*entity).is_some(),
            !piece.colliders.is_empty(),
            "{}: a body only when there is something to collide with",
            placed.piece
        );

        let mut spawned_colliders = Vec::new();
        for child in children {
            let world = app.world();
            if world.get::<PieceCollider>(*child).is_some() {
                spawned_colliders.push(*child);
            }
            if let Some(ladder) = world.get::<Ladder>(*child) {
                ladders += 1;
                let want = piece.ladders[0].in_frame(at, turn);
                assert!(
                    ladder.bottom.distance(want.bottom) < 1e-5
                        && ladder.top.distance(want.top) < 1e-5
                );
                assert!(ladder.facing.distance(want.facing) < 1e-5);
                assert_eq!(ladder.width, want.width);
            }
            if world.get::<Lamp>(*child).is_some() {
                lamps += 1;
            }
        }
        assert_eq!(
            spawned_colliders.len(),
            piece.colliders.len(),
            "{}: colliders",
            placed.piece
        );
        colliders += spawned_colliders.len();
        for (child, collider) in spawned_colliders.iter().zip(&piece.colliders) {
            // The hierarchy must land each box where the Go client's flat
            // placement puts it.
            let (center, rotation) = collider.in_frame(at, turn);
            let global = app.world().get::<GlobalTransform>(*child).unwrap();
            let (_, got_rotation, got_center) = global.to_scale_rotation_translation();
            assert!(
                got_center.distance(center) < 1e-4,
                "{}: collider at {got_center}, want {center}",
                placed.piece
            );
            assert!(
                same_rotation(got_rotation, rotation),
                "{}: collider rotation",
                placed.piece
            );
            let shape = app.world().get::<Collider>(*child).unwrap();
            let aabb = shape.aabb(Vec3::ZERO, Quat::IDENTITY, 0.0);
            let size = aabb.max - aabb.min;
            assert!(
                size.distance(Vec3::from(collider.size)) < 1e-4,
                "{}: collider size",
                placed.piece
            );
        }
    }
    assert!(
        colliders > 0 && ladders > 0,
        "{colliders} colliders, {ladders} ladders, {lamps} lamps"
    );
    let expected_ladders: usize = layout
        .pieces
        .iter()
        .map(|p| kit.piece(&p.piece).unwrap().ladders.len())
        .sum();
    assert_eq!(ladders, expected_ladders);
}

#[derive(Resource, Default)]
struct Hit(Option<(Entity, f32)>);

#[derive(Resource)]
struct CastFrom(Vec3);

fn cast_down(query: SpatialQuery, from: Res<CastFrom>, mut hit: ResMut<Hit>) {
    hit.0 = query
        .cast_ray(
            from.0,
            Dir3::NEG_Y,
            100.0,
            true,
            &SpatialQueryFilter::default(),
        )
        .map(|h| (h.entity, h.distance));
}

#[test]
fn the_catwalk_collides_at_the_height_traversal_expects() {
    let mut app = block_app();
    let (kit, layout) = reference();
    let catwalk = layout
        .pieces
        .iter()
        .find(|p| p.piece == "catwalk")
        .expect("the Hull block has a catwalk");
    let top = kit.piece("catwalk").unwrap().colliders[0].bounds().1.y;
    let from = catwalk.at() + Vec3::new(0.0, 10.0, 0.0);
    app.insert_resource(CastFrom(from))
        .init_resource::<Hit>()
        .add_systems(Update, cast_down);
    for _ in 0..3 {
        app.update();
    }
    let (entity, distance) = app
        .world()
        .resource::<Hit>()
        .0
        .expect("the ray hits the catwalk");
    assert!(app.world().entity(entity).contains::<PieceCollider>());
    let want = from.y - (catwalk.at().y + top);
    assert!(
        (distance - want).abs() < 1e-3,
        "hit {distance} m down, want {want}"
    );
}

#[test]
fn a_dropped_body_rests_on_a_crate() {
    let mut app = block_app();
    let (kit, _) = reference();
    let top = kit.piece("crate").unwrap().colliders[0].bounds().1.y;
    let crate_at = app
        .world_mut()
        .query::<(&Placed, &Transform)>()
        .iter(app.world())
        .find(|(p, _)| p.piece == "crate")
        .map(|(_, t)| t.translation)
        .expect("the block has a crate");
    let probe = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::sphere(0.2),
            Transform::from_translation(crate_at + Vec3::new(0.0, top + 2.0, 0.0)),
        ))
        .id();
    for _ in 0..240 {
        app.update();
    }
    let y = app.world().get::<Position>(probe).unwrap().0.y;
    // The probe's centre rests a radius above the crate's collider, which
    // the hierarchy placed under it.
    let want = crate_at.y + top + 0.2;
    assert!((y - want).abs() < 0.03, "probe rests at {y}, want {want}");
}

#[test]
fn despawning_the_layout_root_removes_every_piece() {
    let mut app = block_app();
    let root = app
        .world_mut()
        .query_filtered::<Entity, With<LayoutRoot>>()
        .single(app.world())
        .unwrap();
    assert!(app.world_mut().query::<&Placed>().iter(app.world()).count() > 0);
    app.world_mut().entity_mut(root).despawn();
    app.update();
    assert_eq!(
        app.world_mut().query::<&Placed>().iter(app.world()).count(),
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&PieceCollider>()
            .iter(app.world())
            .count(),
        0
    );
    assert_eq!(
        app.world_mut().query::<&Ladder>().iter(app.world()).count(),
        0
    );
}
