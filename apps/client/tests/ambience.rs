//! Reference expectations from game/cues_test.go and ambience.go.
use bevy::prelude::*;
use earth_two_client::{
    game::{
        ambience::Ambience,
        sound::{Listener, LoopState, pan_levels},
    },
    landfall::surface::Soundscape,
};
fn ear(at: Vec3) -> Listener {
    Listener {
        at,
        right: Vec3::X,
        active: true,
    }
}
fn close(a: f32, b: f32) {
    assert!((a - b).abs() < 0.0001, "{a} != {b}");
}
fn settle(at: Vec3, storm: f32, speed: f32, playing: bool) -> Ambience {
    let mut mix = Ambience::default();
    for _ in 0..600 {
        mix.update(
            ear(at),
            &Soundscape::default(),
            storm,
            speed,
            playing,
            1. / 60.,
        );
    }
    mix
}
#[test]
fn sounds_fall_off_and_pan_like_go() {
    let listener = ear(Vec3::ZERO);
    assert_eq!(listener.spatial(Vec3::Z, 3., 30.), (1., 0.));
    assert_eq!(listener.spatial(Vec3::Z * 31., 3., 30.), (0., 0.));
    let (mid, _) = listener.spatial(Vec3::Z * 15., 3., 30.);
    assert!(mid > 0. && mid < 1.);
    assert!(listener.spatial(Vec3::X * 10., 3., 30.).1 > 0.5);
    assert!(listener.spatial(Vec3::NEG_X * 10., 3., 30.).1 < -0.5);
    assert_eq!(
        Listener::default().spatial(Vec3::X * 100., 3., 30.),
        (1., 0.)
    );
    assert_eq!(pan_levels(0.), [0.6875, 0.6875]);
    assert_eq!(pan_levels(-1.), [1., 0.]);
    assert_eq!(pan_levels(1.), [0., 1.]);
}
#[test]
fn beds_match_go_zones_storm_rush_and_menu_duck() {
    let outside = Vec3::new(100., 2., 100.);
    let pads = settle(outside, 0., 0., true);
    close(pads.loops[0].volume, 0.75 * 0.55);
    assert!(!pads.loops[1].playing);
    let dome = settle(Vec3::new(20., 2., -30.), 0., 0., true);
    close(dome.loops[0].volume, 0.12 * 0.75 * 0.55);
    close(dome.loops[2].volume, 0.12 * 0.55);
    close(dome.loops[3].volume, 0.55 * 0.55);
    let hull = settle(Vec3::new(-16., 2., 4.), 0., 0., true);
    close(hull.loops[2].volume, 0.55);
    close(hull.loops[3].volume, 0.2 * 0.55);
    let storm = settle(outside, 1., -25., true);
    close(storm.loops[0].volume, 0.3 * 1.25 * 0.55);
    close(storm.loops[1].volume, 0.55);
    let menu = settle(outside, 1., -25., false);
    for (normal, quiet) in storm.loops.iter().zip(menu.loops) {
        close(quiet.volume, normal.volume * 0.4);
    }
}
#[test]
fn nearest_source_changes_pan_and_stops_beyond_earshot() {
    let mut scape = Soundscape::default();
    scape
        .sources
        .insert("generator".into(), vec![Vec3::X * -10., Vec3::X * 15.]);
    let mut mix = Ambience::default();
    for _ in 0..300 {
        mix.update(ear(Vec3::ZERO), &scape, 0., 0., true, 1. / 60.);
    }
    assert!(mix.loops[4].playing && mix.loops[4].pan < -0.69);
    for _ in 0..300 {
        mix.update(ear(Vec3::X * 10.), &scape, 0., 0., true, 1. / 60.);
    }
    assert!(mix.loops[4].pan > 0.69);
    for _ in 0..300 {
        mix.update(ear(Vec3::Z * 100.), &scape, 0., 0., true, 1. / 60.);
    }
    assert!(!mix.loops[4].playing);
    assert_eq!(mix.loops[4].volume, 0.);
}
#[test]
fn loop_fades_follow_elapsed_time_and_inactive_views_do_not_advance() {
    let mut a = LoopState::default();
    let mut b = a;
    for _ in 0..30 {
        a.set(0.8, 1., 0.7, 1. / 30.);
    }
    for _ in 0..144 {
        b.set(0.8, 1., 0.7, 1. / 144.);
    }
    close(a.volume, b.volume);
    close(a.pan, b.pan);
    a.set(0., 1., 0., 0.01);
    assert!(a.playing);
    for _ in 0..600 {
        a.set(0., 1., 0., 1. / 60.);
    }
    assert!(!a.playing);
    assert_eq!(a.volume, 0.);
    a.set(0.01, 0., 0., 1.);
    assert!(!a.playing);
    a.set(0.011, 0., 0., 1.);
    assert!(a.playing);
    let mut mix = settle(Vec3::new(100., 2., 100.), 0., 0., true);
    let before = mix.loops[0].volume;
    mix.update(
        Listener::default(),
        &Soundscape::default(),
        1.,
        25.,
        false,
        10.,
    );
    assert_eq!(mix.loops[0].volume, before);
}
