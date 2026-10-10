//! Finite graphical review of furniture, machinery, bell, HUD and settings.
use bevy::{
    input::InputSystems,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use earth_two_client::{
    game::{
        GamePlugin, GameSet, Screen, Session, StartAt,
        menu::Menu,
        settings::Settings,
        uses::{FurnitureSeat, Kind, Uses},
    },
    landfall::maps::WorldMap,
};
const EVIDENCE: &str = "build/migration-baseline/step-10-world-hud";
fn main() {
    std::fs::create_dir_all(EVIDENCE).unwrap();
    // No account/network action; use a disposable profile for this review.
    unsafe {
        std::env::set_var(
            "EARTH_TWO_IDENTITY_PATH",
            format!("{EVIDENCE}/native-profile/pk"),
        );
    }
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: earth_two_client::physics::source_assets(),
            ..default()
        }))
        .insert_resource(StartAt::Bench)
        .add_plugins(GamePlugin)
        .insert_resource(earth_two_client::sky::Clock::held(11.))
        .insert_resource(earth_two_client::sky::Weather::from_setting(Some("0")))
        .init_resource::<Review>()
        .add_systems(PreUpdate, review.after(InputSystems))
        .add_systems(PostUpdate, release.after(GameSet::Menu))
        .run();
}
#[derive(Resource, Default)]
struct Review {
    frame: usize,
    paused_pose: Option<(Entity, earth_two_client::presentation::AnimationPlayer)>,
}
fn release(mut w: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut c in &mut w {
        c.grab_mode = CursorGrabMode::None;
    }
}
fn capture(w: &mut World, name: &str) {
    use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
    w.spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!("{EVIDENCE}/native-{name}.png")));
}

fn review(w: &mut World) {
    if !w.resource::<Session>().ready || *w.resource::<State<Screen>>().get() == Screen::Loading {
        return;
    }
    w.resource_mut::<ButtonInput<KeyCode>>().reset_all();
    w.query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(w)
        .unwrap()
        .focused = true;
    w.resource_mut::<Review>().frame += 1;
    let frame = w.resource::<Review>().frame;
    let p = w.resource::<Session>().player.unwrap();
    let key = match frame {
        30 => Some(KeyCode::Enter),
        90 | 270 | 490 | 740 => Some(KeyCode::KeyE),
        380 | 650 => Some(KeyCode::KeyP),
        590 => Some(KeyCode::KeyW),
        820 => Some(KeyCode::F1),
        830 => Some(KeyCode::F5),
        840 => Some(KeyCode::F6),
        850 => Some(KeyCode::F7),
        530 | 570 | 910 | 1230 => Some(KeyCode::Escape),
        920 | 925 | 930 | 935 | 940 | 1150 => Some(KeyCode::ArrowDown),
        950 | 1160 => Some(KeyCode::Enter),
        1260 => Some(KeyCode::Escape),
        1300 => Some(KeyCode::F1),
        _ => None,
    };
    if let Some(key) = key {
        w.resource_mut::<ButtonInput<KeyCode>>().press(key);
    }
    match frame {
        80 => {
            assert_eq!(
                w.resource::<earth_two_client::vehicle::Prompt>().text,
                "Sit down"
            );
            capture(w, "bench-offer");
        }
        220 => {
            assert!(w.get::<FurnitureSeat>(p).is_some());
            capture(w, "seated");
        }
        360 => {
            assert!(
                w.get::<earth_two_client::character::CharacterController>(p)
                    .is_some()
            );
            capture(w, "standing");
        }
        370 | 640 => {
            let target = w
                .resource::<Uses>()
                .review(if frame == 370 {
                    Kind::Repair
                } else {
                    Kind::Ring
                })
                .unwrap()
                .approach();
            let mut m = w.resource_mut::<WorldMap>();
            m.marked = true;
            m.dest = target.xz();
        }
        480 => {
            assert_eq!(
                w.resource::<earth_two_client::vehicle::Prompt>().text,
                "Work on it"
            );
            capture(w, "machine-offer");
        }
        520 => {
            assert_eq!(
                w.get::<earth_two_client::character::Intent>(p)
                    .unwrap()
                    .hold,
                earth_two_client::character::Anim::Fix
            );
            for child in w.get::<Children>(p).unwrap() {
                if let Some(st) = w.get::<earth_two_client::character::State>(*child) {
                    assert_eq!(st.current, earth_two_client::character::Anim::Fix);
                    let anim = w
                        .get::<earth_two_client::presentation::AnimationPlayer>(*child)
                        .unwrap();
                    assert_eq!(anim.clip(), "Fixing_Kneeling");
                    info!("Repair pose: {} at {}", anim.clip(), anim.time());
                }
            }
            capture(w, "working");
        }
        531 => {
            let body = w
                .get::<Children>(p)
                .unwrap()
                .iter()
                .find(|e| w.get::<earth_two_client::character::Body>(*e).is_some())
                .unwrap();
            let pose = w
                .get::<earth_two_client::presentation::AnimationPlayer>(body)
                .unwrap()
                .clone();
            assert!(pose.paused);
            w.resource_mut::<Review>().paused_pose = Some((body, pose));
        }
        550 => {
            let (body, pose) = w.resource::<Review>().paused_pose.as_ref().unwrap();
            assert_eq!(
                w.get::<earth_two_client::presentation::AnimationPlayer>(*body)
                    .unwrap(),
                pose,
                "pause changed the body pose while cloth was held"
            );
            capture(w, "paused-working");
        }
        730 => {
            assert_eq!(
                w.resource::<earth_two_client::vehicle::Prompt>().text,
                "Ring the bell"
            );
            capture(w, "bell-offer");
        }
        750 => capture(w, "bell-ring"),
        890 => capture(w, "testing-hud"),
        1000 => {
            assert_eq!(w.resource::<Menu>().screen(), Screen::Settings);
            capture(w, "settings");
        }
        1030 | 1035 | 1040 => {
            let (window, position) = {
                let (id, window) = w
                    .query_filtered::<(Entity, &Window), With<PrimaryWindow>>()
                    .single(w)
                    .unwrap();
                let sc = (window.height() / 760.)
                    .min(window.width() / 1150.)
                    .clamp(0.7, 1.6);
                let top = ((window.height() / sc - 362.) / 2.).max(24.);
                (id, Vec2::new(220. * sc, (top + 129.) * sc))
            };
            w.write_message(bevy::window::WindowEvent::CursorMoved(
                bevy::window::CursorMoved {
                    window,
                    position,
                    delta: None,
                },
            ));
            if frame != 1030 {
                w.write_message(bevy::window::WindowEvent::MouseButtonInput(
                    bevy::input::mouse::MouseButtonInput {
                        window,
                        button: MouseButton::Left,
                        state: if frame == 1035 {
                            bevy::input::ButtonState::Pressed
                        } else {
                            bevy::input::ButtonState::Released
                        },
                    },
                ));
            }
        }
        1100 => {
            assert!(
                (w.resource::<Settings>().volume - 0.9).abs() < 0.001,
                "pointer volume button"
            );
            capture(w, "settings-volume");
        }
        1190 => {
            w.query_filtered::<&mut Window, With<PrimaryWindow>>()
                .single_mut(w)
                .unwrap()
                .resolution
                .set(800., 600.);
        }
        1210 => capture(w, "settings-small"),
        1280 => capture(w, "hud-small"),
        1330 => {
            info!(
                "World/HUD graphical smoke passed: sit, stand, repair, bell, time/weather/shadows, pointer volume, resize"
            );
            w.write_message(AppExit::Success);
        }
        _ => {}
    }
}
