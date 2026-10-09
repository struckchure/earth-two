//! Application wiring from game/game.go and menu.go. Subsystem plugins and
//! loaded content live for the app's lifetime. As in Go, Main menu preserves
//! the current world; Play resumes it instead of spawning a second session.
pub mod ambience;
pub mod camera;
pub mod crowd;
pub mod cues;
pub mod drive_sound;
pub mod effects;
pub mod injuries;
pub mod menu;
pub mod people;
#[cfg(feature = "viewer")]
mod render;
pub mod residents;
pub mod seats;
pub mod sound;
pub mod ui_sound;
pub mod wardrobe;
#[cfg(feature = "viewer")]
mod wardrobe_render;

use crate::{
    character::{
        self, CharacterController, CharacterPlugin, CharacterSystems, Intent, Player, Traversal,
    },
    landfall::{LandfallPlugin, LandfallSet, SpawnLandfall, StreamCentre, terrain::Terrain},
    physics::{EarthPhysicsPlugin, set_paused},
    presentation::{PresentationPlugin, Roster, Wardrobe, content::starting_outfit},
    vehicle::{self, VehiclePlugin, VehicleSystems},
    world::{KitAsset, LayoutAsset, LayoutRoot, WorldPlugin},
};
use avian3d::prelude::*;
use bevy::prelude::*;
use earth_two_world::terrain::{ARRIVAL, CHUNK_SIZE, COLLIDER_RADIUS};

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    #[default]
    Loading,
    Title,
    Playing,
    Paused,
    Dressing,
}

#[derive(Message, Debug, Clone, Copy)]
pub enum MenuAction {
    Play,
    Pause,
    Resume,
    MainMenu,
    Wardrobe,
    Back,
    Focus(usize),
    CycleRow { row: usize, step: i32 },
}

#[derive(Resource, Default)]
pub struct Session {
    pub player: Option<Entity>,
    pub error: Option<String>,
    pub ready: bool,
}

#[derive(Resource)]
pub struct GameAssets {
    pub kit: Handle<KitAsset>,
    pub layout: Handle<LayoutAsset>,
}

/// Review entry points only; normal play always starts at Go's arrival.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub enum StartAt {
    #[default]
    Arrival,
    Hull,
    Buggy,
    Bike,
    Crowd,
    Injury,
    Fatal,
    Traversal,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameSet {
    Menu,
    Input,
    Seats,
    Camera,
}

/// The authored Hull review block is lifted clear of the live terrain.
pub const TRAVERSAL_ORIGIN: Vec3 = Vec3::new(0.0, 40.0, 0.0);

