//! Furniture, machinery and floor bell, preserving game/use.go's authored spots.
use super::{
    GameAssets, Screen,
    camera::GameCamera,
    cues::{Cue, voice},
    menu::Menu,
    seats::Seated,
};
use crate::{
    character::{
        Anim, Body, CAPSULE_HEIGHT, CAPSULE_RADIUS, CharacterController, ControllerState, Controls,
        Intent, Player, Traversal,
    },
    landfall::{surface::Soundscape, terrain::stand_on},
    presentation::MotionSamples,
    vehicle::{Drivable, Driving, Prompt, REACH},
    world::{KitAsset, LayoutAsset},
};
use avian3d::prelude::*;
use bevy::prelude::*;
use earth_two_world::kit::{Kit, Placement};
use std::{
    collections::HashMap,
    f32::consts::{FRAC_PI_2, PI},
};
pub const USE_REACH: f32 = 1.4;
pub const SIT_DOWN: f32 = 1.3;
pub const GET_UP: f32 = 1.03;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Sit,
    Repair,
    Ring,
}
#[derive(Clone, Debug)]
pub struct Spot {
    pub piece: String,
    pub kind: Kind,
    pub at: Vec3,
    pub facing: f32,
}
impl Spot {
    pub fn approach(&self) -> Vec3 {
        self.at
            + if self.kind == Kind::Sit {
                Quat::from_rotation_y(self.facing) * Vec3::Z * 0.45
            } else {
                Vec3::ZERO
            }
    }
    pub fn stand_up(&self) -> Vec3 {
        self.at + Quat::from_rotation_y(self.facing) * Vec3::Z * 0.55
    }
}
#[derive(Resource, Default)]
pub struct Uses {
    pub cells: HashMap<(i32, i32), Vec<Spot>>,
}
fn cell(at: Vec3) -> (i32, i32) {
    ((at.x / 8.).floor() as i32, (at.z / 8.).floor() as i32)
}
impl Uses {
    /// Deterministic authored review points on the main floor.
    pub fn review(&self, kind: Kind) -> Option<&Spot> {
        let piece = match kind {
            Kind::Sit => "stone_bench",
            Kind::Repair => "pump_unit",
            Kind::Ring => "floor_bell",
        };
        self.cells
            .values()
            .flatten()
            .filter(|s| s.piece == piece)
            .min_by(|a, b| {
                a.at.length_squared()
                    .total_cmp(&b.at.length_squared())
                    .then_with(|| a.at.x.total_cmp(&b.at.x))
            })
    }
    pub fn new(kit: &Kit, placed: &[Placement]) -> Self {
        let mut out = Self::default();
        for p in placed {
            let (kind, xs, z, y): (Kind, &[f32], f32, f32) = match p.piece.as_str() {
                "stone_bench" => (Kind::Sit, &[-0.55, 0., 0.55], 0.05, 0.),
                "waiting_bench" => (Kind::Sit, &[-0.55, 0., 0.55], 0.08, 0.03),
                "stool" => (Kind::Sit, &[0.], 0.05, 0.01),
                "office_chair" => (Kind::Sit, &[0.], 0.08, 0.05),
                "cot" => (Kind::Sit, &[-0.45, 0.45], 0.2, 0.),
                "bunk_bed" => (Kind::Sit, &[-0.5, 0.4], 0.3, 0.),
                "air_scrubber" => (Kind::Repair, &[0.], 0.57 + 0.45, 0.),
                "air_fan" => (Kind::Repair, &[0.], 0.6 + 0.45, 0.),
                "generator" => (Kind::Repair, &[0.], 0.56 + 0.45, 0.),
                "pump_unit" => (Kind::Repair, &[0.], 0.59 + 0.45, 0.),
                "valve_station" => (Kind::Repair, &[0.], 0.33 + 0.45, 0.),
                "junction_box" => (Kind::Repair, &[0.], 0.29 + 0.45, 0.),
                "electronics_box" => (Kind::Repair, &[0.], 0.27 + 0.45, 0.),
                "control_console" => (Kind::Repair, &[0.], 0.45 + 0.45, 0.),
                "power_conduit" => (Kind::Repair, &[0.], 0.28 + 0.45, 0.),
                "wind_turbine" => (Kind::Repair, &[0.], 0.4 + 0.45, 0.),
                "floor_bell" => (Kind::Ring, &[0.], 0.95, 0.),
                _ => continue,
            };
            let yaw = p.turns as f32 * FRAC_PI_2;
            let origin = Vec3::from(p.at) + Vec3::Y * stand_on(kit, p);
            for x in xs {
                let at = origin + Quat::from_rotation_y(yaw) * Vec3::new(*x, y, z);
                out.cells.entry(cell(at)).or_default().push(Spot {
                    piece: p.piece.clone(),
                    kind,
                    at,
                    facing: yaw + if kind == Kind::Sit { 0. } else { PI },
                });
            }
        }
        out
    }
    pub fn near(&self, feet: Vec3) -> Option<&Spot> {
        let (cx, cz) = cell(feet);
        let mut best = None;
        let mut distance = USE_REACH;
        for dx in -1..=1 {
            for dz in -1..=1 {
                if let Some(spots) = self.cells.get(&(cx + dx, cz + dz)) {
                    for spot in spots {
                        let floor = spot.approach();
                        let dy = floor.y - feet.y;
                        let d = floor.xz().distance(feet.xz());
                        if (-0.6..=0.9).contains(&dy) && d < distance {
                            best = Some(spot);
                            distance = d;
                        }
                    }
                }
            }
        }
        best
    }
}
#[derive(Component, Clone, Debug)]
pub struct FurnitureSeat {
    pub spot: Spot,
    pub since: f32,
    pub getting_up: f32,
}
#[derive(Message)]
pub struct UseSound {
    pub at: Vec3,
    pub name: &'static str,
    pub volume: f32,
    pub pitch: f32,
}
pub(super) fn load(
    mut commands: Commands,
    game: Res<GameAssets>,
    kits: Res<Assets<KitAsset>>,
    layouts: Res<Assets<LayoutAsset>>,
    uses: Option<Res<Uses>>,
) {
    if uses.is_some() {
        return;
    }
    if let (Some(kit), Some(layout)) = (kits.get(&game.kit), layouts.get(&game.layout)) {
        commands.insert_resource(Uses::new(&kit.0, &layout.0.pieces));
    }
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn interact(
    mut commands: Commands,
    uses: Option<Res<Uses>>,
    controls: Res<Controls>,
    menu: Res<Menu>,
    driving: Res<Driving>,
    time: Res<Time>,
    mut players: Query<
        (
            Entity,
            &mut Intent,
            &Transform,
            &CharacterController,
            &Traversal,
            &Children,
        ),
        (With<Player>, Without<Seated>, Without<Body>),
    >,
    mut sitting: Query<
        (
            Entity,
            &mut Intent,
            &mut FurnitureSeat,
            &mut Seated,
            &Children,
        ),
        With<Player>,
    >,
    mut bodies: Query<&mut Transform, (With<Body>, Without<Player>)>,
    cars: Query<(&Drivable, &Transform), Without<Body>>,
    mut prompt: ResMut<Prompt>,
    mut sounds: MessageWriter<UseSound>,
) {
    if !controls.enabled || driving.active() || menu.screen() != Screen::Playing {
        return;
    }
    for (entity, mut intent, mut chair, mut seat, children) in &mut sitting {
        chair.since += time.delta_secs();
        if chair.getting_up > 0. {
            chair.getting_up -= time.delta_secs();
            seat.anim = Anim::SitUp;
            if chair.getting_up <= 0. {
                let centre = chair.spot.stand_up() + Vec3::Y * CAPSULE_HEIGHT / 2.;
                commands
                    .entity(entity)
                    .remove::<(Seated, FurnitureSeat)>()
                    .insert((
                        CharacterController::standing(CAPSULE_RADIUS, CAPSULE_HEIGHT, 0.3),
                        Transform::from_translation(centre),
                        MotionSamples::at(centre, CAPSULE_HEIGHT),
                        Traversal::default(),
                    ));
                for child in children {
                    if let Ok(mut tr) = bodies.get_mut(*child) {
                        tr.rotation = Quat::from_rotation_y(chair.spot.facing);
                    }
                }
            }
        } else if chair.since < SIT_DOWN {
            seat.anim = Anim::SitDown;
        } else {
            seat.anim = Anim::Sitting;
            if intent.act == Anim::Interact || intent.move_dir != Vec3::ZERO {
                chair.getting_up = GET_UP;
                seat.anim = Anim::SitUp;
                sounds.write(UseSound {
                    at: chair.spot.at,
                    name: "creak",
                    volume: 0.35,
                    pitch: 0.9,
                });
            } else if prompt.key.is_empty() {
                prompt.key = "E".into();
                prompt.text = "Get up".into();
            }
        }
        *intent = default();
    }
    let (Some(uses), Ok((entity, mut intent, tr, cc, tv, children))) = (uses, players.single_mut())
    else {
        return;
    };
    if !cc.grounded || tv.active() {
        return;
    }
    let feet = tr.translation - Vec3::Y * cc.height / 2.;
    if !prompt.key.is_empty()
        || cars
            .iter()
            .any(|(d, t)| d.driver.is_none() && t.translation.distance(feet) < REACH + 2.)
    {
        return;
    }
    if intent.hold == Anim::Fix {
        prompt.key = "Move".into();
        prompt.text = "Stop working".into();
        return;
    }
    let Some(spot) = uses.near(feet) else {
        return;
    };
    prompt.key = "E".into();
    prompt.text = match spot.kind {
        Kind::Sit => "Sit down",
        Kind::Repair => "Work on it",
        Kind::Ring => "Ring the bell",
    }
    .into();
    if intent.act != Anim::Interact {
        return;
    }
    intent.act = Anim::Idle;
    match spot.kind {
        Kind::Sit => {
            commands
                .entity(entity)
                .remove::<(
                    CharacterController,
                    ControllerState,
                    Collider,
                    RigidBody,
                    Position,
                    Rotation,
                    LinearVelocity,
                    AngularVelocity,
                )>()
                .insert((
                    FurnitureSeat {
                        spot: spot.clone(),
                        since: 0.,
                        getting_up: 0.,
                    },
                    Seated {
                        vehicle: Entity::PLACEHOLDER,
                        anim: Anim::SitDown,
                        hidden: false,
                        hands: vec![],
                        feet: vec![],
                    },
                    Intent::default(),
                    Traversal::default(),
                ));
            sounds.write(UseSound {
                at: spot.at,
                name: "creak",
                volume: 0.5,
                pitch: 1.,
            });
        }
        Kind::Repair => {
            intent.hold = Anim::Fix;
        }
        Kind::Ring => {
            intent.act = Anim::Interact;
            sounds.write(UseSound {
                at: spot.at,
                name: "bell",
                volume: 1.,
                pitch: 1.,
            });
        }
    }
    if spot.kind != Kind::Sit {
        for child in children {
            if let Ok(mut tr) = bodies.get_mut(*child) {
                tr.rotation = Quat::from_rotation_y(spot.facing);
            }
        }
    }
}
pub(super) fn sounds(
    mut events: MessageReader<UseSound>,
    cameras: Query<&Transform, With<GameCamera>>,
    menu: Res<Menu>,
    scape: Option<Res<Soundscape>>,
    mut cues: MessageWriter<Cue>,
) {
    let Ok(camera) = cameras.single() else {
        events.clear();
        return;
    };
    let v = voice(camera, menu.screen());
    for e in events.read() {
        let bell = e.name == "bell";
        let at = if bell {
            let Some(at) = scape.as_ref().and_then(|s| s.bell) else {
                continue;
            };
            at
        } else {
            e.at
        };
        cues.write(v.at(
            e.name,
            at,
            e.volume,
            if bell { 10. } else { 3. },
            if bell { 120. } else { 28. },
            e.pitch,
        ));
    }
}
