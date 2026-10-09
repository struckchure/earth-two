//! Character presentation: how the people of Earth Two look and move on
//! screen. A port of the Go `character` package's animate, skin, outfit,
//! footwear, appearance, roster, family, presentation, distant and cloth
//! files, and the illusion render pieces they lean on (animation, bones,
//! pose, skinning, cloth).
//!
//! The movement side (locomotion, traversal, the controller) is
//! `crate::character`'s; this module reads its `Character`, `Intent`,
//! `CharacterController`, `Traversal`, `Body` and `State`. Everything that
//! needs no GPU compiles without the `viewer` feature and is tested
//! headless; loading the GLBs, the skin and garment materials and the
//! cloth's mesh writeback live in `viewer.rs`.

pub mod anim;
pub mod animate;
pub mod appearance;
pub mod bones;
pub mod cloth;
pub mod content;
pub mod distant;
pub mod family;
pub mod footwear;
pub mod graph;
pub mod mesh;
pub mod outfit;
pub mod player;
// Keep the source-module name for the Go presentation.go parity port.
#[allow(clippy::module_inception)]
pub mod presentation;
pub mod roster;
pub mod verlet;
#[cfg(feature = "viewer")]
pub mod viewer;

use avian3d::prelude::{Physics, PhysicsSystems, PhysicsTime};
use bevy::app::AnimationSystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;

pub use anim::{Anim, Clip};
pub use outfit::{Outfit, Slot, Wardrobe};
pub use player::{AnimationFinished, AnimationPlayer, ClipLibrary, Libraries};
pub use presentation::MotionSamples;
pub use roster::{Model, Roster, Skin};

use crate::character::CharacterSystems;

/// Paused reports whether physics is paused: the Go physics.Settings.Paused,
/// read from Avian's clock when it's there.
pub fn paused(physics: Option<&Time<Physics>>) -> bool {
    physics.is_some_and(|t| t.is_paused())
}

/// The sets this module's systems run in, for others to order against.
#[derive(SystemSet, Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum PresentationSystems {
    /// Update: loaded model meshes and garment joint bindings, before material adoption.
    Models,
    /// Update, in the character module's Act set: dressing, clothing and
    /// animating, after the body has turned to face its way.
    Act,
    /// PostUpdate: the body's interpolated position and the clip clocks,
    /// before Bevy samples the pose.
    Advance,
    /// PostUpdate: pose modifiers between the clip clocks and Bevy's
    /// sampling (render.Animate between AdvanceAnimations and AttachBones).
    Pose,
}

/// Plugin runs the presentation systems. It needs the character plugin
/// (for its resources and sets) and a roster and wardrobe, which the viewer
/// loads (or a test inserts).
#[derive(Default)]
pub struct PresentationPlugin {
    /// DisableCloth keeps clothes animated by their skeleton without the
    /// per-vertex cloth solver, for platforms with a tight frame budget: the
    /// browser.
    pub disable_cloth: bool,
}

impl PresentationPlugin {
    /// The platform's default: desktop simulates cloth, the browser doesn't.
    pub fn platform() -> PresentationPlugin {
        PresentationPlugin {
            disable_cloth: cfg!(target_arch = "wasm32"),
        }
    }
}

/// ClothEnabled is a resource: whether garments get the cloth solver.
#[derive(Resource, Clone, Copy, Debug)]
pub struct ClothEnabled(pub bool);

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy::animation::AnimationPlugin>() {
            app.add_plugins(bevy::animation::AnimationPlugin);
        }
        app.init_resource::<crate::character::Controls>()
            .init_resource::<family::Family>()
            .init_resource::<Libraries>()
            .init_resource::<Roster>()
            .init_resource::<Wardrobe>()
            .init_resource::<graph::Graphs>()
            .init_resource::<mesh::ModelStore>()
            .insert_resource(ClothEnabled(!self.disable_cloth))
            .add_message::<AnimationFinished>()
            .add_systems(PreUpdate, family::index_family)
            .add_systems(
                FixedPostUpdate,
                presentation::remember_motion.after(PhysicsSystems::Writeback),
            )
            .configure_sets(
                Update,
                PresentationSystems::Act
                    .in_set(CharacterSystems::Act)
                    .after(crate::character::locomotion::face),
            )
            .add_systems(
                Update,
                (outfit::dress, cloth::clothe, animate::animate)
                    .chain()
                    .in_set(PresentationSystems::Act),
            )
            .configure_sets(
                PostUpdate,
                (PresentationSystems::Advance, PresentationSystems::Pose)
                    .chain()
                    .before(AnimationSystems)
                    .before(TransformSystems::Propagate),
            )
            .add_systems(
                PostUpdate,
                (presentation::present_motion, player::advance_animations)
                    .chain()
                    .in_set(PresentationSystems::Advance),
            )
            .add_systems(
                PostUpdate,
                (
                    distant::lockstep,
                    outfit::mirror_pose,
                    bones::attach_to_bones,
                    graph::apply_players,
                )
                    .chain()
                    .in_set(PresentationSystems::Pose),
            );
        #[cfg(feature = "viewer")]
        app.add_plugins(viewer::ViewerPlugin);
    }
}