pub struct GamePlugin;
impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            EarthPhysicsPlugin,
            WorldPlugin,
            CharacterPlugin,
            VehiclePlugin,
            PresentationPlugin::platform(),
            LandfallPlugin::default(),
            crate::shading::ShadingPlugin::earth_two(),
            crate::sky::SkyPlugin,
            crate::identity::IdentityPlugin,
            residents::ResidentsPlugin,
            people::PeoplePlugin,
            ambience::AmbiencePlugin,
            cues::CuesPlugin,
        ))
        .init_state::<Screen>()
        .init_resource::<Session>()
        .init_resource::<menu::Menu>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<StartAt>()
        .init_resource::<crowd::TestCrowd>()
        .add_systems(Startup, crowd::review)
        .add_systems(Update, crowd::input.before(GameSet::Menu))
        .add_systems(
            Update,
            crowd::sync
                .before(residents::ResidentsSet)
                .after(GameSet::Menu),
        )
        .init_resource::<camera::Orbit>()
        .init_resource::<vehicle::GroundCover>()
        .add_message::<MenuAction>()
        .configure_sets(
            Update,
            (GameSet::Menu, GameSet::Input)
                .chain()
                .before(CharacterSystems::Input),
        )
        .configure_sets(
            Update,
            GameSet::Seats
                .after(VehicleSystems::Drive)
                .before(CharacterSystems::Act),
        )
        .configure_sets(
            Update,
            GameSet::Camera
                .after(CharacterSystems::Act)
                .before(LandfallSet::Stream),
        )
        .add_systems(Startup, request_world)
        .add_systems(
            Update,
            (menu::keys, menu::actions, lock_controls)
                .chain()
                .in_set(GameSet::Menu),
        )
        .add_systems(Update, camera::steer.in_set(GameSet::Input))
        .add_systems(
            Update,
            wardrobe::face_camera
                .after(GameSet::Seats)
                .after(CharacterSystems::Act)
                .before(GameSet::Camera),
        )
        .add_systems(
            Update,
            (injuries::recover, injuries::review_injury)
                .chain()
                .after(GameSet::Input)
                .before(CharacterSystems::Input),
        )
        .add_systems(
            Update,
            injuries::release_downed_drivers
                .after(injuries::recover)
                .before(VehicleSystems::Drive),
        )
        .add_systems(Update, (spawn_player, finish_loading).chain())
        .add_systems(
            Update,
            seats::driving_input
                .before(VehicleSystems::Drive)
                .after(CharacterSystems::Input),
        )
        .add_systems(
            Update,
            (seats::offer, seats::receive, seats::sit)
                .chain()
                .in_set(GameSet::Seats),
        )
        .add_systems(
            Update,
            (camera::follow, respawn).chain().in_set(GameSet::Camera),
        )
        .add_systems(Update, cover_ground.after(LandfallSet::Stream))
        .add_systems(OnEnter(Screen::Playing), unlock)
        .add_systems(OnExit(Screen::Playing), lock);
        #[cfg(feature = "viewer")]
        app.add_plugins((
            render::GameRenderPlugin,
            crate::presentation::pose::PosePlugin,
        ));
    }
}

fn request_world(
    mut commands: Commands,
    assets: Res<AssetServer>,
    start: Res<StartAt>,
    mut layouts: ResMut<Assets<LayoutAsset>>,
) {
    let request = SpawnLandfall::load(&assets);
    commands.insert_resource(GameAssets {
        kit: request.layout.kit.clone(),
        layout: request.layout.layout.clone(),
    });
    commands.spawn(request);
    if matches!(*start, StartAt::Traversal) {
        // Go's development block is intentionally absent from packed assets.
        // Embed placements only; retain the exact authored models/colliders.
        let mut layout = earth_two_world::kit::Layout::parse(include_bytes!(
            "../../../../assets/world/hull_block.json"
        ))
        .expect("valid Hull block");
        for piece in &mut layout.pieces {
            piece.at = (Vec3::from(piece.at) + TRAVERSAL_ORIGIN).to_array();
        }
        commands.spawn(crate::world::SpawnLayout {
            kit: assets.load("world/world.json"),
            layout: layouts.add(LayoutAsset(layout)),
        });
        commands.spawn((
            RigidBody::Static,
            Collider::cuboid(40.0, 1.0, 40.0),
            Transform::from_translation(TRAVERSAL_ORIGIN - Vec3::Y * 0.5),
        ));
    }
}

fn spawn_player(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    roster: Res<Roster>,
    wardrobe: Res<Wardrobe>,
    mut session: ResMut<Session>,
    start: Res<StartAt>,
    cars: Query<(&vehicle::Drivable, &Transform)>,
) {
    if terrain.is_none() || roster.skins.is_empty() || session.player.is_some() {
        return;
    }
    let feet = match *start {
        StartAt::Arrival | StartAt::Crowd | StartAt::Injury | StartAt::Fatal => ARRIVAL,
        StartAt::Hull => Vec3::new(-15.0, 0.0, 6.0),
        StartAt::Traversal => TRAVERSAL_ORIGIN + Vec3::new(1.0, 0.0, 1.95),
        StartAt::Buggy | StartAt::Bike => {
            let name = if matches!(*start, StartAt::Bike) {
                "bike"
            } else {
                "buggy"
            };
            let Some((car, tr)) =
                cars.iter()
                    .filter(|(d, _)| d.name == name)
                    .min_by(|(_, a), (_, b)| {
                        a.translation
                            .distance_squared(ARRIVAL)
                            .total_cmp(&b.translation.distance_squared(ARRIVAL))
                    })
            else {
                return;
            };
            let seat = car.spec.seats.first().unwrap();
            let (p, _) = vehicle::spec::seat_pose(tr.translation, tr.rotation, seat);
            {
                let offset = if matches!(*start, StartAt::Bike) {
                    tr.rotation * Vec3::X
                } else {
                    Vec3::X * 1.6
                };
                Vec3::new(p.x + offset.x, tr.translation.y, p.z + offset.z)
            }
        }
    };
    let feet = Vec3::new(
        feet.x,
        feet.y
            .max(earth_two_world::terrain::walk_height(feet.x, feet.z)),
        feet.z,
    );
    let player = roster.spawn(&mut commands, 0, feet, 0.0);
    commands
        .entity(player)
        .insert((Player, StreamCentre, starting_outfit(&wardrobe)));
    #[cfg(feature = "viewer")]
    commands.entity(player).insert(Visibility::Inherited);
    session.player = Some(player);
}

