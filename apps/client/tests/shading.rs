//! The shading package's tests, from the Go `shading/*_test.go`.

use bevy::prelude::*;
use earth_two_client::{
    shading::{
        BeamEye, Beams, LAMP_INTENSITY_SCALE, Look, MAX_SPOTS, ShadingPlugin, SpotLight, Zone,
        lamp_light,
    },
    world::Lamp,
};

/// `TestFixedLampsIncludeMoreThanCameraBudget`: every fixed lamp, however
/// far off, becomes a light with the game's colour, intensity and range;
/// none is dropped for a camera budget.
#[test]
fn fixed_lamps_include_more_than_camera_budget() {
    let lamps: Vec<(Vec3, Lamp)> = (0..64)
        .map(|i| {
            (
                Vec3::new(i as f32 * 300.0, 4.0, 0.0),
                Lamp {
                    color: [1.0, 1.0, 1.0],
                    intensity: 2.0,
                    range: 12.0,
                },
            )
        })
        .collect();
    let lights: Vec<_> = lamps.iter().filter_map(|(_, l)| lamp_light(l)).collect();
    assert_eq!(lights.len(), 64, "all fixed lamps must reach the shader");
    let far = lights[63];
    assert_eq!(lamps[63].0.x, 18900.0);
    assert_eq!(far.range, 12.0);
    assert_eq!(far.color, Vec3::splat(2.0), "colour times intensity");
    // render.PointLight's default intensity is 1.
    let unlit = lamp_light(&Lamp {
        color: [0.5, 0.25, 1.0],
        intensity: 0.0,
        range: 3.0,
    })
    .expect("a lamp with reach");
    assert_eq!(unlit.color, Vec3::new(0.5, 0.25, 1.0));
    // The scale Bevy divides back out, so the shader sees the game's
    // numbers.
    assert!((LAMP_INTENSITY_SCALE / (4.0 * std::f32::consts::PI) - 1.0).abs() < 1e-6);
}

/// `TestLampGroupsKeepEveryLightAndItsWholeReach`, as far as it applies:
/// there's no grouping to lose a lamp in, but a lamp with no reach gives
/// no light, and every other one gives exactly its own.
#[test]
fn lamps_keep_every_light() {
    let lamps = [
        Lamp {
            color: [1.0, 0.0, 0.0],
            intensity: 1.0,
            range: 18.0,
        },
        Lamp {
            color: [0.0, 1.0, 0.0],
            intensity: 1.0,
            range: 0.0,
        },
        Lamp {
            color: [0.0, 0.0, 1.0],
            intensity: 3.0,
            range: 8.0,
        },
    ];
    let lights: Vec<_> = lamps.iter().filter_map(lamp_light).collect();
    assert_eq!(lights.len(), 2, "empty lamps give no light");
    assert_eq!(lights[0].range, 18.0);
    assert_eq!(lights[1].color, Vec3::new(0.0, 0.0, 3.0));
}

#[cfg(feature = "viewer")]
#[test]
fn lamp_point_light_round_trips_the_game_scale() {
    use earth_two_client::shading::lamp_point_light;
    let light = lamp_point_light(&Lamp {
        color: [1.0, 0.5, 0.25],
        intensity: 2.0,
        range: 12.0,
    })
    .expect("a light");
    // Bevy stores colour * intensity / 4π: the game's colour * intensity.
    let c = light.color.to_linear();
    let stored = Vec3::new(c.red, c.green, c.blue) * light.intensity / (4.0 * std::f32::consts::PI);
    assert!(
        (stored - Vec3::new(2.0, 1.0, 0.5)).length() < 1e-4,
        "{stored}"
    );
    assert_eq!(light.range, 12.0);
    assert!(!light.shadow_maps_enabled);
}

fn spot_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, ShadingPlugin::default()));
    app
}

/// `TestSpotlightsFollowTransformsAndClearWhenOff`.
#[test]
fn spotlights_follow_transforms_and_clear_when_off() {
    let mut app = spot_app();
    let world = app.world_mut();
    world.spawn((BeamEye { order: 0 }, Transform::from_xyz(1000.0, 0.0, 0.0)));
    world.spawn((BeamEye { order: 1 }, Transform::IDENTITY));
    for i in (1..=12).rev() {
        world
            .spawn(
                Transform::from_xyz(i as f32, 0.0, 0.0)
                    .with_rotation(Quat::from_axis_angle(Vec3::Y, 90f32.to_radians())),
            )
            .with_child((
                SpotLight {
                    color: [255, 255, 255],
                    intensity: 2.0,
                    range: 50.0,
                    inner: 16.0,
                    outer: 28.0,
                    enabled: true,
                    priority: 0,
                },
                Transform::from_xyz(0.0, 1.0, -1.0),
            ));
    }
    // The closest light is off, and must not consume a slot.
    world.spawn((
        SpotLight {
            range: 50.0,
            intensity: 2.0,
            ..default()
        },
        Transform::IDENTITY,
    ));
    app.update();
    let beams = app.world().resource::<Beams>().0.clone();
    assert_eq!(beams.len(), MAX_SPOTS, "{} spotlights", beams.len());
    // Local -Z becomes world -X; the nearest parent at X=1 has a lens at X=0.
    let (p, d) = (beams[0].position, beams[0].direction);
    assert!(
        p.x.abs() <= 0.001 && p.y == 1.0 && p.z.abs() <= 0.001 && d.x <= -0.99,
        "light did not follow its parent's pose: position {p}, direction {d}"
    );
    assert!(
        beams[7].position.x <= 7.01,
        "distant lights displaced nearer lights"
    );
    assert!(
        beams[0].cos_outer < beams[0].cos_inner,
        "reversed cone edges: {:?}",
        beams[0]
    );
    // The driven vehicle's lamps remain selected even among nearer cars.
    let mut lights = app
        .world_mut()
        .query::<(&mut SpotLight, &GlobalTransform)>();
    for (mut l, tr) in lights.iter_mut(app.world_mut()) {
        if tr.translation().x > 10.0 {
            l.priority = 1;
        }
    }
    app.update();
    assert!(
        app.world().resource::<Beams>().0[0].position.x >= 10.0,
        "nearby parked cars displaced a priority beam"
    );
    let mut lights = app.world_mut().query::<&mut SpotLight>();
    for mut l in lights.iter_mut(app.world_mut()) {
        l.enabled = false;
    }
    app.update();
    assert!(
        app.world().resource::<Beams>().0.is_empty(),
        "disabled lights left stale beams in the shader"
    );
}

/// A zone's weight is the shader's blend: 1 well inside, 0 outside, and
/// half way through its blend band.
#[test]
fn zone_weight_is_the_shaders_blend() {
    let z = Zone {
        min: Vec3::new(-40.0, -5.0, -12.0),
        max: Vec3::new(8.0, 11.0, 20.0),
        blend: 2.0,
        ambient: [255, 186, 130],
        brightness: 0.8,
        sun: [255, 240, 220],
    };
    assert_eq!(z.weight(Vec3::new(-10.0, 2.0, 4.0)), 1.0);
    assert_eq!(z.weight(Vec3::new(30.0, 2.0, 4.0)), 0.0);
    assert!((z.weight(Vec3::new(7.0, 2.0, 4.0)) - 0.5).abs() < 1e-6);
    let look = Look::earth_two();
    assert_eq!(look.zones.len(), 2);
    assert_eq!(look.fog_end, 19100.0);
}
