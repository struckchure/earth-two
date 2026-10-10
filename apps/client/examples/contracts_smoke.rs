//! Finite graphical review of PAD-001 through live keyboard, navigation and UI.
use bevy::{
    input::InputSystems,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use earth_two_client::{
    game::{
        GamePlugin, GameSet, Screen, Session,
        contracts::{ContractState, Contracts},
        menu::Menu,
    },
    landfall::maps::WorldMap,
};
const EVIDENCE: &str = "build/migration-baseline/step-9-contracts";
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
    w.resource_mut::<ButtonInput<MouseButton>>().reset_all();
    {
        let mut window = w
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(w)
            .unwrap();
        window.focused = true;
    }
    w.resource_mut::<Review>().frame += 1;
    let frame = w.resource::<Review>().frame;
    let key = match frame {
        30 => Some(KeyCode::Enter),
        80 => Some(KeyCode::KeyJ),
        140 => Some(KeyCode::KeyJ),
        180 | 440 => Some(KeyCode::KeyP),
        300 | 570 => Some(KeyCode::KeyE),
        700 => Some(KeyCode::KeyJ),
        760 => Some(KeyCode::KeyJ),
        _ => None,
    };
    if let Some(k) = key {
        w.resource_mut::<ButtonInput<KeyCode>>().press(k);
    }
    match frame {
        120 => {
            assert_eq!(w.resource::<Menu>().screen(), Screen::ContractJournal);
            capture(w, "journal-available");
        }
        160 | 420 => {
            let c = w.resource::<Contracts>();
            let target = if frame == 160 { c.offer } else { c.delivery };
            let mut m = w.resource_mut::<WorldMap>();
            m.marked = true;
            m.dest = target.xz();
        }
        290 => capture(w, "terminal"),
        340 => {
            assert_eq!(
                w.resource::<Menu>().screen(),
                Screen::ContractOffer,
                "terminal E must review"
            );
            capture(w, "offer");
        }
        375 | 380 | 382 => {
            let (window, size) = {
                let (entity, window) = w
                    .query_filtered::<(Entity, &Window), With<PrimaryWindow>>()
                    .single(w)
                    .unwrap();
                (entity, Vec2::new(window.width(), window.height()))
            };
            let sc = earth_two_client::game::navigation::scale(size);
            let at = Vec2::new(
                ((size.x / sc - 540.) / 2.).max(16.),
                ((size.y / sc - 580.) / 2.).max(24.),
            );
            let position = (at + Vec2::new(270., 467.)) * sc;
            w.get_mut::<Window>(window)
                .unwrap()
                .set_cursor_position(Some(position));
            w.write_message(bevy::window::WindowEvent::CursorMoved(
                bevy::window::CursorMoved {
                    window,
                    position,
                    delta: None,
                },
            ));
            if frame != 375 {
                w.write_message(bevy::window::WindowEvent::MouseButtonInput(
                    bevy::input::mouse::MouseButtonInput {
                        window,
                        button: MouseButton::Left,
                        state: if frame == 380 {
                            bevy::input::ButtonState::Pressed
                        } else {
                            bevy::input::ButtonState::Released
                        },
                    },
                ));
            }
        }
        405 => {
            assert_eq!(w.resource::<Contracts>().state, ContractState::Accepted);
            capture(w, "filed");
        }
        550 => {
            assert_eq!(w.resource::<Contracts>().state, ContractState::Accepted);
            capture(w, "delivery");
        }
        570 => capture(w, "handover-frame"),
        600 => {
            assert_eq!(w.resource::<Menu>().screen(), Screen::ContractJournal);
            assert_eq!(w.resource::<Contracts>().debt, 1850);
            capture(w, "settled");
        }
        670 => capture(w, "journal-receipt"),
        740 => capture(w, "completed-hud"),
        790 => {
            w.query_filtered::<&mut Window, With<PrimaryWindow>>()
                .single_mut(w)
                .unwrap()
                .resolution
                .set(800., 600.);
        }
        850 => capture(w, "journal-small"),
        890 => {
            assert_eq!(w.resource::<Contracts>().completed.len(), 1);
            assert_eq!(w.resource::<Contracts>().balance, 0);
            info!("Contract graphical smoke passed: review, accept, deliver, journal and resize");
            w.write_message(AppExit::Success);
        }
        _ => {}
    }
}
