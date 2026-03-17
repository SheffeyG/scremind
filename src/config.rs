use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub overlay: OverlayConfig,

    #[serde(default)]
    pub foreground: ForegroundConfig,

    #[serde(default)]
    pub interval_reminder: IntervalReminder,

    #[serde(default)]
    pub schedule_reminder: Vec<ScheduleReminder>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OverlayConfig {
    #[serde(default = "default_fade_duration")]
    pub fade_duration: f64,

    #[serde(default = "default_hold_duration")]
    pub hold_duration: [f64; 2],

    #[serde(default = "default_fps")]
    pub fps: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IntervalReminder {
    #[serde(default = "default_interval")]
    pub interval: u64,

    #[serde(default = "default_bg_color")]
    pub bg_color: [u8; 4],
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScheduleReminder {
    pub time: String,

    #[serde(default = "default_bg_color")]
    pub bg_color: [u8; 4],
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ForegroundConfig {
    #[serde(default = "default_font_size")]
    pub font_size: i32,

    #[serde(default = "default_font_name")]
    pub font_name: String,

    #[serde(default = "default_fg_color")]
    pub fg_color: [u8; 4],
}

fn default_fade_duration() -> f64 {
    1.0
}

fn default_hold_duration() -> [f64; 2] {
    [1.0, 5.0]
}

fn default_fps() -> u32 {
    60
}

fn default_interval() -> u64 {
    30 * 60
}

fn default_bg_color() -> [u8; 4] {
    [255, 255, 255, 30]
}

fn default_fg_color() -> [u8; 4] {
    [255, 255, 255, 150]
}

fn default_font_size() -> i32 {
    72
}

fn default_font_name() -> String {
    "Arial".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            overlay: OverlayConfig::default(),
            interval_reminder: IntervalReminder::default(),
            schedule_reminder: vec![],
            foreground: ForegroundConfig::default(),
        }
    }
}

impl Default for OverlayConfig {
    fn default() -> Self {
        OverlayConfig {
            fade_duration: default_fade_duration(),
            hold_duration: default_hold_duration(),
            fps: default_fps(),
        }
    }
}

impl Default for IntervalReminder {
    fn default() -> Self {
        IntervalReminder {
            interval: default_interval(),
            bg_color: default_bg_color(),
        }
    }
}

impl Default for ForegroundConfig {
    fn default() -> Self {
        ForegroundConfig {
            font_size: default_font_size(),
            font_name: default_font_name(),
            fg_color: default_fg_color(),
        }
    }
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();

        if !path.exists() {
            let default_config = Config::default();
            let toml_str = toml::to_string_pretty(&default_config)?;
            fs::write(path, toml_str)?;
            println!("Created default config file: {}", path.display());
            return Ok(default_config);
        }

        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
