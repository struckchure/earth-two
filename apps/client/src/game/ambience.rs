//! game/ambience.go: the same zone beds and nearest machines as the Go client.
use super::{
    GameSet, Screen,
    camera::GameCamera,
    sound::{Listener, LoopState, MENU_DUCK, MIX_AMBIENCE},
};
use crate::{
    landfall::{surface::Soundscape, zones::places_at},
    sky::Weather,
    vehicle::Driving,
};
use bevy::prelude::*;

pub const NAMES: [&str; 8] = [
    "wind",
    "storm",
    "hull_hum",
    "dome_air",
    "generator",
    "fans",
    "fountain",
    "market",
];
pub const SOURCE_RANGES: [(f32, f32); 4] = [(3., 24.), (3., 18.), (2., 22.), (5., 32.)];

#[derive(Resource, Default)]
pub struct Ambience {
    pub loops: [LoopState; 8],
}
impl Ambience {
    pub fn update(
        &mut self,
        ear: Listener,
        scape: &Soundscape,
        storm: f32,
        speed: f32,
        playing: bool,
        dt: f32,
    ) {
        if !ear.active {
            return;
        }
        let at = places_at(ear.at);
        let rush = (speed.abs() / 25.).clamp(0., 1.);
        let gain = MIX_AMBIENCE * if playing { 1. } else { MENU_DUCK };
        let beds = [
            (at.outside * (1. - 0.7 * storm) + 0.12 * at.dome) * (0.75 + 0.5 * rush),
            storm * (at.outside + 0.3 * at.dome + 0.1 * at.hull),
            at.hull + 0.12 * at.dome,
            0.55 * at.dome + 0.2 * at.hull,
        ];
        for (s, volume) in self.loops.iter_mut().zip(beds) {
            s.set(volume * gain, 1., 0., dt);
        }
        for (i, (near, far)) in SOURCE_RANGES.into_iter().enumerate() {
            let index = i + 4;
            let Some((at, _)) = scape.nearest(NAMES[index], ear.at) else {
                continue;
            };
            let (volume, pan) = ear.spatial(at, near, far);
            self.loops[index].set(volume * gain, 1., pan, dt);
        }
    }
}

pub struct AmbiencePlugin;
impl Plugin for AmbiencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Ambience>()
            .add_systems(Update, mix.after(GameSet::Camera));
        #[cfg(feature = "viewer")]
        app.add_plugins(super::sound::output::SoundOutputPlugin);
    }
}
pub(super) fn mix(
    cameras: Query<&Transform, With<GameCamera>>,
    scape: Option<Res<Soundscape>>,
    weather: Res<Weather>,
    driving: Res<Driving>,
    screen: Res<State<Screen>>,
    time: Res<Time>,
    mut ambience: ResMut<Ambience>,
) {
    let (Ok(camera), Some(scape)) = (cameras.single(), scape) else {
        return;
    };
    ambience.update(
        Listener {
            at: camera.translation,
            right: *camera.right(),
            active: true,
        },
        &scape,
        weather.storm,
        driving.speed,
        *screen.get() == Screen::Playing,
        time.delta_secs(),
    );
}
