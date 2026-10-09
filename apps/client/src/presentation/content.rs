//! The game's people: game/game.go's clip table, model list and starting
//! outfit, as data.

use std::collections::HashMap;

use super::anim::{Anim, Clip};
use super::outfit::{Outfit, Slot, Wardrobe};
use super::roster::Model;

/// STAIR_STEP and STAIR_DOWN_STEP are where in the stair clips' cycles a
/// foot lands on a step's edge (tools/makehuman/traversal.py): going up, the
/// left foot lands 0.175 of the way in, 0.22 m above the ground under the
/// body; coming down, 5/28 of the way in, 0.209 m below it.
pub const STAIR_STEP: f32 = 0.54;
pub const STAIR_DOWN_STEP: f32 = 0.527;

/// makehuman is the clip table for the people tools/makehuman builds, all
/// retargeted onto MakeHuman's game engine rig: Mixamo's motion capture for
/// the everyday moves, and Quaternius's Universal Animation Library for the
/// rest. Jumps run from take-off to touchdown, timed from frame strips.
pub fn makehuman() -> HashMap<Anim, Clip> {
    let named = |a: Anim, name: &str| (a, Clip::named(name));
    HashMap::from([
        named(Anim::Idle, "Breathing Idle"),
        named(Anim::Walk, "Walking"),
        named(Anim::Run, "Running"),
        (
            Anim::Jump,
            Clip {
                name: "Jumping".into(),
                start: 0.65,
                land: 1.05,
                ..Default::default()
            },
        ),
        (
            Anim::RunJump,
            Clip {
                name: "Running Jump".into(),
                start: 0.13,
                land: 0.62,
                ..Default::default()
            },
        ),
        named(Anim::Interact, "Interact"),
        named(Anim::Punch, "Punching"),
        named(Anim::PunchRight, "Punching Mirrored"),
        named(Anim::PickUp, "Picking Up"),
        named(Anim::Slide, "Traversal_Slide"),
        named(Anim::Roll, "Roll"),
        named(Anim::LadderClimb, "Traversal_Ladder"),
        named(Anim::Vault, "Traversal_Vault"),
        named(Anim::Mantle, "Traversal_Mantle"),
        named(Anim::WallKick, "Traversal_WallKick"),
        named(Anim::WallKickRight, "Traversal_WallKickRight"),
        named(Anim::WallFall, "Traversal_WallKickFall"),
        named(Anim::WallFallRight, "Traversal_WallKickFallRight"),
        named(Anim::WallLand, "Traversal_WallLand"),
        named(Anim::Crouch, "Traversal_CrouchIdle"), // Mixamo's Crouching Idle
        named(Anim::StandUp, "Traversal_StandUp"),
        named(Anim::LadderExit, "Traversal_LadderExit"),
        named(Anim::LadderEnter, "Traversal_Ladder"),
        named(Anim::Fall, "Traversal_Fall"),
        // Mixamo's Ascending and Descending Stairs, their feet put on the
        // kit's steps.
        (
            Anim::StairsUp,
            Clip {
                name: "Traversal_StairsUp".into(),
                step: STAIR_STEP,
                ..Default::default()
            },
        ),
        (
            Anim::StairsDown,
            Clip {
                name: "Traversal_StairsDown".into(),
                step: STAIR_DOWN_STEP,
                ..Default::default()
            },
        ),
        // Mixamo's Crouched Walking, a little lower to pass under the ducts.
        named(Anim::CrouchWalk, "Traversal_CrouchWalk"),
        // In a vehicle's seat (Ride falls back to it): the library's driving
        // loop.
        named(Anim::Drive, "Driving_Loop"),
        // On a bench, a stool or a bunk, at a machine, and the emotes: the
        // library's.
        named(Anim::SitDown, "Sitting_Enter"),
        named(Anim::Sitting, "Sitting_Idle_Loop"),
        named(Anim::SitUp, "Sitting_Exit"),
        named(Anim::Fix, "Fixing_Kneeling"),
        named(Anim::Talk, "Idle_Talking_Loop"),
        named(Anim::Dance, "Dance_Loop"),
    ])
}

/// people are the character models in assets/characters (see CREDITS.txt).
/// They're made in metres, so they keep their own heights.
pub fn people() -> Vec<Model> {
    ["characters/man.glb", "characters/woman.glb"]
        .into_iter()
        .map(|path| Model {
            path: path.into(),
            clips: makehuman(),
            scale: 1.0,
        })
        .collect()
}

/// The wardrobe file, relative to the asset root.
pub const WARDROBE: &str = "characters/wardrobe.json";

/// starting_outfit is what the player first wears: whichever of these the
/// first body's wardrobe has.
pub fn starting_outfit(w: &Wardrobe) -> Outfit {
    let mut o = Outfit::default();
    if w.bodies.is_empty() {
        return o;
    }
    for (slot, name) in [
        (Slot::Hair, "Short"),
        (Slot::Top, "T-shirt"),
        (Slot::Bottom, "Cargo pants"),
        (Slot::Shoes, "White trainers"),
    ] {
        if let Some(i) = w.find(0, slot, name) {
            o.put(slot, i);
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_clip_is_in_the_glb() {
        // The clip names the GLBs carry (assets/characters/man.glb).
        let table = makehuman();
        assert_eq!(table.len(), 34);
        assert_eq!(table[&Anim::Jump].start, 0.65);
        assert_eq!(table[&Anim::StairsDown].step, STAIR_DOWN_STEP);
        assert_eq!(people().len(), 2);
    }
}
