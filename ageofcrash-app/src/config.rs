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
        if let Ok(user_ron) = ron::from_str::<ron::Value>(&existing_content) {
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

    #[test]
    fn test_default_config_can_be_created() {
        let config = Config::default();
        assert_eq!(config.hotkey.ctrl, true);
        assert_eq!(config.hotkey.key, "F12");
        assert_eq!(config.debug, false);
    }

    #[test]
    fn test_merge_configs_preserves_user_settings() {
        let default = Config::default();
        let mut user = Config::default();
        user.debug = true;
        user.hotkey.key = "F1".to_string();
        user.barrier.width = 500;

        let merged = Config::merge_configs(&default, &user).unwrap();

        // User settings should be preserved
        assert_eq!(merged.debug, true);
        assert_eq!(merged.hotkey.key, "F1");
        assert_eq!(merged.barrier.width, 500);

        // Other default settings should remain
        assert_eq!(merged.hotkey.ctrl, true);
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
        assert_eq!(merged_config.hotkey.ctrl, false);
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
        assert_eq!(merged.hotkey.ctrl, false);
        assert_eq!(merged.hotkey.key, "F5");
        assert_eq!(merged.debug, true);

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

            // Can't directly compare enums without PartialEq, so serialize both
            let original_json = serde_json::to_value(&pos).unwrap();
            let restored_json = serde_json::to_value(&restored.hud.position).unwrap();
            assert_eq!(original_json, restored_json);
        }
    }
}
