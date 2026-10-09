//! Handing the clip clock's pose to Bevy: each model's clips as nodes of an
//! AnimationGraph, and a system that sets Bevy's AnimationPlayer to the
//! current clip at its time, crossfaded from the previous one by the
//! blend, as raylib's UpdateModelAnimationEx did for the Go code.

use bevy::animation::graph::{AnimationGraphHandle, AnimationNodeIndex};
use bevy::animation::{AnimationPlayer as BevyPlayer, RepeatAnimation};
use bevy::prelude::*;
use std::collections::HashMap;

use super::player::{AnimationPlayer, Libraries};

/// ClipGraph is one model's clips in a graph: a node for each, all under
/// the root, weighted per frame.
#[derive(Clone, Debug)]
pub struct ClipGraph {
    pub handle: Handle<AnimationGraph>,
    pub nodes: HashMap<String, AnimationNodeIndex>,
}

/// Graphs is a resource: the graph for each model path. The viewer builds
/// one from each GLB's named clips.
#[derive(Resource, Default)]
pub struct Graphs {
    pub by_model: HashMap<String, ClipGraph>,
}

impl Graphs {
    /// build makes model's graph from its named clips.
    pub fn build(
        &mut self,
        model: &str,
        clips: impl IntoIterator<Item = (String, Handle<AnimationClip>)>,
        graphs: &mut Assets<AnimationGraph>,
    ) -> &ClipGraph {
        let mut graph = AnimationGraph::new();
        let mut nodes = HashMap::new();
        for (name, clip) in clips {
            let node = graph.add_clip(clip, 1.0, graph.root);
            nodes.insert(name, node);
        }
        self.by_model.entry(model.to_string()).or_insert(ClipGraph {
            handle: graphs.add(graph),
            nodes,
        })
    }

    pub fn get(&self, model: &str) -> Option<&ClipGraph> {
        self.by_model.get(model)
    }
}

/// Animated links an entity with a clip clock to the entity Bevy animates
/// (the armature root of its spawned scene, which carries Bevy's
/// AnimationPlayer). The viewer sets it once the scene is in.
#[derive(Component, Clone, Copy, Debug)]
pub struct Animated(pub Entity);

/// The frame a once-clip holds past its end, as raylib held the keyframe
/// before a looped clip's final copy of its first: a sixtieth of a second
/// short of the duration.
pub const HOLD_BACK: f32 = 1.0 / 60.0;

/// What one clip gets on Bevy's player: its node, where it is, how much of
/// the pose it has and whether it loops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeState {
    pub node: AnimationNodeIndex,
    pub time: f32,
    pub weight: f32,
    pub once: bool,
}

/// node_states is what the clip clock asks Bevy to play this frame: the
/// current clip, and the previous while a crossfade runs.
pub fn node_states(
    p: &AnimationPlayer,
    graph: &ClipGraph,
    durations: impl Fn(&str) -> Option<f32>,
) -> Vec<NodeState> {
    let mut out = vec![];
    let place = |clip: &str, time: f32, once: bool, weight: f32| -> Option<NodeState> {
        let node = *graph.nodes.get(clip)?;
        let duration = durations(clip).unwrap_or(0.0);
        let time = if once {
            time.min((duration - HOLD_BACK).max(0.0))
        } else if duration > 0.0 {
            time % duration
        } else {
            0.0
        };
        Some(NodeState {
            node,
            time,
            weight,
            once,
        })
    };
    let blend = p.blend();
    if let Some(prev) = p.previous()
        && blend < 1.0
        && let Some(s) = place(&prev.clip, prev.time, prev.once, 1.0 - blend)
    {
        out.push(s);
    }
    let weight = if out.is_empty() { 1.0 } else { blend };
    if let Some(s) = place(p.clip(), p.time(), p.once(), weight) {
        out.push(s);
    }
    out
}

/// apply_players sets each Bevy player to its clip clock's pose. Bevy's own
/// clocks stay paused: time is the clip clock's.
pub fn apply_players(
    mut commands: Commands,
    clocks: Query<(&AnimationPlayer, &Animated)>,
    mut players: Query<(&mut BevyPlayer, Option<&AnimationGraphHandle>)>,
    graphs: Res<Graphs>,
    libraries: Res<Libraries>,
) {
    for (clock, animated) in &clocks {
        let Ok((mut player, handle)) = players.get_mut(animated.0) else {
            continue;
        };
        let Some(graph) = graphs.get(&clock.model) else {
            continue;
        };
        if handle.is_none_or(|h| h.0 != graph.handle) {
            commands
                .entity(animated.0)
                .insert(AnimationGraphHandle(graph.handle.clone()));
        }
        let states = node_states(clock, graph, |name| {
            libraries
                .get(&clock.model)
                .and_then(|lib| lib.duration_of(name))
        });
        let keep: Vec<AnimationNodeIndex> = states.iter().map(|s| s.node).collect();
        let playing: Vec<AnimationNodeIndex> =
            player.playing_animations().map(|(n, _)| *n).collect();
        for node in playing {
            if !keep.contains(&node) {
                player.stop(node);
            }
        }
        for s in states {
            let active = player.play(s.node);
            active
                .set_weight(s.weight)
                .set_repeat(if s.once {
                    RepeatAnimation::Never
                } else {
                    RepeatAnimation::Forever
                })
                .set_speed(0.0)
                .set_seek_time(s.time)
                .pause();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> ClipGraph {
        let mut g = AnimationGraph::new();
        let walk = g.add_clip(Handle::default(), 1.0, g.root);
        let jump = g.add_clip(Handle::default(), 1.0, g.root);
        ClipGraph {
            handle: Handle::default(),
            nodes: HashMap::from([("Walk".to_string(), walk), ("Jump".to_string(), jump)]),
        }
    }

    fn durations(name: &str) -> Option<f32> {
        match name {
            "Walk" => Some(1.0),
            "Jump" => Some(0.5),
            _ => None,
        }
    }

    #[test]
    fn crossfade_weights_follow_the_blend() {
        let g = graph();
        let mut p = AnimationPlayer::default();
        p.play("Walk");
        p.play_once("Jump").fade_in(1.0);
        p.fade = 0.25;
        let s = node_states(&p, &g, durations);
        assert_eq!(s.len(), 2);
        assert_eq!(
            (s[0].node, s[0].weight, s[0].once),
            (g.nodes["Walk"], 0.75, false)
        );
        assert_eq!(
            (s[1].node, s[1].weight, s[1].once),
            (g.nodes["Jump"], 0.25, true)
        );
    }

    #[test]
    fn once_clips_hold_short_of_their_end() {
        let g = graph();
        let mut p = AnimationPlayer::default();
        p.play_once("Jump");
        p.seek(5.0);
        let s = node_states(&p, &g, durations);
        assert_eq!(s.len(), 1);
        assert!((s[0].time - (0.5 - HOLD_BACK)).abs() < 1e-6 && s[0].weight == 1.0);
    }

    #[test]
    fn unknown_clips_play_nothing() {
        let g = graph();
        let mut p = AnimationPlayer::default();
        p.play("Swim");
        assert!(node_states(&p, &g, durations).is_empty());
    }
}
