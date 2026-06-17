//! Lightweight animation engine.
//!
//! Manages multiple concurrent `Animation`s on a shared `Timeline`.
//! Each animation tracks a single `f32` value from `start` to `end`
//! over a `duration` with an `Easing` curve.
//!
//! Usage:
//!   1. Call `animator.animate(end, duration, easing)` → returns `AnimationId`
//!   2. On each tick: `animator.update(dt)`
//!   3. In the view: `let value = animator.get(id)` — returns the interpolated value.

use std::collections::HashMap;
use std::time::Duration;

use crate::ui::easing::Easing;

pub type AnimationId = u64;

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Idle,
    Running,
    Finished,
}

#[derive(Debug, Clone)]
struct Animation {
    id: AnimationId,
    start: f32,
    end: f32,
    duration: Duration,
    elapsed: Duration,
    easing: Easing,
    state: State,
}

impl Animation {
    fn new(id: AnimationId, from: f32, to: f32, duration: Duration, easing: Easing) -> Self {
        Self {
            id,
            start: from,
            end: to,
            duration,
            elapsed: Duration::ZERO,
            easing,
            state: State::Running,
        }
    }

    fn value(&self) -> f32 {
        match self.state {
            State::Idle => self.start,
            State::Finished => self.end,
            State::Running => {
                let t = if self.duration.is_zero() {
                    1.0
                } else {
                    let t = self.elapsed.as_secs_f32() / self.duration.as_secs_f32();
                    t.min(1.0)
                };
                let eased = self.easing.apply(t);
                self.start + (self.end - self.start) * eased
            }
        }
    }

    fn advance(&mut self, dt: Duration) {
        if self.state != State::Running {
            return;
        }
        self.elapsed += dt;
        if self.elapsed >= self.duration {
            self.elapsed = self.duration;
            self.state = State::Finished;
        }
    }
}

/// Manages a set of active animation tracks.
#[derive(Debug)]
pub struct Animator {
    animations: HashMap<AnimationId, Animation>,
    next_id: u64,
}

impl Animator {
    pub fn new() -> Self {
        Self {
            animations: HashMap::new(),
            next_id: 1,
        }
    }

    /// Start a new animation. Returns an `AnimationId` that the view
    /// can use to read the current interpolated value.
    ///
    /// `from` is the starting value (read from the current state).
    /// If an animation with the same `id` already exists it is replaced.
    pub fn animate(
        &mut self,
        id: AnimationId,
        from: f32,
        to: f32,
        duration: Duration,
        easing: Easing,
    ) -> AnimationId {
        let anim = Animation::new(id, from, to, duration, easing);
        self.animations.insert(id, anim);
        self.next_id = self.next_id.max(id + 1);
        id
    }

    /// Animate to a value, but if an animation with this `id` already
    /// exists, start from where that animation currently is (seamless
    /// retargeting).
    pub fn animate_from_current(
        &mut self,
        id: AnimationId,
        to: f32,
        duration: Duration,
        easing: Easing,
    ) -> AnimationId {
        let from = self.current_value(id);
        let anim = Animation::new(id, from, to, duration, easing);
        self.animations.insert(id, anim);
        id
    }

    /// Read the current interpolated value of an animation.
    /// Returns the `end` value if the animation does not exist.
    pub fn get(&self, id: AnimationId) -> f32 {
        self.animations.get(&id).map_or(0.0, |a| a.value())
    }

    /// Read the current value, falling back to a default if the
    /// animation isn't registered.
    pub fn get_or(&self, id: AnimationId, default: f32) -> f32 {
        self.animations.get(&id).map_or(default, |a| a.value())
    }

    /// Convenience: read the current value of an animation that targets
    /// a known `end`. Before the animation starts this returns `end`; while
    /// running it returns the interpolated value; after finishing `end`.
    pub fn current_value(&self, id: AnimationId) -> f32 {
        self.get(id)
    }

    /// Advance all running animations by `dt`.
    pub fn update(&mut self, dt: Duration) {
        for anim in self.animations.values_mut() {
            anim.advance(dt);
        }
    }

    /// Remove finished animations. Call periodically to keep the
    /// animator compact.
    pub fn reap_finished(&mut self) {
        self.animations.retain(|_, a| a.state != State::Finished);
    }

    /// How many animations are currently running.
    pub fn running_count(&self) -> usize {
        self.animations.values().filter(|a| a.state == State::Running).count()
    }

    /// Check if a specific animation is still running.
    pub fn is_running(&self, id: AnimationId) -> bool {
        self.animations.get(&id).map_or(false, |a| a.state == State::Running)
    }

    /// Reserve a unique animation ID.
    pub fn reserve_id(&mut self) -> AnimationId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

impl Default for Animator {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animates_from_start_to_end() {
        let mut anim = Animator::new();
        let id = anim.animate(1, 0.0, 100.0, Duration::from_secs(1), Easing::Linear);
        assert!((anim.get(id) - 0.0).abs() < 0.1);
        anim.update(Duration::from_millis(500));
        let v = anim.get(id);
        assert!(v > 40.0 && v < 60.0, "at 50% should be ~50, got {v}");
        anim.update(Duration::from_millis(500));
        assert!((anim.get(id) - 100.0).abs() < 0.1);
    }

    #[test]
    fn retargeting_is_seamless() {
        let mut anim = Animator::new();
        let id = anim.animate(1, 0.0, 100.0, Duration::from_millis(500), Easing::Linear);
        anim.update(Duration::from_millis(250));
        let mid = anim.get(id);
        assert!(mid > 40.0 && mid < 60.0);
        // Retarget from where we are to 200.
        anim.animate_from_current(id, 200.0, Duration::from_millis(500), Easing::Linear);
        anim.update(Duration::from_millis(500));
        let v = anim.get(id);
        assert!(v > 140.0 && v < 210.0, "should reach ~200, got {v}");
    }

    #[test]
    fn finished_animation_holds_end() {
        let mut anim = Animator::new();
        let id = anim.animate(1, 10.0, 20.0, Duration::ZERO, Easing::Linear);
        assert!((anim.get(id) - 20.0).abs() < 0.1);
    }

    #[test]
    fn running_count_accuracy() {
        let mut anim = Animator::new();
        anim.animate(1, 0.0, 1.0, Duration::from_secs(1), Easing::Linear);
        anim.animate(2, 0.0, 1.0, Duration::from_millis(1), Easing::Linear);
        assert_eq!(anim.running_count(), 2);
        anim.update(Duration::from_millis(10));
        assert_eq!(anim.running_count(), 1);
    }
}
