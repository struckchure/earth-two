//! Makes the vehicles drivable: the player walks up to one, presses E, sits
//! in it and drives it anywhere, and presses E to get out. Port of the Go
//! `vehicle` package.
//!
//! A drivable vehicle is a root entity with a [`sim::Vehicle`] (the wheeled
//! vehicle illusion had from Jolt, here stepped on an Avian body), its
//! [`Drivable`] and its input and state; its chassis is one convex hull
//! child collider, and each wheel model is a child posed from
//! [`sim::VehicleState`] so it spins, steers and rides its suspension.
//! Parked, with nobody in it, a vehicle is Static: it costs nothing, and
//! needs no ground colliders under it. Getting in makes it Dynamic.
//!
//! How a vehicle is built (its wheels, seat and chassis) comes from its
//! piece in world.json, as a [`Spec`] (see tools/world/vehicles.py); how it
//! handles is a table ([`handling::handling`]), so tuning needs no rebuild
//! of the assets.
//!
//! Driver input is a [`Controls`] component on the vehicle root that the
//! interaction port fills from the keys; seats, getting in and out are the
//! character port's, through [`Seats`], [`drive::enter`],
//! [`drive::EnteredVehicle`] and [`drive::LeftVehicle`].

pub mod assemble;
pub mod drive;
pub mod handling;
pub mod headlamps;
pub mod impacts;
pub mod pedestrians;
pub mod sim;
pub mod spec;
pub mod tyres;

use avian3d::prelude::*;
use bevy::{prelude::*, transform::TransformSystems};

pub use assemble::{ParkedSpec, VehicleKit, spawn_parked};
pub use drive::{
    Controls, Driving, EnteredVehicle, LeftVehicle, Prompt, REACH, enter, nearest_seat,
};
pub use handling::{Handling, SeatAnim, handling};
pub use headlamps::{Headlamp, HeadlampBeam, LightCycle};
pub use impacts::ImpactRules;
pub use sim::{Vehicle, VehicleInput, VehicleState, VehicleStep, WheelState};
pub use spec::{SeatSpec, Spec};
pub use tyres::Tyre;

use crate::character::CharacterSystems;

/// A vehicle the player can get into. It goes on the root, the physics
/// body.
#[derive(Component, Debug, Clone)]
pub struct Drivable {
    /// What the HUD calls it: "buggy".
    pub name: String,
    pub spec: Spec,
    pub handling: Handling,
    /// Who's in the driving seat; None for nobody.
    pub driver: Option<Entity>,
    /// Where it was parked to begin with, for when it's lost off the edge
    /// of the world.
    pub home: (Vec3, Quat),
    /// The current power state. Automatic lamps follow the driver and the
    /// night; lamps explicitly switched on stay on when parked.
    pub headlamps: bool,
    pub(crate) lights_mode: headlamps::HeadlampMode,

    pub(crate) steer: f32,    // the steering as eased toward the keys
    pub(crate) still: f32,    // seconds at rest with nobody in it
    pub(crate) upset: f32,    // seconds on its side or roof
    pub(crate) righting: f32, // seconds R has been held to right it
    pub(crate) tyred: bool,   // whether its Tyres are out, parked
}

impl Drivable {
    /// Switches the lamps by hand, as H does: on stays on when parked, off
    /// lasts the drive.
    pub fn switch_headlamps(&mut self, on: bool) {
        self.lights_mode = if on {
            headlamps::HeadlampMode::On
        } else {
            headlamps::HeadlampMode::Off
        };
    }

    pub fn new(name: &str, spec: Spec, handling: Handling, home: (Vec3, Quat)) -> Self {
        Drivable {
            name: name.to_string(),
            spec,
            handling,
            driver: None,
            home,
            headlamps: false,
            lights_mode: headlamps::HeadlampMode::Automatic,
            steer: 0.0,
            still: 0.0,
            upset: 0.0,
            righting: 0.0,
            tyred: false,
        }
    }
}

/// The vehicle's seats, as its Spec has them, for the character port: the
/// first is the driver's.
#[derive(Component, Debug, Clone, Default)]
pub struct Seats(pub Vec<SeatSpec>);

/// Marks a wheel model, a child of its vehicle's root.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WheelOf {
    pub index: usize,
}

/// An optional resource the game inserts: whether there's ground to
/// collide with at x, z. A vehicle left where there isn't any is parked
/// (made Static) at once, before it can fall through the world.
#[derive(Resource, Default)]
pub struct GroundCover {
    pub covered: Option<Box<dyn Fn(f32, f32) -> bool + Send + Sync>>,
}

/// The vehicle systems, for ordering others against them.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VehicleSystems {
    /// Update: assembling, steering, offering, presenting and the tyres;
    /// after the character's input and before it acts, as the Go plugin
    /// chained them.
    Drive,
    /// FixedUpdate: parking, running people over, and waking pedestrians,
    /// around the vehicle step.
    Fixed,
}

/// Runs the vehicles. It needs `EarthPhysicsPlugin`; the character plugin
/// supplies the people it drives into.
pub struct VehiclePlugin;

impl Plugin for VehiclePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(sim::VehicleSimPlugin)
            .init_resource::<Prompt>()
            .init_resource::<Driving>()
            .init_resource::<LightCycle>()
            .init_resource::<VehicleKit>()
            .init_resource::<ImpactRules>()
            .init_resource::<impacts::ImpactMemory>()
            .init_resource::<pedestrians::PedestrianPhysics>()
            .init_resource::<crate::character::Controls>()
            .add_message::<EnteredVehicle>()
            .add_message::<LeftVehicle>()
            .configure_sets(
                Update,
                VehicleSystems::Drive
                    .after(CharacterSystems::Input)
                    .before(CharacterSystems::Act),
            )
            .add_systems(
                Update,
                (
                    assemble::collect_kit_specs,
                    assemble::assemble,
                    drive::steer,
                    drive::present,
                    tyres::tyres,
                )
                    .chain()
                    .in_set(VehicleSystems::Drive),
            )
            .add_systems(
                FixedUpdate,
                (
                    drive::settle.before(tyres::tyres),
                    tyres::tyres,
                    pedestrians::prepare_pedestrians
                        .after(impacts::run_over)
                        .before(CharacterSystems::Move),
                    // Go predicts impacts before physics.Prepare/moveCharacters.
                    // Moving an upright capsule first can evade the predictive
                    // hit and leave an infinite-mass body under the wheels.
                    impacts::run_over.before(CharacterSystems::Move),
                    pedestrians::restore_pedestrian_steps
                        .after(CharacterSystems::Move)
                        .after(impacts::run_over),
                )
                    .in_set(VehicleSystems::Fixed),
            )
            .add_systems(
                PhysicsSchedule,
                pedestrians::character_response.in_set(SolverSystems::PreSubstep),
            )
            .add_systems(
                FixedPostUpdate,
                impacts::impact_contacts.after(PhysicsSystems::Writeback),
            )
            .add_systems(
                PostUpdate,
                headlamps::update_headlamps.before(TransformSystems::Propagate),
            );
        #[cfg(feature = "viewer")]
        app.add_systems(
            PostUpdate,
            headlamps::light_headlamps.after(headlamps::update_headlamps),
        );
    }
}
