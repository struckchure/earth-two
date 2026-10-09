//! Far-off bodies posed in step: character/distant.go.

use bevy::prelude::*;

use super::player::AnimationPlayer;
use crate::character::{Body, State};

/// Distant on a character's body marks it as far off, which the game decides
/// (it knows where the camera is). A distant character is posed in step with
/// every other distant one playing the same clip, and isn't fitted to the
/// ground under its feet, so a crowd far off costs a pose for each of the
/// clips it plays, not for each person in it. At a distance nobody sees that
/// they move together.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct Distant;

/// LOCKSTEP_PHASES are the moments distant bodies are put off by, in
/// seconds: apart in any clip about a second long, as walks and runs are.
pub const LOCKSTEP_PHASES: [f32; 2] = [0.0, 0.53];

/// lockstep_time is where a distant body's clip is at elapsed seconds: the
/// time since the start, which every looping clip wraps round, put off by
/// one of LOCKSTEP_PHASES (by which body it is, so it keeps to it).
pub fn lockstep_time(elapsed: f32, entity: Entity) -> f32 {
    elapsed + LOCKSTEP_PHASES[entity.index_u32() as usize % LOCKSTEP_PHASES.len()]
}

/// lockstep poses every distant body at one of a few moments of its clip,
/// with no crossfade (it changes clips at once).
#[allow(clippy::type_complexity)]
pub fn lockstep(
    mut bodies: Query<(Entity, &mut AnimationPlayer, &State), (With<Body>, With<Distant>)>,
    clock: Res<Time>,
) {
    let t = clock.elapsed_secs();
    for (e, mut p, state) in &mut bodies {
        if state.downed {
            continue;
        }
        p.seek(lockstep_time(t, e));
        p.settle();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_spread_a_crowd() {
        let a = Entity::from_raw_u32(2).unwrap();
        let b = Entity::from_raw_u32(3).unwrap();
        assert_eq!(lockstep_time(1.0, a), 1.0);
        assert!((lockstep_time(1.0, b) - 1.53).abs() < 1e-6);
    }
}
