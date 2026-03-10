use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub overlay: OverlayConfig,

    #[serde(default)]
    pub interval_reminder: IntervalReminder,

    #[serde(default)]
    pub scheduled_reminders: Vec<ScheduledReminder>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OverlayConfig {
    #[serde(default = "default_fade_duration")]
    pub fade_duration: f64,

    #[serde(default = "default_fps")]
    pub fps: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IntervalReminder {
    #[serde(default = "default_interval")]
    pub interval: u64,

    #[serde(default)]
    pub color: ColorConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScheduledReminder {
    pub time: String,

    #[serde(default)]
    pub color: ColorConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColorConfig {
    #[serde(default = "default_red")]
    pub r: u8,

    #[serde(default = "default_green")]
    pub g: u8,

    #[serde(default = "default_blue")]
    pub b: u8,

    #[serde(default = "default_alpha")]
    pub a: u8,
}

fn default_fade_duration() -> f64 {
    1.0
}

fn default_fps() -> u32 {
    60
}

fn default_interval() -> u64 {
    30 * 60
}

fn default_alpha() -> u8 {
    30
}

fn default_red() -> u8 {
    255
}

fn default_green() -> u8 {
    255
}

fn default_blue() -> u8 {
    255
}

impl Default for Config {
    fn default() -> Self {
        Config {
            overlay: OverlayConfig::default(),
            interval_reminder: IntervalReminder::default(),
            scheduled_reminders: vec![],
        }
    }
}

impl Default for OverlayConfig {
    fn default() -> Self {
        OverlayConfig {
            fade_duration: default_fade_duration(),
            fps: default_fps(),
        }
    }
}

impl Default for IntervalReminder {
    fn default() -> Self {
        IntervalReminder {
            interval: default_interval(),
            color: ColorConfig::default(),
        }
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        ColorConfig {
            r: default_red(),
            g: default_green(),
            b: default_blue(),
            a: default_alpha(),
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
