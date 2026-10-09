//! Showing the physics position between fixed steps: character/presentation.go.

use bevy::prelude::*;

use super::anim::clamp;
use crate::character::{Body, CharacterController};

/// MotionSamples keeps the last two physics positions for visual
/// interpolation. The root and controller keep their authoritative physics
/// position.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct MotionSamples {
    pub(crate) previous: Vec3,
    pub(crate) current: Vec3,
    height: f32,
    /// How many fixed steps apart previous and current were (more than one
    /// for a character stepped only every so often), and how many have
    /// passed since current.
    span: u32,
    since: u32,
}

impl MotionSamples {
    pub fn at(center: Vec3, height: f32) -> MotionSamples {
        MotionSamples {
            previous: center,
            current: center,
            height,
            ..Default::default()
        }
    }

    pub fn new(previous: Vec3, current: Vec3) -> MotionSamples {
        MotionSamples {
            previous,
            current,
            ..Default::default()
        }
    }

    pub fn current(&self) -> Vec3 {
        self.current
    }

    /// Position returns the position to draw at between physics steps. An
    /// external move or a teleport is shown immediately instead of streaking
    /// across the map.
    pub fn position(&self, current: Vec3, alpha: f32) -> Vec3 {
        if current != self.current || self.previous.distance_squared(self.current) > 1.0 {
            return current;
        }
        let t = clamp(
            (self.since as f32 + alpha) / self.span.max(1) as f32,
            0.0,
            1.0,
        );
        self.previous.lerp(self.current, t)
    }

    /// Speed is how fast the character really went over the ground when
    /// last stepped, physics steps being dt seconds: nothing, for one walking
    /// into a wall.
    pub fn speed(&self, dt: f32) -> f32 {
        if dt <= 0.0 {
            return 0.0;
        }
        (self.current.x - self.previous.x).hypot(self.current.z - self.previous.z)
            / (dt * self.span.max(1) as f32)
    }

    /// remember takes the position after a fixed step, if physics stepped
    /// the character.
    pub fn remember(&mut self, position: Vec3, height: f32, stepped: bool) {
        if !stepped {
            self.since += 1;
            return;
        }
        // Resizing moves the capsule centre, not its feet. Put the older
        // sample into the new centre convention before interpolating.
        if self.height > 0.0 {
            self.current.y += (height - self.height) / 2.0;
        }
        self.previous = self.current;
        self.current = position;
        self.height = height;
        self.span = self.since + 1;
        self.since = 0;
    }
}

/// remember_motion runs in FixedPostUpdate after the physics writeback.
pub fn remember_motion(mut q: Query<(&Transform, &mut MotionSamples, &CharacterController)>) {
    for (tr, mut s, cc) in &mut q {
        s.remember(tr.translation, cc.height, cc.stepped || cc.every <= 1);
    }
}

/// present_motion puts each body where its root was between the last two
/// fixed steps, feet at the bottom of the capsule.
pub fn present_motion(
    mut bodies: Query<(&ChildOf, &mut Transform), With<Body>>,
    roots: Query<(&Transform, &MotionSamples, &CharacterController), Without<Body>>,
    fixed: Res<Time<Fixed>>,
) {
    let alpha = fixed.overstep_fraction();
    for (parent, mut tr) in &mut bodies {
        let Ok((root, samples, cc)) = roots.get(parent.parent()) else {
            continue;
        };
        let offset = samples.position(root.translation, alpha) - root.translation;
        tr.translation = Vec3::new(0.0, -cc.height / 2.0, 0.0) + offset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // character/presentation_test.go TestMotionSamplesPosition.
    #[test]
    fn position_interpolates_and_shows_teleports_at_once() {
        let mut s = MotionSamples::new(Vec3::new(1.0, 2.0, 0.0), Vec3::new(1.2, 2.1, 0.0));
        for alpha in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let got = s.position(s.current, alpha);
            let want = s.previous.lerp(s.current, alpha);
            assert!(
                got.distance(want) <= 1e-5,
                "alpha {alpha}: {got} want {want}"
            );
        }
        assert_eq!(s.current, Vec3::new(1.2, 2.1, 0.0));
        let respawn = Vec3::new(0.0, 2.0, 0.0);
        assert_eq!(s.position(respawn, 0.5), respawn);
        s.current = Vec3::new(20.0, 0.0, 0.0);
        assert_eq!(s.position(s.current, 0.5), s.current);
    }

    // TestCapsuleResizeInterpolationKeepsFeetPlanted.
    #[test]
    fn resize_keeps_feet_planted() {
        let mut s = MotionSamples::at(Vec3::new(0.0, 0.9, 0.0), 1.8);
        for height in [0.9f32, 1.8] {
            let center = Vec3::new(0.0, height / 2.0, 0.0);
            s.remember(center, height, true);
            for alpha in [0.0, 0.5, 1.0] {
                let y = s.position(center, alpha).y - height / 2.0;
                assert!(
                    y.abs() <= 0.001,
                    "resize to {height} at alpha {alpha} lifted feet by {y}"
                );
            }
        }
    }
}
