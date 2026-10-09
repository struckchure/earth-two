//! The people that can be spawned: character/roster.go and skin.go. The
//! Body, State and the movement components are the character module's;
//! this adds the skins, the spawn and the dressing.

use bevy::prelude::*;
use std::collections::HashMap;

use super::anim::{Anim, Clip};
use super::player::AnimationPlayer;
use super::presentation::MotionSamples;
use crate::character::{
    Body, CAPSULE_HEIGHT, CAPSULE_RADIUS, Character, CharacterController, Health, Intent, State,
    Traversal, TraversalConfig,
};

pub use crate::character::BODY_HEIGHT;

/// Model describes a character model file: a skinned glTF holding the mesh
/// and its clips, facing +Z with its feet at the origin.
#[derive(Clone, Debug, Default)]
pub struct Model {
    /// Relative to the asset root.
    pub path: String,
    pub clips: HashMap<Anim, Clip>,
    /// Scale sizes the model; 0 fits it to BODY_HEIGHT. Models made in
    /// metres can keep their own height with 1.
    pub scale: f32,
}

/// Skin is a loaded Model. The viewer keeps the handles; here a skin is its
/// path, its clip table and its scale.
#[derive(Clone, Debug, Default)]
pub struct Skin {
    pub model: String,
    pub clips: HashMap<Anim, Clip>,
    /// Scale brings the model to BODY_HEIGHT.
    pub scale: f32,
}

impl Skin {
    pub fn new(model: &str, clips: HashMap<Anim, Clip>, scale: f32) -> Skin {
        Skin {
            model: model.to_string(),
            clips,
            scale,
        }
    }

    /// Has reports whether the skin has a clip for a.
    pub fn has(&self, a: Anim) -> bool {
        self.clips.get(&a).is_some_and(|c| !c.name.is_empty())
    }

    /// clip is the clip for a, falling back from RunJump to Jump and from
    /// anything else missing to Idle.
    pub fn clip(&self, a: Anim) -> Clip {
        if let Some(c) = self.clips.get(&a)
            && !c.name.is_empty()
        {
            return c.clone();
        }
        match a {
            Anim::RunJump => self.clip(Anim::Jump),
            Anim::WallKickRight => self.clip(Anim::WallKick),
            Anim::PunchRight => self.clip(Anim::Punch),
            Anim::Fall => self.clip(Anim::Jump),
            Anim::WallFallRight => self.clip(Anim::WallFall),
            Anim::WallFall => self.clip(Anim::Jump),
            _ => self.clips.get(&Anim::Idle).cloned().unwrap_or_default(),
        }
    }
}

/// Roster is a resource with the characters that can be spawned.
#[derive(Resource, Clone, Debug, Default)]
pub struct Roster {
    pub skins: Vec<Skin>,
}

/// action is the one-shot a body plays when asked for act on skin: a punch
/// is with whichever hand's turn it is, the left if the skin has no right.
pub fn action(st: &State, act: Anim, skin: &Skin) -> Anim {
    if act == Anim::Punch && st.right_punch && skin.has(Anim::PunchRight) {
        return Anim::PunchRight;
    }
    act
}

/// Wear dresses a body in skin, starting it idle.
pub fn wear(
    roster: &Roster,
    st: &mut State,
    p: &mut AnimationPlayer,
    tr: &mut Transform,
    skin: usize,
) {
    let s = &roster.skins[skin];
    st.skin = skin;
    st.current = Anim::Idle;
    *p = AnimationPlayer::new(&s.model);
    p.play(&s.clip(Anim::Idle).name);
    tr.scale = Vec3::splat(s.scale);
}

impl Roster {
    /// Spawn adds a character wearing skin (wrapped to the roster's size),
    /// standing with its feet at feet: a root with the movement components,
    /// MotionSamples and a Transform at the capsule's centre, and a Body
    /// child with the State, the player and the model's scale. The phase
    /// (0 to 1) puts its idle off by up to two seconds, so crowds don't breathe in
    /// step. Returns the root; add your own components to it afterwards.
    pub fn spawn(&self, commands: &mut Commands, skin: usize, feet: Vec3, phase: f32) -> Entity {
        let n = self.skins.len();
        let skin = skin % n.max(1);
        let s = &self.skins[skin];
        let mut player = AnimationPlayer::new(&s.model);
        player.play(&s.clip(Anim::Idle).name);
        player.seek(phase * 2.0);

        let center = feet + Vec3::Y * (CAPSULE_HEIGHT / 2.0);
        let root = commands
            .spawn((
                Character::default(),
                Health::default(),
                MotionSamples::at(center, CAPSULE_HEIGHT),
                Intent::default(),
                Traversal::default(),
                TraversalConfig::default(),
                CharacterController::standing(CAPSULE_RADIUS, CAPSULE_HEIGHT, 0.3),
                Transform::from_translation(center),
            ))
            .with_child((
                Body,
                State {
                    skin,
                    ..Default::default()
                },
                player,
                Transform::from_xyz(0.0, -CAPSULE_HEIGHT / 2.0, 0.0)
                    .with_scale(Vec3::splat(s.scale)),
            ))
            .id();
        #[cfg(feature = "viewer")]
        commands.entity(root).insert(Visibility::Inherited);
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clips(idle: &str) -> HashMap<Anim, Clip> {
        HashMap::from([(Anim::Idle, Clip::named(idle))])
    }

    // character/skin_test.go TestWear.
    #[test]
    fn wear_dresses_the_body() {
        let r = Roster {
            skins: vec![
                Skin::new("a", clips("a-idle"), 1.0),
                Skin::new("b", clips("b-idle"), 0.5),
            ],
        };
        let mut st = State {
            current: Anim::Run,
            ..Default::default()
        };
        let mut p = AnimationPlayer {
            paused: true,
            ..Default::default()
        };
        let mut tr = Transform::IDENTITY;
        wear(&r, &mut st, &mut p, &mut tr, 1);
        assert!(st.skin == 1 && st.current == Anim::Idle);
        assert!(p.clip() == "b-idle" && !p.paused);
        assert_eq!(tr.scale, Vec3::splat(0.5));
    }

    // character/animate_test.go TestSkinClipFallbacks.
    #[test]
    fn clip_fallbacks() {
        let s = Skin::new(
            "m",
            HashMap::from([
                (Anim::Idle, Clip::named("idle")),
                (
                    Anim::Jump,
                    Clip {
                        name: "jump".into(),
                        start: 0.3,
                        ..Default::default()
                    },
                ),
            ]),
            1.0,
        );
        let c = s.clip(Anim::RunJump);
        assert!(c.name == "jump" && c.start == 0.3);
        assert_eq!(s.clip(Anim::Walk).name, "idle");
        assert!(!s.has(Anim::Punch));
    }

    // TestPunchesAlternate.
    #[test]
    fn punches_alternate() {
        let both = Skin::new(
            "m",
            HashMap::from([
                (Anim::Punch, Clip::named("left")),
                (Anim::PunchRight, Clip::named("right")),
            ]),
            1.0,
        );
        let mut st = State::default();
        assert_eq!(action(&st, Anim::Punch, &both), Anim::Punch);
        st.right_punch = true;
        assert_eq!(action(&st, Anim::Punch, &both), Anim::PunchRight);
        let left = Skin::new(
            "m",
            HashMap::from([(Anim::Punch, Clip::named("left"))]),
            1.0,
        );
        assert_eq!(action(&st, Anim::Punch, &left), Anim::Punch);
        assert_eq!(action(&st, Anim::PickUp, &both), Anim::PickUp);
    }
}
