use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq)]
enum FadeState {
    FadeIn,
    Hold,
    FadeOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationUpdate {
    Continue(u8),
    Close,
}

#[derive(Debug)]
pub struct FadeAnimation {
    state: FadeState,
    state_started_at: Instant,
    target_alpha: u8,
    fade_duration: f64,
    hold_duration: [f64; 2],
}

fn smootherstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

pub(crate) fn normalize_fade_duration(duration: f64) -> f64 {
    duration.max(0.1)
}

impl FadeAnimation {
    pub fn new(target_alpha: u8, fade_duration: f64, hold_duration: [f64; 2]) -> Self {
        Self {
            state: FadeState::FadeIn,
            state_started_at: Instant::now(),
            target_alpha,
            fade_duration: normalize_fade_duration(fade_duration),
            hold_duration,
        }
    }

    pub fn tick(&mut self, input_received: bool) -> AnimationUpdate {
        match self.state {
            FadeState::FadeIn => self.tick_fade_in(),
            FadeState::Hold => self.tick_hold(input_received),
            FadeState::FadeOut => self.tick_fade_out(),
        }
    }

    fn tick_fade_in(&mut self) -> AnimationUpdate {
        let progress = self.state_started_at.elapsed().as_secs_f64() / self.fade_duration;
        if progress >= 1.0 {
            self.transition_to(FadeState::Hold);
            AnimationUpdate::Continue(self.target_alpha)
        } else {
            let eased = smootherstep(progress);
            AnimationUpdate::Continue((self.target_alpha as f64 * eased) as u8)
        }
    }

    fn tick_hold(&mut self, input_received: bool) -> AnimationUpdate {
        let hold_elapsed = self.state_started_at.elapsed().as_secs_f64();
        let min_hold = self.hold_duration[0];
        let max_hold = self.hold_duration[1];

        if hold_elapsed >= min_hold && (hold_elapsed >= max_hold || input_received) {
            self.transition_to(FadeState::FadeOut);
        }

        AnimationUpdate::Continue(self.target_alpha)
    }

    fn tick_fade_out(&mut self) -> AnimationUpdate {
        let progress = self.state_started_at.elapsed().as_secs_f64() / self.fade_duration;
        if progress >= 1.0 {
            AnimationUpdate::Close
        } else {
            let eased = smootherstep(progress);
            AnimationUpdate::Continue((self.target_alpha as f64 * (1.0 - eased)) as u8)
        }
    }

    fn transition_to(&mut self, state: FadeState) {
        self.state = state;
        self.state_started_at = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    fn state_name(state: FadeState) -> &'static str {
        match state {
            FadeState::FadeIn => "fade-in",
            FadeState::Hold => "hold",
            FadeState::FadeOut => "fade-out",
        }
    }

    #[test]
    fn fade_duration_is_clamped() {
        let animation = FadeAnimation::new(200, 0.0, [0.0, 0.0]);
        assert_eq!(animation.fade_duration, 0.1);
    }

    #[test]
    fn input_does_not_exit_hold_before_min_duration() {
        let mut animation = FadeAnimation::new(200, 0.1, [0.05, 0.2]);

        sleep(Duration::from_millis(110));
        assert_eq!(animation.tick(false), AnimationUpdate::Continue(200));
        assert_eq!(state_name(animation.state), "hold");

        sleep(Duration::from_millis(20));
        assert_eq!(animation.tick(true), AnimationUpdate::Continue(200));
        assert_eq!(state_name(animation.state), "hold");
    }

    #[test]
    fn input_exits_hold_after_min_duration() {
        let mut animation = FadeAnimation::new(200, 0.1, [0.05, 0.2]);

        sleep(Duration::from_millis(110));
        let _ = animation.tick(false);
        sleep(Duration::from_millis(60));

        assert_eq!(animation.tick(true), AnimationUpdate::Continue(200));
        assert_eq!(state_name(animation.state), "fade-out");
    }

    #[test]
    fn max_hold_forces_fade_out_without_input() {
        let mut animation = FadeAnimation::new(200, 0.1, [0.05, 0.08]);

        sleep(Duration::from_millis(110));
        let _ = animation.tick(false);
        sleep(Duration::from_millis(90));

        assert_eq!(animation.tick(false), AnimationUpdate::Continue(200));
        assert_eq!(state_name(animation.state), "fade-out");
    }

    #[test]
    fn fade_out_closes_after_duration() {
        let mut animation = FadeAnimation::new(200, 0.1, [0.0, 0.0]);

        sleep(Duration::from_millis(110));
        let _ = animation.tick(false);
        let _ = animation.tick(true);
        sleep(Duration::from_millis(110));

        assert_eq!(animation.tick(false), AnimationUpdate::Close);
    }
}
