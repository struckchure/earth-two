use avian3d::prelude::*;
use bevy::prelude::*;
use earth_two_client::{
    physics::{probe_position, run_smoke, set_paused, smoke_app},
    scenes::Ground,
};

#[test]
fn bsn_body_falls_and_settles_on_bsn_floor() {
    let mut app = smoke_app();
    app.update();
    let start = probe_position(&mut app).unwrap();
    for _ in 0..15 {
        app.update();
    }
    let falling = probe_position(&mut app).unwrap();
    assert!(falling.y < start.y - 0.1, "gravity must move the probe");
    assert!(falling.y > 0.5, "probe should still be falling");
    run_smoke().unwrap();
}

#[test]
fn physics_pause_freezes_motion_and_resume_continues_it() {
    let mut app = smoke_app();
    for _ in 0..10 {
        app.update();
    }
    set_paused(&mut app.world_mut().resource_mut::<Time<Physics>>(), true);
    let before = probe_position(&mut app).unwrap();
    for _ in 0..30 {
        app.update();
    }
    assert_eq!(probe_position(&mut app).unwrap(), before);
    set_paused(&mut app.world_mut().resource_mut::<Time<Physics>>(), false);
    for _ in 0..10 {
        app.update();
    }
    assert!(probe_position(&mut app).unwrap().y < before.y);
}

#[derive(Resource, Default)]
struct GroundHit(Option<(Entity, f32)>);

fn cast_to_ground(query: SpatialQuery, mut result: ResMut<GroundHit>) {
    result.0 = query
        .cast_ray(
            Vec3::new(3.0, 5.0, 0.0),
            Dir3::NEG_Y,
            10.0,
            true,
            &SpatialQueryFilter::default(),
        )
        .map(|hit| (hit.entity, hit.distance));
}

#[test]
fn spatial_query_hits_the_bsn_floor_at_the_expected_height() {
    let mut app = smoke_app();
    app.init_resource::<GroundHit>()
        .add_systems(Update, cast_to_ground);
    for _ in 0..3 {
        app.update();
    }
    let (entity, distance) = app.world().resource::<GroundHit>().0.unwrap();
    assert!(app.world().entity(entity).contains::<Ground>());
    assert!((distance - 5.0).abs() < 0.001);
}
