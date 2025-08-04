use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub hotkey: HotkeyConfig,
    pub barrier: BarrierConfig,
    pub hud: HudConfig,
    pub debug: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HotkeyConfig {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarrierConfig {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub buffer_zone: i32,
    pub push_factor: i32,
    pub overlay_color: OverlayColor,
    pub overlay_alpha: u8, // 0-255, where 255 is opaque, 0 is transparent
    pub audio_feedback: AudioFeedbackConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFeedbackConfig {
    pub on_barrier_hit: AudioOption,
    pub on_barrier_entry: AudioOption,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioOption {
    None,
    File(String), // Path to audio file
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayColor {
    pub r: u8, // Red component (0-255)
    pub g: u8, // Green component (0-255)
    pub b: u8, // Blue component (0-255)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HudConfig {
    pub enabled: bool,
    pub position: HudPosition,
    pub background_alpha: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HudPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

// Parse the default config from config.ron at compile time (embedded) and runtime (parsed)
static DEFAULT_CONFIG: OnceLock<Config> = OnceLock::new();

fn get_default_config() -> &'static Config {
    DEFAULT_CONFIG.get_or_init(|| {
        const DEFAULT_CONFIG_STR: &str = include_str!("../../config.ron");
        ron::from_str(DEFAULT_CONFIG_STR)
            .expect("Failed to parse embedded config.ron - config file is invalid")
    })
}

impl Default for Config {
    fn default() -> Self {
        get_default_config().clone()
    }
}

impl Config {
    pub fn load_from_file<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = ron::from_str(&content)?;
        Ok(config)
    }

    pub fn load_or_create(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let config: Config = ron::from_str(&content)?;
                Ok(config)
            }
            Err(_) => {
                let config = Config::default();
                config.save(path)?;
                Ok(config)
            }
        }
    }

    pub fn save(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

pub fn vk_code_from_string(key: &str) -> Option<u32> {
    use winapi::um::winuser::*;

    match key.to_uppercase().as_str() {
        "F1" => Some(VK_F1 as u32),
        "F2" => Some(VK_F2 as u32),
        "F3" => Some(VK_F3 as u32),
        "F4" => Some(VK_F4 as u32),
        "F5" => Some(VK_F5 as u32),
        "F6" => Some(VK_F6 as u32),
        "F7" => Some(VK_F7 as u32),
        "F8" => Some(VK_F8 as u32),
        "F9" => Some(VK_F9 as u32),
        "F10" => Some(VK_F10 as u32),
        "F11" => Some(VK_F11 as u32),
        "F12" => Some(VK_F12 as u32),
        "A" => Some(0x41),
        "B" => Some(0x42),
        "C" => Some(0x43),
        "D" => Some(0x44),
        "E" => Some(0x45),
        "F" => Some(0x46),
        "G" => Some(0x47),
        "H" => Some(0x48),
        "I" => Some(0x49),
        "J" => Some(0x4A),
        "K" => Some(0x4B),
        "L" => Some(0x4C),
        "M" => Some(0x4D),
        "N" => Some(0x4E),
        "O" => Some(0x4F),
        "P" => Some(0x50),
        "Q" => Some(0x51),
        "R" => Some(0x52),
        "S" => Some(0x53),
        "T" => Some(0x54),
        "U" => Some(0x55),
        "V" => Some(0x56),
        "W" => Some(0x57),
        "X" => Some(0x58),
        "Y" => Some(0x59),
        "Z" => Some(0x5A),
        "0" => Some(0x30),
        "1" => Some(0x31),
        "2" => Some(0x32),
        "3" => Some(0x33),
        "4" => Some(0x34),
        "5" => Some(0x35),
        "6" => Some(0x36),
        "7" => Some(0x37),
        "8" => Some(0x38),
        "9" => Some(0x39),
        _ => None,
    }
}
