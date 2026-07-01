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

impl FadeAnimation {
    pub fn new(target_alpha: u8, fade_duration: f64, hold_duration: [f64; 2]) -> Self {
        Self {
            state: FadeState::FadeIn,
            state_started_at: Instant::now(),
            target_alpha,
            fade_duration: fade_duration.max(0.1),
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
            AnimationUpdate::Continue((self.target_alpha as f64 * progress) as u8)
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
            AnimationUpdate::Continue((self.target_alpha as f64 * (1.0 - progress)) as u8)
        }
    }

    fn transition_to(&mut self, state: FadeState) {
        self.state = state;
        self.state_started_at = Instant::now();
    }
}
