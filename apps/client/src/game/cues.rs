//! State-driven one-shot cues from game/cues.go. Particle emission is separate.
use super::{
    GameSet, Screen,
    camera::GameCamera,
    sound::{Listener, MENU_DUCK},
};
use crate::{
    character::{Anim, Body, Character, CharacterController, CharacterPhysics, State as BodyState},
    landfall::{
        surface::{Footing, Soundscape},
        terrain::TerrainBody,
    },
};
use bevy::{prelude::*, transform::TransformSystems};
use std::collections::HashMap;

#[derive(Message, Clone, Debug)]
pub struct Cue {
    pub name: &'static str,
    pub volume: f32,
    pub pan: f32,
    pub pitch: f32,
    pub jitter: f32,
}
#[derive(Clone, Copy)]
pub struct Voice {
    pub ear: Listener,
    pub duck: f32,
}
impl Voice {
    pub fn at(
        self,
        name: &'static str,
        at: Vec3,
        volume: f32,
        near: f32,
        far: f32,
        pitch: f32,
    ) -> Cue {
        let (gain, pan) = self.ear.spatial(at, near, far);
        Cue {
            name,
            volume: volume * gain * 0.8 * self.duck,
            pan,
            pitch,
            jitter: 0.06,
        }
    }
    pub fn ui(name: &'static str, volume: f32) -> Cue {
        Cue {
            name,
            volume: volume * 0.5,
            pan: 0.,
            pitch: 1.,
            jitter: 0.03,
        }
    }
}
#[derive(Resource)]
pub struct CueRandom(u64);
impl Default for CueRandom {
    fn default() -> Self {
        Self(crate::sky::time::UnixTime::now().0 as u64 | 1)
    }
}
impl CueRandom {
    pub fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / 16777216.
    }
}
pub(super) fn voice(camera: &Transform, screen: Screen) -> Voice {
    Voice {
        ear: Listener {
            at: camera.translation,
            right: *camera.right(),
            active: true,
        },
        duck: if screen == Screen::Playing {
            1.
        } else {
            MENU_DUCK
        },
    }
}
#[derive(SystemSet, Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct CueSet;
pub struct CuesPlugin;
impl Plugin for CuesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Cue>()
            .init_resource::<CueRandom>()
            .init_resource::<super::drive_sound::DriveSound>()
            .init_resource::<super::ui_sound::UiSoundState>()
            .add_systems(
                Update,
                (
                    body_cues,
                    super::drive_sound::drive_cues,
                    super::ui_sound::ui_cues,
                )
                    .chain()
                    .after(GameSet::Camera),
            )
            .add_systems(
                PostUpdate,
                foot_cues.after(TransformSystems::Propagate).in_set(CueSet),
            )
            .add_systems(
                FixedPostUpdate,
                super::drive_sound::crash_cues.after(avian3d::prelude::PhysicsSystems::Writeback),
            );
    }
}
#[derive(Clone, Copy)]
pub struct BodyMemory {
    anim: Anim,
    grounded: bool,
    fall: f32,
    clank: f32,
}
impl BodyMemory {
    pub fn new(anim: Anim, grounded: bool) -> Self {
        Self {
            anim,
            grounded,
            fall: 0.,
            clank: 0.,
        }
    }
    pub fn update(
        &mut self,
        cur: Anim,
        cc: &CharacterController,
        under: Footing,
        dt: f32,
        random: &mut CueRandom,
    ) -> Vec<(&'static str, f32, f32)> {
        let mut out = Vec::new();
        if !cc.grounded && cc.velocity.y < 0. {
            self.fall = self.fall.max(-cc.velocity.y);
        }
        if self.grounded && !cc.grounded && cc.velocity.y > 1. && !cc.controlled {
            out.push(("cloth", 0.5, 1.));
        }
        if !self.grounded && cc.grounded && !cc.controlled && self.fall > 2.5 {
            let k = ((self.fall - 2.5) / 6.).clamp(0., 1.);
            let (name, pitch) = under.step();
            out.push((name, 0.6 + 0.4 * k, pitch * 0.9));
            out.push((
                if under.loose() {
                    "land_loose"
                } else {
                    "land_hard"
                },
                if under.loose() { 0.3 } else { 0.25 } + 0.6 * k,
                1.,
            ));
        }
        if cc.grounded {
            self.fall = 0.;
        }
        if cur != self.anim {
            match cur {
                Anim::Roll => {
                    out.push(("whoosh", 0.5, 0.8));
                    out.push(("cloth", 0.6, 0.9));
                }
                Anim::Slide => out.push(("slide", 0.8, 1.)),
                Anim::WallKick | Anim::WallKickRight => {
                    out.push(("thump", 0.7, 1.));
                    out.push(("cloth", 0.5, 1.1));
                }
                Anim::Vault | Anim::Mantle | Anim::LadderEnter => {
                    out.push(("grab", 0.7, 1.));
                    out.push(("cloth", 0.4, 1.));
                }
                Anim::Punch | Anim::PunchRight => out.push(("whoosh", 0.45, 1.25)),
                Anim::Interact | Anim::PickUp => out.push(("cloth", 0.4, 1.1)),
                Anim::SitDown => out.push(("creak", 0.5, 1.)),
                Anim::SitUp => out.push(("creak", 0.35, 0.9)),
                Anim::Fix => self.clank = 0.4,
                _ => {}
            }
        }
        if cur == Anim::Fix {
            self.clank -= dt;
            if self.clank <= 0. {
                out.push(("clank", 0.5, 1.));
                self.clank = 0.7 + 1.1 * random.unit();
            }
        }
        self.anim = cur;
        self.grounded = cc.grounded;
        out
    }
}
fn underfoot(
    scape: &Soundscape,
    physics: &CharacterPhysics,
    ground: &Query<(), With<TerrainBody>>,
    root: Entity,
    feet: Vec3,
) -> Footing {
    let on_kit = physics
        .cast_ray_excluding(feet + Vec3::Y * 0.3, Vec3::NEG_Y, 0.8, root)
        .is_some_and(|h| !ground.contains(h.entity) && !ground.contains(physics.body_of(h.entity)));
    scape.surface_at(feet, on_kit)
}
#[allow(clippy::too_many_arguments)]
fn body_cues(
    cameras: Query<&Transform, With<GameCamera>>,
    screen: Res<State<Screen>>,
    time: Res<Time>,
    roots: Query<(&Transform, &CharacterController), With<Character>>,
    bodies: Query<(Entity, &ChildOf, &BodyState), With<Body>>,
    scape: Option<Res<Soundscape>>,
    physics: CharacterPhysics,
    ground: Query<(), With<TerrainBody>>,
    mut memory: Local<HashMap<Entity, BodyMemory>>,
    mut random: ResMut<CueRandom>,
    mut cues: MessageWriter<Cue>,
) {
    let (Ok(camera), Some(scape)) = (cameras.single(), scape) else {
        return;
    };
    let v = voice(camera, *screen.get());
    memory.retain(|e, _| bodies.contains(*e));
    for (e, parent, st) in &bodies {
        let Ok((tr, cc)) = roots.get(parent.parent()) else {
            continue;
        };
        let feet = tr.translation - Vec3::Y * cc.height / 2.;
        let m = memory
            .entry(e)
            .or_insert_with(|| BodyMemory::new(st.current, cc.grounded));
        let under = underfoot(&scape, &physics, &ground, parent.parent(), feet);
        for (name, volume, pitch) in m.update(st.current, cc, under, time.delta_secs(), &mut random)
        {
            cues.write(v.at(name, feet + Vec3::Y, volume, 3., 28., pitch));
        }
    }
}
#[derive(Default, Clone, Copy, Debug)]
pub struct Foot {
    low: f32,
    since: f32,
    lifted: bool,
    ready: bool,
}
impl Foot {
    pub fn track(&mut self, h: f32, dt: f32) -> bool {
        self.since += dt;
        if !self.ready {
            self.low = h;
            self.ready = true;
            return false;
        }
        self.low = h.min(self.low + 0.05 * dt);
        if h > self.low + 0.06 {
            self.lifted = true;
        }
        if !self.lifted || h > self.low + 0.035 {
            return false;
        }
        self.lifted = false;
        if self.since < 0.22 {
            return false;
        }
        self.since = 0.;
        true
    }
}
pub fn stepping(a: Anim) -> bool {
    !a.airborne()
        && !a.one_shot()
        && !a.held()
        && !matches!(
            a,
            Anim::Slide
                | Anim::Roll
                | Anim::LadderClimb
                | Anim::LadderEnter
                | Anim::LadderExit
                | Anim::Vault
                | Anim::Mantle
                | Anim::WallKick
                | Anim::WallKickRight
                | Anim::WallFall
                | Anim::WallFallRight
                | Anim::WallLand
                | Anim::Drive
                | Anim::Ride
                | Anim::SitDown
                | Anim::Sitting
                | Anim::SitUp
        )
}
/// Live GLTF joints, resolved on the body's own skin, never garment skeletons.
#[derive(Component)]
pub struct AudibleFeet(pub [Entity; 2]);
struct FootMemory {
    bones: [Entity; 2],
    feet: [Foot; 2],
}
#[allow(clippy::too_many_arguments)]
fn foot_cues(
    cameras: Query<&Transform, With<GameCamera>>,
    screen: Res<State<Screen>>,
    time: Res<Time>,
    roots: Query<(&Transform, &CharacterController), With<Character>>,
    bodies: Query<(Entity, &ChildOf, &BodyState, &AudibleFeet), With<Body>>,
    joints: Query<&GlobalTransform>,
    scape: Option<Res<Soundscape>>,
    physics: CharacterPhysics,
    ground: Query<(), With<TerrainBody>>,
    mut memory: Local<HashMap<Entity, FootMemory>>,
    mut cues: MessageWriter<Cue>,
) {
    let (Ok(camera), Some(scape)) = (cameras.single(), scape) else {
        return;
    };
    let v = voice(camera, *screen.get());
    memory.retain(|e, _| bodies.contains(*e));
    for (body, parent, st, bones) in &bodies {
        let Ok((tr, cc)) = roots.get(parent.parent()) else {
            continue;
        };
        let feet = tr.translation - Vec3::Y * cc.height / 2.;
        let entry = memory.entry(body).or_insert(FootMemory {
            bones: bones.0,
            feet: [Foot::default(); 2],
        });
        if entry.bones != bones.0 {
            *entry = FootMemory {
                bones: bones.0,
                feet: [Foot::default(); 2],
            };
        }
        let speed = cc.velocity.xz().length();
        for (bone, f) in bones.0.iter().zip(&mut entry.feet) {
            let Ok(g) = joints.get(*bone) else {
                continue;
            };
            if f.track(g.translation().y - feet.y, time.delta_secs())
                && cc.grounded
                && !cc.controlled
                && stepping(st.current)
                && speed > 0.3
            {
                let (name, pitch) =
                    underfoot(&scape, &physics, &ground, parent.parent(), feet).step();
                let mut volume = 0.35 + 0.5 * ((speed - 1.) / 4.).clamp(0., 1.);
                if cc.height < 1.5 {
                    volume *= 0.5;
                }
                cues.write(v.at(name, g.translation(), volume, 3., 28., pitch));
            }
        }
    }
}
