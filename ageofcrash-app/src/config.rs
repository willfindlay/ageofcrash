use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tracing::{info, warn};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
                // Try to parse the existing config fully
                match ron::from_str::<Config>(&content) {
                    Ok(config) => {
                        // Config is valid, but check if we need to add any missing fields
                        let merged_config = Self::merge_configs(&Config::default(), &config)?;

                        // Save back if the merged config is different from what we loaded
                        // This adds any new fields that were missing
                        let merged_content = ron::ser::to_string_pretty(
                            &merged_config,
                            ron::ser::PrettyConfig::default(),
                        )?;
                        if merged_content.trim() != content.trim() {
                            info!("Adding missing configuration fields to {}", path);
                            std::fs::write(path, merged_content)?;
                        }

                        Ok(merged_config)
                    }
                    Err(parse_error) => {
                        // If parsing fails, try to merge partial config with defaults
                        warn!(
                            "Config file has errors: {}. Attempting to merge with defaults...",
                            parse_error
                        );
                        let merged_config = Self::merge_with_defaults(&content)?;

                        // Save the merged config back to file
                        info!("Saving merged configuration to {}", path);
                        merged_config.save(path)?;

                        Ok(merged_config)
                    }
                }
            }
            Err(_) => {
                // Create new default config if file doesn't exist
                info!("Config file not found. Creating default config at {}", path);
                let config = Config::default();
                config.save(path)?;
                Ok(config)
            }
        }
    }

    /// Merge two configs using JSON merging (user config overrides defaults)
    fn merge_configs(
        default: &Config,
        user: &Config,
    ) -> Result<Config, Box<dyn std::error::Error>> {
        // Convert both configs to JSON
        let default_json = serde_json::to_value(default)?;
        let user_json = serde_json::to_value(user)?;

        // Merge JSON values (user overrides default)
        let merged_json = Self::merge_json_values(default_json, user_json);

        // Convert back to Config
        let merged_config: Config = serde_json::from_value(merged_json)?;
        Ok(merged_config)
    }

    /// Merge a partial/invalid config with the default config, preserving valid user settings
    fn merge_with_defaults(existing_content: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Start with the default config as JSON
        let default_config = Config::default();
        let mut default_json = serde_json::to_value(&default_config)?;

        // Try to parse user config as RON and convert to JSON for merging
        if let Ok(user_ron) = ron::from_str::<ron::Value>(existing_content) {
            // Convert RON to JSON string then parse as JSON
            let ron_as_json_str = ron::to_string(&user_ron)?;
            if let Ok(user_json) = serde_json::from_str::<serde_json::Value>(&ron_as_json_str) {
                default_json = Self::merge_json_values(default_json, user_json);
            }
        }

        // Convert merged JSON back to Config
        let merged_config: Config = serde_json::from_value(default_json)?;
        Ok(merged_config)
    }

    /// Recursively merge two JSON values (right overrides left)
    fn merge_json_values(left: serde_json::Value, right: serde_json::Value) -> serde_json::Value {
        match (left, right) {
            (serde_json::Value::Object(mut left_map), serde_json::Value::Object(right_map)) => {
                // Merge objects recursively
                for (key, right_value) in right_map {
                    let merged_value = if let Some(left_value) = left_map.remove(&key) {
                        Self::merge_json_values(left_value, right_value)
                    } else {
                        right_value
                    };
                    left_map.insert(key, merged_value);
                }
                serde_json::Value::Object(left_map)
            }
            (_, right) => {
                // For non-objects, right side takes precedence
                right
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
    fn test_merge_configs_preserves_user_settings() {
        let default = Config::default();
        let user = Config {
            debug: true,
            hotkey: crate::config::HotkeyConfig {
                key: "F1".to_string(),
                ..Config::default().hotkey
            },
            barrier: crate::config::BarrierConfig {
                width: 500,
                ..Config::default().barrier
            },
            ..Config::default()
        };

        let merged = Config::merge_configs(&default, &user).unwrap();

        // User settings should be preserved
        assert!(merged.debug);
        assert_eq!(merged.hotkey.key, "F1");
        assert_eq!(merged.barrier.width, 500);

        // Other default settings should remain
        assert!(merged.hotkey.ctrl);
        assert_eq!(merged.barrier.height, default.barrier.height);
    }

    #[test]
    fn test_merge_configs_adds_missing_fields() {
        // Test the actual merge logic directly with JSON values
        let default_config = Config::default();
        let default_json = serde_json::to_value(&default_config).unwrap();

        // Create partial user JSON (simulating what comes from a partial config file)
        let user_json = serde_json::json!({
            "hotkey": {
                "ctrl": false,
                "key": "F2"
                // Missing alt, shift fields
            },
            "barrier": {
                "x": 100
                // Missing most barrier fields
            }
            // Missing hud and debug entirely
        });

        let merged_json = Config::merge_json_values(default_json, user_json);
        let merged_config: Config = serde_json::from_value(merged_json).unwrap();

        // User settings should be preserved
        assert!(!merged_config.hotkey.ctrl);
        assert_eq!(merged_config.hotkey.key, "F2");
        assert_eq!(merged_config.barrier.x, 100);

        // Missing fields should come from defaults
        assert_eq!(merged_config.hotkey.alt, default_config.hotkey.alt);
        assert_eq!(merged_config.hotkey.shift, default_config.hotkey.shift);
        assert_eq!(merged_config.barrier.width, default_config.barrier.width);
        assert_eq!(merged_config.hud.enabled, default_config.hud.enabled);
        assert_eq!(merged_config.debug, default_config.debug);
    }

    #[test]
    fn test_merge_with_defaults_handles_partial_ron() {
        let partial_ron = r#"(
            hotkey: (
                ctrl: false,
                key: "F5",
            ),
            debug: true,
        )"#;

        let merged = Config::merge_with_defaults(partial_ron).unwrap();

        // User settings should be preserved
        assert!(!merged.hotkey.ctrl);
        assert_eq!(merged.hotkey.key, "F5");
        assert!(merged.debug);

        // Missing fields should come from defaults
        let default = Config::default();
        assert_eq!(merged.hotkey.alt, default.hotkey.alt);
        assert_eq!(merged.hotkey.shift, default.hotkey.shift);
        assert_eq!(merged.barrier.x, default.barrier.x);
        assert_eq!(merged.hud.enabled, default.hud.enabled);
    }

    #[test]
    fn test_merge_with_defaults_handles_invalid_ron() {
        let invalid_ron = r#"this is not valid ron"#;

        let merged = Config::merge_with_defaults(invalid_ron).unwrap();

        // Should fall back to complete defaults
        let default = Config::default();
        assert_eq!(merged.hotkey.ctrl, default.hotkey.ctrl);
        assert_eq!(merged.hotkey.key, default.hotkey.key);
        assert_eq!(merged.debug, default.debug);
    }

    #[test]
    fn test_merge_json_values_deep_merge() {
        let left = serde_json::json!({
            "a": 1,
            "b": {
                "c": 2,
                "d": 3
            },
            "e": 4
        });

        let right = serde_json::json!({
            "b": {
                "c": 99,
                "f": 5
            },
            "g": 6
        });

        let merged = Config::merge_json_values(left, right);

        assert_eq!(merged["a"], 1); // From left
        assert_eq!(merged["b"]["c"], 99); // Overridden by right
        assert_eq!(merged["b"]["d"], 3); // From left (preserved)
        assert_eq!(merged["b"]["f"], 5); // From right (new field)
        assert_eq!(merged["e"], 4); // From left
        assert_eq!(merged["g"], 6); // From right (new field)
    }

    #[test]
    fn test_merge_json_values_right_overrides_primitives() {
        let left = serde_json::json!({"a": 1, "b": "old"});
        let right = serde_json::json!({"a": 2, "c": "new"});

        let merged = Config::merge_json_values(left, right);

        assert_eq!(merged["a"], 2); // Overridden
        assert_eq!(merged["b"], "old"); // Preserved
        assert_eq!(merged["c"], "new"); // Added
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
        let json = serde_json::to_value(&config_with_none).unwrap();
        let restored: Config = serde_json::from_value(json).unwrap();

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
            let config = Config {
                hud: HudConfig {
                    position: pos.clone(),
                    ..Config::default().hud
                },
                ..Config::default()
            };

            let json = serde_json::to_value(&config).unwrap();
            let restored: Config = serde_json::from_value(json).unwrap();

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
        let color = OverlayColor { r: 128, g: 64, b: 192 };
        
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
        let restored: Config = ron::from_str(&ron_string).unwrap();
        
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