fn finish_loading(
    assets: Res<AssetServer>,
    game: Res<GameAssets>,
    layouts: Query<(), With<LayoutRoot>>,
    mut session: ResMut<Session>,
    screen: Res<State<Screen>>,
    mut next: ResMut<NextState<Screen>>,
) {
    for handle in [game.kit.id().untyped(), game.layout.id().untyped()] {
        if let bevy::asset::LoadState::Failed(error) = assets.load_state(handle) {
            session.error = Some(error.to_string());
        }
    }
    #[cfg(not(feature = "viewer"))]
    {
        session.ready = session.player.is_some() && !layouts.is_empty();
    }
    #[cfg(feature = "viewer")]
    {
        let _ = &layouts;
    }
    if *screen.get() == Screen::Loading && session.ready && session.error.is_none() {
        next.set(Screen::Title);
    }
}

fn lock_controls(
    screen: Res<State<Screen>>,
    mut controls: ResMut<character::Controls>,
    mut physics: ResMut<Time<Physics>>,
) {
    controls.enabled = *screen.get() == Screen::Playing;
    set_paused(&mut physics, !controls.enabled);
}
fn unlock(mut controls: ResMut<character::Controls>, mut physics: ResMut<Time<Physics>>) {
    controls.enabled = true;
    set_paused(&mut physics, false);
}
fn lock(mut controls: ResMut<character::Controls>, mut physics: ResMut<Time<Physics>>) {
    controls.enabled = false;
    set_paused(&mut physics, true);
}

fn cover_ground(
    terrain: Option<Res<Terrain>>,
    mut ground: ResMut<vehicle::GroundCover>,
    day: Res<crate::sky::Daylight>,
    mut lights: ResMut<vehicle::LightCycle>,
) {
    lights.night = day.night > 0.01 || day.dusk > 0.75;
    if let Some(t) = terrain {
        let (ci, cj) = (t.ci, t.cj);
        ground.covered = Some(Box::new(move |x, z| {
            x > (ci - COLLIDER_RADIUS) as f32 * CHUNK_SIZE + 24.0
                && x < (ci + COLLIDER_RADIUS + 1) as f32 * CHUNK_SIZE - 24.0
                && z > (cj - COLLIDER_RADIUS) as f32 * CHUNK_SIZE + 24.0
                && z < (cj + COLLIDER_RADIUS + 1) as f32 * CHUNK_SIZE - 24.0
        }));
    }
}
fn respawn(
    mut people: Query<
        (
            &mut Transform,
            &mut CharacterController,
            &mut Traversal,
            &mut Intent,
        ),
        With<Player>,
    >,
) {
    for (mut tr, mut cc, mut traversal, mut intent) in &mut people {
        if tr.translation.y < -20.0 {
            tr.translation = ARRIVAL + Vec3::Y * 2.0;
            cc.velocity = Vec3::ZERO;
            cc.walk = Vec3::ZERO;
            cc.controlled = false;
            cc.height = 1.8;
            *traversal = default();
            *intent = default();
        }
    }
}

#[cfg(feature = "viewer")]
pub use render::run;
