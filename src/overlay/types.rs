use crate::animation::FadeAnimation;
use crate::config::{Config, Rgba};

use super::render::OverlayRenderer;

#[derive(Debug, Clone)]
pub struct OverlayParams {
    pub bg_color: Rgba,
    pub fade_duration: f64,
    pub hold_duration: [f64; 2],
    pub fps: u32,
    pub time_str: String,
    pub font_size: i32,
    pub font_name: String,
    pub fg_color: Rgba,
}

impl OverlayParams {
    pub fn from_config(config: &Config, bg_color: Rgba, time_str: String) -> Self {
        Self {
            bg_color,
            fade_duration: normalize_fade_duration(config.overlay.fade_duration),
            hold_duration: normalize_hold_duration(config.overlay.hold_duration),
            fps: normalize_fps(config.overlay.fps),
            time_str: normalize_time_text(time_str),
            font_size: normalize_font_size(config.foreground.font_size),
            font_name: normalize_font_name(&config.foreground.font_name),
            fg_color: config.foreground.fg_color,
        }
    }
}

#[derive(Debug)]
pub struct OverlayRuntimeState {
    pub animation: FadeAnimation,
    pub current_alpha: u8,
    pub input_received: bool,
}

impl OverlayRuntimeState {
    pub fn new(params: &OverlayParams) -> Self {
        Self {
            animation: FadeAnimation::new(
                params.bg_color.3,
                params.fade_duration,
                params.hold_duration,
            ),
            current_alpha: 0,
            input_received: false,
        }
    }
}

#[derive(Debug)]
pub struct OverlayViewState {
    pub bg_color: Rgba,
    pub time_str: String,
    pub time_wide: Vec<u16>,
    pub fps: u32,
    pub timer_interval_ms: u32,
    pub font_size: i32,
    pub font_name: String,
    pub font_name_wide: Vec<u16>,
    pub fg_color: Rgba,
}

impl OverlayViewState {
    pub fn from_params(params: OverlayParams) -> Self {
        let time_wide = params.time_str.encode_utf16().collect();
        let font_name_wide = params
            .font_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        Self {
            bg_color: params.bg_color,
            time_str: params.time_str,
            time_wide,
            fps: params.fps,
            timer_interval_ms: normalize_timer_interval(params.fps),
            font_size: params.font_size,
            font_name: params.font_name,
            font_name_wide,
            fg_color: params.fg_color,
        }
    }
}

#[derive(Debug)]
pub struct OverlayWindowState {
    pub runtime: OverlayRuntimeState,
    pub view: OverlayViewState,
    pub renderer: OverlayRenderer,
}

impl OverlayWindowState {
    pub fn new(params: OverlayParams) -> Self {
        Self {
            runtime: OverlayRuntimeState::new(&params),
            view: OverlayViewState::from_params(params),
            renderer: OverlayRenderer::default(),
        }
    }
}

pub fn normalize_fade_duration(duration: f64) -> f64 {
    duration.max(0.1)
}

pub fn normalize_hold_duration(hold: [f64; 2]) -> [f64; 2] {
    let min_hold = hold[0].max(0.0);
    let max_hold = hold[1].max(min_hold);
    [min_hold, max_hold]
}

pub fn normalize_fps(fps: u32) -> u32 {
    fps.max(1)
}

pub fn normalize_timer_interval(fps: u32) -> u32 {
    (1000 / normalize_fps(fps)).max(1)
}

pub fn normalize_font_size(font_size: i32) -> i32 {
    font_size.max(1)
}

pub fn normalize_font_name(font_name: &str) -> String {
    let trimmed = font_name.trim();
    if trimmed.is_empty() {
        "Arial".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn normalize_time_text(time_str: String) -> String {
    if time_str.is_empty() {
        " ".to_string()
    } else {
        time_str
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ForegroundConfig, IntervalReminder, OverlayConfig, ScheduleReminder};

    #[test]
    fn normalizes_invalid_overlay_values() {
        let config = Config {
            overlay: OverlayConfig {
                fade_duration: 0.0,
                hold_duration: [-1.0, -2.0],
                fps: 0,
            },
            foreground: ForegroundConfig {
                font_size: 0,
                font_name: "   ".to_string(),
                fg_color: Rgba(1, 2, 3, 4),
            },
            interval_reminder: IntervalReminder::default(),
            schedule_reminder: vec![ScheduleReminder {
                time: "12:00".to_string(),
                bg_color: Rgba(5, 6, 7, 8),
            }],
        };

        let params = OverlayParams::from_config(&config, Rgba(9, 10, 11, 12), String::new());

        assert_eq!(params.fade_duration, 0.1);
        assert_eq!(params.hold_duration, [0.0, 0.0]);
        assert_eq!(params.fps, 1);
        assert_eq!(params.font_size, 1);
        assert_eq!(params.font_name, "Arial");
        assert_eq!(params.time_str, " ");
    }

    #[test]
    fn timer_interval_is_never_zero() {
        assert_eq!(normalize_timer_interval(0), 1000);
        assert_eq!(normalize_timer_interval(60), 16);
    }
}
