//! Go cue reference thresholds and transition regressions.
use bevy::prelude::*;
use earth_two_client::{
    character::{Anim, CharacterController},
    game::{
        Screen,
        cues::{BodyMemory, CueRandom, Foot, Voice, stepping},
        drive_sound::{DriveSound, engine_sound},
        sound::Listener,
        ui_sound::UiSoundState,
    },
    landfall::surface::Footing,
    vehicle::{VehicleInput, VehicleState, WheelState},
};
#[test]
fn feet_follow_plants_not_a_timer() {
    let walk = |stride: f32, lift: f32| {
        let mut f = Foot::default();
        let mut steps = 0;
        for i in 0..600 {
            let phase = i as f32 / 60. / stride;
            let h = 0.09 + lift * (std::f32::consts::TAU * phase).sin().max(0.);
            if f.track(h, 1. / 60.) {
                steps += 1;
            }
        }
        steps
    };
    assert!((8..=10).contains(&walk(1.1, 0.18)));
    assert!((13..=15).contains(&walk(0.7, 0.3)));
    assert_eq!(walk(1., 0.02), 0);
    for a in [
        Anim::Idle,
        Anim::Walk,
        Anim::Run,
        Anim::StairsUp,
        Anim::StairsDown,
        Anim::Crouch,
        Anim::CrouchWalk,
    ] {
        assert!(stepping(a));
    }
    for a in [
        Anim::Jump,
        Anim::Fall,
        Anim::Roll,
        Anim::Slide,
        Anim::Vault,
        Anim::LadderClimb,
        Anim::Sitting,
        Anim::Drive,
        Anim::Punch,
        Anim::Fix,
    ] {
        assert!(!stepping(a));
    }
}
#[test]
fn jump_landing_and_actions_only_sound_on_transitions() {
    let mut rng = CueRandom::default();
    let mut cc = CharacterController {
        grounded: true,
        ..default()
    };
    let mut m = BodyMemory::new(Anim::Idle, true);
    assert!(
        m.update(Anim::Idle, &cc, Footing::Paving, 0.1, &mut rng)
            .is_empty()
    );
    cc.grounded = false;
    cc.velocity.y = 4.;
    assert_eq!(
        m.update(Anim::Jump, &cc, Footing::Paving, 0.1, &mut rng),
        vec![("cloth", 0.5, 1.)]
    );
    cc.velocity.y = -8.5;
    assert!(
        m.update(Anim::Fall, &cc, Footing::Sand, 0.1, &mut rng)
            .is_empty()
    );
    cc.grounded = true;
    cc.velocity = Vec3::ZERO;
    let cues = m.update(Anim::Idle, &cc, Footing::Sand, 0.1, &mut rng);
    assert_eq!(cues[0], ("step_sand", 1., 0.9));
    assert_eq!(cues[1].0, "land_loose");
    assert!((cues[1].1 - 0.9).abs() < 0.00001);
    assert!(
        m.update(Anim::Idle, &cc, Footing::Sand, 0.1, &mut rng)
            .is_empty()
    );
    assert_eq!(
        m.update(Anim::Roll, &cc, Footing::Sand, 0.1, &mut rng),
        vec![("whoosh", 0.5, 0.8), ("cloth", 0.6, 0.9)]
    );
    assert!(
        m.update(Anim::Roll, &cc, Footing::Sand, 0.1, &mut rng)
            .is_empty()
    );
    assert!(
        m.update(Anim::Fix, &cc, Footing::Grating, 0.2, &mut rng)
            .is_empty()
    );
    assert_eq!(
        m.update(Anim::Fix, &cc, Footing::Grating, 0.21, &mut rng),
        vec![("clank", 0.5, 1.)]
    );
}
#[test]
fn kerbs_and_controlled_landings_are_quiet() {
    let mut rng = CueRandom::default();
    for (fall, controlled) in [(2.5, false), (10., true)] {
        let mut m = BodyMemory::new(Anim::Fall, false);
        let mut cc = CharacterController {
            velocity: Vec3::NEG_Y * fall,
            controlled,
            ..default()
        };
        assert!(
            m.update(Anim::Fall, &cc, Footing::Rock, 0.1, &mut rng)
                .is_empty()
        );
        cc.grounded = true;
        assert!(
            m.update(Anim::Idle, &cc, Footing::Rock, 0.1, &mut rng)
                .is_empty()
        );
    }
}
#[test]
fn world_and_ui_mixes_keep_go_gains_and_jitter() {
    let v = Voice {
        ear: Listener {
            at: Vec3::ZERO,
            right: Vec3::X,
            active: true,
        },
        duck: 0.4,
    };
    let cue = v.at("cloth", Vec3::Z, 0.5, 3., 28., 1.);
    assert!((cue.volume - 0.16).abs() < 0.00001);
    assert_eq!(cue.jitter, 0.06);
    assert_eq!(v.at("cloth", Vec3::Z * 29., 1., 3., 28., 1.).volume, 0.);
    let cue = Voice::ui("ui_stamp", 0.9);
    assert_eq!(cue.volume, 0.45);
    assert_eq!(cue.jitter, 0.03);
}
#[test]
fn each_motor_and_tyres_follow_revs_surface_pause_and_exit() {
    let state = VehicleState {
        rpm: 6000.,
        gear: 1,
        speed: 18.,
        touching: 4,
        wheels: vec![WheelState::default(); 4],
    };
    let input = VehicleInput {
        forward: 1.,
        ..default()
    };
    for (i, kind) in ["bike", "trike", "buggy", "rover", "truck"]
        .into_iter()
        .enumerate()
    {
        let snd = engine_sound(kind);
        assert_eq!(snd.index, i);
        let mut m = DriveSound::default();
        for _ in 0..600 {
            m.tick(1. / 60.);
            m.engine(kind, 1000., 6000., &state, &input, true, true, 1. / 60.);
        }
        assert!((m.loops[i].pitch - snd.pitch * snd.rev).abs() < 0.0001);
        assert!((m.loops[i].volume - (snd.idle + snd.throttling + 0.1) * 0.8).abs() < 0.0001);
        assert!((m.loops[5].volume - 0.64).abs() < 0.0001);
        assert_eq!(m.loops[..5].iter().filter(|s| s.playing).count(), 1);
        for _ in 0..600 {
            m.engine(kind, 1000., 6000., &state, &input, false, true, 1. / 60.);
        }
        assert!((m.loops[5].volume - 0.32).abs() < 0.0001);
        let mut shifted = state.clone();
        shifted.gear = 2;
        m.engine(kind, 1000., 6000., &shifted, &input, false, true, 1. / 60.);
        assert_eq!(m.shift, 0.18);
        assert!(m.loops[i].pitch < snd.pitch * snd.rev);
        for _ in 0..600 {
            m.engine(kind, 1000., 6000., &state, &input, false, false, 1. / 60.);
        }
        assert!(m.loops.iter().all(|s| !s.playing));
        m.engine(kind, 1000., 6000., &state, &input, false, true, 1.);
        assert!(m.loops[i].playing);
        for _ in 0..600 {
            m.quiet(1. / 60.);
        }
        assert!(m.loops.iter().all(|s| !s.playing));
    }
}
#[test]
fn skids_and_crashes_respect_thresholds_and_cooldowns() {
    let mut m = DriveSound::default();
    assert_eq!(m.skid_volume(3.5, 6., 1., 0.5), None);
    assert_eq!(m.skid_volume(4., 10., 0.4, 1.), None);
    assert!(m.skid_volume(4., 10., 1., 1.).is_some());
    assert_eq!(m.skid_volume(10., 20., 1., 1.), None);
    m.tick(1.2);
    assert!(m.skid_volume(0., 7., 1., 1.).is_some());
    assert_eq!(m.crash_sound(2.49, false), None);
    assert_eq!(m.crash_sound(14.5, false), Some(("crash_metal", 1., 0.85)));
    assert_eq!(m.crash_sound(20., true), None);
    m.tick(0.3);
    assert_eq!(m.crash_sound(2.5, true), Some(("crash_ground", 0.5, 1.)));
}
#[test]
fn menu_transitions_and_new_notes_sound_once() {
    let mut m = UiSoundState::default();
    assert!(m.update(Screen::Loading, 0, 0, None, "").is_empty());
    assert!(m.update(Screen::Title, 1, 0, None, "").is_empty());
    assert_eq!(
        m.update(Screen::Playing, 0, 0, None, "")[0].name,
        "ui_stamp"
    );
    assert_eq!(m.update(Screen::Paused, 1, 0, None, "")[0].name, "ui_page");
    assert_eq!(m.update(Screen::Paused, 1, 1, None, "")[0].name, "ui_move");
    assert!(m.update(Screen::Paused, 1, 1, None, "").is_empty());
    assert_eq!(m.update(Screen::Title, 1, 0, None, "")[0].name, "ui_stamp");
    assert_eq!(
        m.update(Screen::Playing, 0, 0, None, "")[0].name,
        "ui_stamp"
    );
    assert_eq!(
        m.update(Screen::Playing, 0, 0, None, "Slow down to get out")[0].name,
        "ui_deny"
    );
    assert!(
        m.update(Screen::Playing, 0, 0, None, "Slow down to get out")
            .is_empty()
    );
}

#[test]
fn map_marks_clear_and_arrival_have_the_source_cues_once() {
    let mut memory = UiSoundState::default();
    memory.update(Screen::Playing, 0, 0, None, "");
    assert_eq!(
        memory.update(Screen::Mapping, 1, 0, None, "")[0].name,
        "ui_open"
    );
    assert!(
        memory
            .navigation(Screen::Mapping, false, Vec2::ZERO)
            .is_empty()
    );
    assert_eq!(
        memory.navigation(Screen::Mapping, true, Vec2::ONE)[0].name,
        "ui_mark"
    );
    assert!(
        memory
            .navigation(Screen::Mapping, true, Vec2::ONE)
            .is_empty()
    );
    assert_eq!(
        memory.navigation(Screen::Mapping, false, Vec2::ONE)[0].name,
        "ui_mark"
    );
    memory.navigation(Screen::Mapping, true, Vec2::ONE);
    assert_eq!(
        memory.navigation(Screen::Playing, false, Vec2::ONE)[0].name,
        "ui_arrive"
    );
    assert!(
        memory
            .navigation(Screen::Playing, false, Vec2::ONE)
            .is_empty()
    );
}
