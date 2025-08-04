use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tracing::info;

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
#[serde(untagged)]
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

#[derive(Debug, Clone, PartialEq)]
pub enum HudPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

// This is an ugly work around, but it seems to be the only way to make config-rs work properly with RON enums.
impl Serialize for HudPosition {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            HudPosition::TopLeft => "TopLeft",
            HudPosition::TopRight => "TopRight",
            HudPosition::BottomLeft => "BottomLeft",
            HudPosition::BottomRight => "BottomRight",
        };
        serializer.serialize_str(s)
    }
}

// This is an ugly work around, but it seems to be the only way to make config-rs work properly with RON enums.
impl<'de> Deserialize<'de> for HudPosition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "TopLeft" => Ok(HudPosition::TopLeft),
            "TopRight" => Ok(HudPosition::TopRight),
            "BottomLeft" => Ok(HudPosition::BottomLeft),
            "BottomRight" => Ok(HudPosition::BottomRight),
            _ => Err(serde::de::Error::custom(format!(
                "Unknown HudPosition: {}",
                s
            ))),
        }
    }
}

// Embed the default config from config.ron at compile time
const DEFAULT_CONFIG_STR: &str = include_str!("../../config.ron");

// Parse the default config from config.ron at compile time (embedded) and runtime (parsed)
static DEFAULT_CONFIG: OnceLock<Config> = OnceLock::new();

