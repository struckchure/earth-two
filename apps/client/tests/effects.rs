//! Go emission gates, cadence and billboard/lifecycle regressions.
use bevy::prelude::*;
use earth_two_client::{
    character::{Anim, CharacterController},
    game::{
        cues::{BodyMemory, CueRandom},
        effects,
    },
    landfall::surface::{Footing, Soundscape},
    sky::{
        Effects, Particle, Rgba,
        effects::{MAX_PARTICLES, lit},
    },
    vehicle::{Spec, VehicleState, WheelState, spec::WheelSpec},
};
fn sand() -> Vec3 {
    Vec3::new(6000., 0., 4000.)
}
#[test]
fn steps_raise_loose_dust_only_outside_the_dome() {
    for (under, at, want) in [
        (Footing::Sand, sand(), 4),
        (Footing::Soil, sand(), 4),
        (Footing::Grating, sand(), 0),
        (Footing::Rug, sand(), 0),
        (Footing::Soil, Vec3::new(20., 0., -30.), 0),
    ] {
        let mut fx = Effects::seeded(3);
        effects::foot(&mut fx, under, at, Vec3::Z * 4., 4.);
        assert_eq!(fx.ps.len(), want);
        assert!(
            fx.ps
                .iter()
                .all(|p| p.size0 == 0.1 && p.life >= 0.9 && p.life < 1.5)
        );
    }
}
#[test]
fn landing_and_trails_keep_original_thresholds_and_surfaces() {
    let mut m = BodyMemory::new(Anim::Fall, false);
    let mut cc = CharacterController {
        grounded: false,
        velocity: Vec3::NEG_Y * 8.5,
        ..default()
    };
    m.update(
        Anim::Fall,
        &cc,
        Footing::Sand,
        0.1,
        &mut CueRandom::default(),
    );
    cc.grounded = true;
    assert_eq!(m.landing_strength(&cc), Some(1.));
    let mut fx = Effects::seeded(1);
    effects::body(
        &mut fx,
        sand(),
        cc.velocity,
        Footing::Sand,
        true,
        Anim::Idle,
        m.landing_strength(&cc),
        0.1,
    );
    assert_eq!(fx.ps.len(), 24);
    assert!((fx.ps[0].size1 - 1.3).abs() < 0.00001);
    let mut hard = Effects::seeded(1);
    effects::body(
        &mut hard,
        sand(),
        cc.velocity,
        Footing::Paving,
        true,
        Anim::Roll,
        Some(1.),
        1.,
    );
    assert!(hard.ps.is_empty());
    effects::body(
        &mut hard,
        sand(),
        Vec3::Z * 4.,
        Footing::Soil,
        true,
        Anim::Roll,
        None,
        1.,
    );
    assert_eq!(hard.ps.len(), 28);
    effects::body(
        &mut hard,
        sand(),
        Vec3::Z * 4.,
        Footing::Soil,
        false,
        Anim::Slide,
        None,
        1.,
    );
    assert_eq!(hard.ps.len(), 28);
    cc.controlled = true;
    assert_eq!(m.landing_strength(&cc), None);
}
fn wheel_fixture() -> (Spec, VehicleState) {
    (
        Spec {
            wheels: vec![WheelSpec {
                radius: 0.5,
                ..default()
            }],
            ..default()
        },
        VehicleState {
            speed: 10.,
            wheels: vec![WheelState {
                contact: true,
                ..default()
            }],
            ..default()
        },
    )
}
#[test]
fn wheel_emission_is_independent_of_frame_rate_and_requires_contact_and_loose_ground() {
    let (spec, mut st) = wheel_fixture();
    let scape = Soundscape::default();
    for hz in [30, 60, 120] {
        let mut fx = Effects::seeded(8);
        let mut dust = vec![];
        for _ in 0..hz {
            effects::wheels(
                &mut fx,
                &mut dust,
                &spec,
                &st,
                &Transform::from_translation(sand()),
                Vec3::Z * 10.,
                &scape,
                1. / hz as f32,
            );
        }
        assert!((17..=18).contains(&fx.ps.len()), "{hz}: {}", fx.ps.len());
        assert!((fx.ps.len() as f32 + dust[0] - 18.).abs() < 0.00002);
    }
    let mut fx = Effects::seeded(8);
    let mut dust = vec![];
    st.wheels[0].contact = false;
    effects::wheels(
        &mut fx,
        &mut dust,
        &spec,
        &st,
        &Transform::from_translation(sand()),
        Vec3::ZERO,
        &scape,
        1.,
    );
    assert!(fx.ps.is_empty());
    st.wheels[0].contact = true;
    st.speed = 0.;
    st.wheels[0].spin = 20.;
    effects::wheels(
        &mut fx,
        &mut dust,
        &spec,
        &st,
        &Transform::from_translation(sand()),
        Vec3::ZERO,
        &scape,
        1.,
    );
    assert_eq!(
        fx.ps.len(),
        18,
        "spinning a stationary wheel still raises dust"
    );
    effects::wheels(
        &mut fx,
        &mut dust,
        &spec,
        &st,
        &Transform::from_xyz(-16., 0., 4.),
        Vec3::ZERO,
        &scape,
        1.,
    );
    assert_eq!(fx.ps.len(), 18, "Hull deck is hard");
}
#[test]
fn accepted_crashes_emit_ground_rings_or_fast_metal_sparks() {
    let mut fx = Effects::seeded(2);
    effects::crash(&mut fx, sand(), Vec3::X, 7., false);
    assert!(fx.ps.is_empty());
    effects::crash(&mut fx, sand(), Vec3::X, 14.5, false);
    assert_eq!(fx.ps.len(), 36);
    assert!(
        fx.ps
            .iter()
            .all(|p| p.fall == 9.8 && p.size0 == 0.05 && p.life >= 0.25 && p.life < 0.6)
    );
    effects::crash(&mut fx, sand(), Vec3::X, 14.5, true);
    assert_eq!(fx.ps.len(), 66);
    assert!(
        fx.ps[36..]
            .iter()
            .all(|p| p.size0 == 0.15 && p.fall == -0.05)
    );
}
#[test]
fn billboard_sort_floor_fade_and_empty_frame_clear_old_geometry() {
    let mut fx = Effects::seeded(3);
    for (z, age) in [(8., 0.25), (20., 0.25), (-5., 0.25), (1., 0.25)] {
        fx.add(Particle {
            pos: Vec3::new(0., 0., z),
            age,
            life: 1.,
            size0: 0.5,
            size1: 0.5,
            colour: Rgba {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            ..default()
        });
    }
    let mut verts = vec![[0.; 3]; 4 * MAX_PARTICLES];
    let mut cols = vec![Rgba::default(); 4 * MAX_PARTICLES];
    assert_eq!(
        fx.quads(
            Vec3::ZERO,
            Vec3::Z,
            Vec3::NEG_X,
            Vec3::Y,
            &mut verts,
            &mut cols
        ),
        3
    );
    assert_eq!(verts[0][2], 20.);
    assert_eq!(verts[4][2], 8.);
    assert_eq!(cols[8].a, 0);
    assert!(
        (verts[0][1] + 0.05).abs() < 0.00001,
        "held above the original floor"
    );
    fx.ps.clear();
    assert_eq!(
        fx.quads(Vec3::ZERO, Vec3::Z, Vec3::X, Vec3::Y, &mut verts, &mut cols),
        0
    );
    assert!(verts[..12].iter().all(|p| *p == [0.; 3]));
}
#[test]
fn cap_replaces_greatest_relative_age_and_tint_uses_srgb_zone_bytes() {
    let mut fx = Effects::seeded(3);
    for i in 0..MAX_PARTICLES {
        fx.add(Particle {
            life: 10.,
            age: if i == 7 { 9. } else { 1. },
            pos: Vec3::Y,
            ..default()
        });
    }
    fx.add(Particle {
        life: 1.,
        pos: Vec3::new(9., 9., 9.),
        ..default()
    });
    assert_eq!(fx.ps.len(), MAX_PARTICLES);
    assert_eq!(fx.ps[7].pos, Vec3::splat(9.));
    let c = lit(
        earth_two_client::sky::effects::DUST_COLOUR,
        effects::light(Vec3::new(-16., 2., 4.)),
        0.4,
    );
    assert_eq!(
        c,
        Rgba {
            r: 184,
            g: 112,
            b: 76,
            a: 102
        }
    );
}