fn get_default_config() -> &'static Config {
    DEFAULT_CONFIG.get_or_init(|| {
        // Use config-rs to parse the embedded default config
        let settings = config::Config::builder()
            .add_source(config::File::from_str(
                DEFAULT_CONFIG_STR,
                config::FileFormat::Ron,
            ))
            .build()
            .expect("Failed to build default config");

        settings
            .try_deserialize()
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
        // Use config-rs for layered loading with defaults as base
        let settings = config::Config::builder()
            .add_source(config::File::from_str(
                DEFAULT_CONFIG_STR,
                config::FileFormat::Ron,
            ))
            .add_source(config::File::from(path.as_ref()).format(config::FileFormat::Ron))
            .build()?;

        let config: Config = settings.try_deserialize()?;
        Ok(config)
    }

    pub fn load_or_create(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Check if user config file exists
        let user_config_exists = std::path::Path::new(path).exists();

        // Build layered configuration using config-rs
        let mut builder = config::Config::builder();

        // Layer 1: Always load embedded defaults from config.ron
        builder = builder.add_source(config::File::from_str(
            DEFAULT_CONFIG_STR,
            config::FileFormat::Ron,
        ));

        // Layer 2: User config file (if it exists) - overrides defaults
        if user_config_exists {
            builder =
                builder.add_source(config::File::with_name(path).format(config::FileFormat::Ron));
        }

        // Build and deserialize the configuration
        let settings = builder.build()?;
        let config: Config = settings.try_deserialize()?;

        // Create default config file if it doesn't exist
        if !user_config_exists {
            info!("Config file not found. Creating default config at {}", path);
            config.save(path)?;
        }

        Ok(config)
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

#[cfg(test)]
mod tests {
    use super::*;
    use winapi::um::winuser::*;

    #[test]
    fn test_default_config_can_be_created() {
        let config = Config::default();
        assert!(config.hotkey.ctrl);
        assert_eq!(config.hotkey.key, "F12");
        assert!(!config.debug);
    }

    #[test]
    fn test_audio_option_serialization() {
        let config_with_none = Config {
            barrier: BarrierConfig {
                audio_feedback: AudioFeedbackConfig {
                    on_barrier_hit: AudioOption::None,
                    on_barrier_entry: AudioOption::File("test.wav".to_string()),
                },
                ..Config::default().barrier
            },
            ..Config::default()
        };

        // Should be able to serialize and deserialize
        let ron_string = ron::to_string(&config_with_none).unwrap();
        let settings = config::Config::builder()
            .add_source(config::File::from_str(&ron_string, config::FileFormat::Ron))
            .build()
            .unwrap();
        let restored: Config = settings.try_deserialize().unwrap();

        match restored.barrier.audio_feedback.on_barrier_hit {
            AudioOption::None => {}
            _ => panic!("Expected None"),
        }

        match restored.barrier.audio_feedback.on_barrier_entry {
            AudioOption::File(path) => assert_eq!(path, "test.wav"),
            _ => panic!("Expected File"),
        }
    }

    #[test]
    fn test_hud_position_serialization() {
        let positions = vec![
            HudPosition::TopLeft,
            HudPosition::TopRight,
            HudPosition::BottomLeft,
            HudPosition::BottomRight,
        ];

        for pos in positions {
            let mut config = Config::default();
            config.hud.position = pos.clone();
            dbg!(&config);

            let ron_string = ron::to_string(&config).unwrap();
            let settings = config::Config::builder()
                .add_source(config::File::from_str(&ron_string, config::FileFormat::Ron))
                .build()
                .unwrap();
            let restored: Config = settings.try_deserialize().unwrap();

            // Now we can directly compare since HudPosition has PartialEq
            assert_eq!(restored.hud.position, pos);
        }
    }

    #[test]
    fn test_hotkey_config_creation() {
        let config = HotkeyConfig {
            ctrl: true,
            alt: false,
            shift: true,
            key: "F12".to_string(),
        };

        assert!(config.ctrl);
        assert!(!config.alt);
        assert!(config.shift);
        assert_eq!(config.key, "F12");
    }

    #[test]
    fn test_barrier_config_creation() {
        let config = BarrierConfig {
            x: 100,
            y: 200,
            width: 300,
            height: 150,
            buffer_zone: 25,
            push_factor: 50,
            overlay_color: OverlayColor { r: 255, g: 0, b: 0 },
            overlay_alpha: 128,
            audio_feedback: AudioFeedbackConfig {
                on_barrier_hit: AudioOption::None,
                on_barrier_entry: AudioOption::File("sound.wav".to_string()),
            },
        };

        assert_eq!(config.x, 100);
        assert_eq!(config.y, 200);
        assert_eq!(config.width, 300);
        assert_eq!(config.height, 150);
        assert_eq!(config.buffer_zone, 25);
        assert_eq!(config.push_factor, 50);
        assert_eq!(config.overlay_color.r, 255);
        assert_eq!(config.overlay_color.g, 0);
        assert_eq!(config.overlay_color.b, 0);
        assert_eq!(config.overlay_alpha, 128);

        match config.audio_feedback.on_barrier_hit {
            AudioOption::None => {}
            _ => panic!("Expected None"),
        }

        match config.audio_feedback.on_barrier_entry {
            AudioOption::File(path) => assert_eq!(path, "sound.wav"),
            _ => panic!("Expected File"),
        }
    }

    #[test]
    fn test_hud_config_creation() {
        let config = HudConfig {
            enabled: true,
            position: HudPosition::BottomRight,
            background_alpha: 200,
        };

        assert!(config.enabled);
        assert_eq!(config.position, HudPosition::BottomRight);
        assert_eq!(config.background_alpha, 200);
    }

    #[test]
    fn test_audio_feedback_config_creation() {
        let config = AudioFeedbackConfig {
            on_barrier_hit: AudioOption::File("hit.wav".to_string()),
            on_barrier_entry: AudioOption::None,
        };

        match config.on_barrier_hit {
            AudioOption::File(path) => assert_eq!(path, "hit.wav"),
            _ => panic!("Expected File"),
        }

        match config.on_barrier_entry {
            AudioOption::None => {}
            _ => panic!("Expected None"),
        }
    }

    #[test]
    fn test_overlay_color_creation() {
        let color = OverlayColor {
            r: 128,
            g: 64,
            b: 192,
        };

        assert_eq!(color.r, 128);
        assert_eq!(color.g, 64);
        assert_eq!(color.b, 192);
    }

    #[test]
    fn test_config_struct_full_construction() {
        let config = Config {
            hotkey: HotkeyConfig {
                ctrl: false,
                alt: true,
                shift: false,
                key: "F1".to_string(),
            },
            barrier: BarrierConfig {
                x: 50,
                y: 1080,
                width: 150,
                height: 75,
                buffer_zone: 20,
                push_factor: 30,
                overlay_color: OverlayColor { r: 0, g: 255, b: 0 },
                overlay_alpha: 100,
                audio_feedback: AudioFeedbackConfig {
                    on_barrier_hit: AudioOption::File("beep.wav".to_string()),
                    on_barrier_entry: AudioOption::File("enter.wav".to_string()),
                },
            },
            hud: HudConfig {
                enabled: false,
                position: HudPosition::TopLeft,
                background_alpha: 180,
            },
            debug: true,
        };

        // Verify hotkey config
        assert!(!config.hotkey.ctrl);
        assert!(config.hotkey.alt);
        assert!(!config.hotkey.shift);
        assert_eq!(config.hotkey.key, "F1");

        // Verify barrier config
        assert_eq!(config.barrier.x, 50);
        assert_eq!(config.barrier.y, 1080);
        assert_eq!(config.barrier.width, 150);
        assert_eq!(config.barrier.height, 75);
        assert_eq!(config.barrier.buffer_zone, 20);
        assert_eq!(config.barrier.push_factor, 30);
        assert_eq!(config.barrier.overlay_color.r, 0);
        assert_eq!(config.barrier.overlay_color.g, 255);
        assert_eq!(config.barrier.overlay_color.b, 0);
        assert_eq!(config.barrier.overlay_alpha, 100);

        // Verify HUD config
        assert!(!config.hud.enabled);
        assert_eq!(config.hud.position, HudPosition::TopLeft);
        assert_eq!(config.hud.background_alpha, 180);

        // Verify debug flag
        assert!(config.debug);
    }

    #[test]
    fn test_vk_code_from_string_function_keys() {
        // Test various function keys (only F1-F12 are supported)
        assert_eq!(vk_code_from_string("F1"), Some(VK_F1 as u32));
        assert_eq!(vk_code_from_string("F5"), Some(VK_F5 as u32));
        assert_eq!(vk_code_from_string("F12"), Some(VK_F12 as u32));

        // Test case sensitivity
        assert_eq!(vk_code_from_string("f1"), Some(VK_F1 as u32));
        assert_eq!(vk_code_from_string("f12"), Some(VK_F12 as u32));
    }

    #[test]
    fn test_vk_code_from_string_alphabet() {
        // Test alphabet keys
        assert_eq!(vk_code_from_string("A"), Some(0x41));
        assert_eq!(vk_code_from_string("M"), Some(0x4D));
        assert_eq!(vk_code_from_string("Z"), Some(0x5A));

        // Test lowercase (should still work)
        assert_eq!(vk_code_from_string("a"), Some(0x41));
        assert_eq!(vk_code_from_string("z"), Some(0x5A));
    }

    #[test]
    fn test_vk_code_from_string_numbers() {
        // Test number keys
        assert_eq!(vk_code_from_string("0"), Some(0x30));
        assert_eq!(vk_code_from_string("5"), Some(0x35));
        assert_eq!(vk_code_from_string("9"), Some(0x39));
    }

    #[test]
    fn test_vk_code_from_string_unsupported_keys() {
        // Test that unsupported special keys return None
        assert_eq!(vk_code_from_string("SPACE"), None);
        assert_eq!(vk_code_from_string("ENTER"), None);
        assert_eq!(vk_code_from_string("ESC"), None);
        assert_eq!(vk_code_from_string("TAB"), None);
    }

    #[test]
    fn test_vk_code_from_string_invalid_keys() {
        // Test invalid keys
        assert_eq!(vk_code_from_string("INVALID"), None);
        assert_eq!(vk_code_from_string("F24"), None); // Unsupported function key (only F1-F12)
        assert_eq!(vk_code_from_string("F25"), None); // Unsupported function key
        assert_eq!(vk_code_from_string(""), None); // Empty string
        assert_eq!(vk_code_from_string("123"), None); // Invalid format
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let original = Config::default();

        // Serialize to RON
        let ron_string = ron::to_string(&original).unwrap();

        // Deserialize back
        let settings = config::Config::builder()
            .add_source(config::File::from_str(&ron_string, config::FileFormat::Ron))
            .build()
            .unwrap();
        let restored: Config = settings.try_deserialize().unwrap();

        // Verify key fields are preserved
        assert_eq!(restored.hotkey.ctrl, original.hotkey.ctrl);
        assert_eq!(restored.hotkey.key, original.hotkey.key);
        assert_eq!(restored.barrier.x, original.barrier.x);
        assert_eq!(restored.barrier.width, original.barrier.width);
        assert_eq!(restored.hud.enabled, original.hud.enabled);
        assert_eq!(restored.debug, original.debug);
    }

    #[test]
    fn test_config_clone() {
        let original = Config::default();
        let cloned = original.clone();

        // Verify they have the same values
        assert_eq!(cloned.hotkey.ctrl, original.hotkey.ctrl);
        assert_eq!(cloned.hotkey.key, original.hotkey.key);
        assert_eq!(cloned.barrier.x, original.barrier.x);
        assert_eq!(cloned.hud.enabled, original.hud.enabled);
        assert_eq!(cloned.debug, original.debug);
    }

    #[test]
    fn test_audio_option_variants() {
        // Test None variant
        let none_option = AudioOption::None;
        match none_option {
            AudioOption::None => {}
            _ => panic!("Expected None variant"),
        }

        // Test File variant
        let file_option = AudioOption::File("test.wav".to_string());
        match file_option {
            AudioOption::File(path) => assert_eq!(path, "test.wav"),
            _ => panic!("Expected File variant"),
        }
    }

    #[test]
    fn test_hud_position_all_variants() {
        let positions = [
            HudPosition::TopLeft,
            HudPosition::TopRight,
            HudPosition::BottomLeft,
            HudPosition::BottomRight,
        ];

        // Test that all variants can be created and are unique
        for (i, pos1) in positions.iter().enumerate() {
            for (j, pos2) in positions.iter().enumerate() {
                if i == j {
                    assert_eq!(pos1, pos2);
                } else {
                    assert_ne!(pos1, pos2);
                }
            }
        }
    }

    #[test]
    fn test_default_config_values() {
        let config = Config::default();

        // Test that default values are reasonable
        assert!(config.hotkey.ctrl); // Default should require Ctrl
        assert_eq!(config.hotkey.key, "F12"); // Default key should be F12
        assert_eq!(config.barrier.x, 0); // Default barrier at bottom-left corner
        assert!(config.barrier.y > 0); // Should have a positive Y (screen height)
        assert!(config.barrier.width > 0); // Should have positive width
        assert!(config.barrier.height > 0); // Should have positive height
        assert!(config.barrier.buffer_zone >= 0); // Buffer zone should be non-negative
        assert!(config.barrier.push_factor > 0); // Push factor should be positive
        assert_eq!(config.barrier.overlay_alpha, 200); // Default from config.ron
        assert!(config.hud.enabled); // HUD enabled by default
        assert!(!config.debug); // Debug disabled by default
    }
}
