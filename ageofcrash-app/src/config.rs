use figment::{providers::Serialized, Figment, Profile};
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
        // Use Figment to layer defaults with user config
        let defaults = Config::default();
        let config: Config = Figment::new()
            .merge(Serialized::defaults(&defaults))
            .merge(Serialized::from(
                Self::load_ron_file(path)?,
                Profile::Default,
            ))
            .extract()?;
        Ok(config)
    }

    fn load_ron_file<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = ron::from_str(&content)?;
        Ok(config)
    }

    pub fn load_or_create(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Check if user config file exists
        let user_config_exists = std::path::Path::new(path).exists();

        // Build layered configuration using Figment
        let defaults = Config::default();
        let mut figment = Figment::new().merge(Serialized::defaults(&defaults));

        // Layer user config file if it exists (overrides defaults)
        if user_config_exists {
            let user_config = Self::load_ron_file(path)?;
            figment = figment.merge(Serialized::from(user_config, Profile::Default));
        }

        // Extract the configuration
        let config: Config = figment.extract()?;

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
    use proptest::prelude::*;
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
        let restored: Config = ron::from_str(&ron_string).unwrap();

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
    fn test_figment_preserves_file_syntax() {
        let file_option = AudioOption::File("test.wav".to_string());
        let none_option = AudioOption::None;

        // Test that RON serialization preserves File("path") syntax
        let file_ron = ron::to_string(&file_option).unwrap();
        let none_ron = ron::to_string(&none_option).unwrap();

        println!("AudioOption::File serialized: {}", file_ron);
        println!("AudioOption::None serialized: {}", none_ron);

        // Verify the File("path") syntax is preserved
        assert_eq!(file_ron, "File(\"test.wav\")");
        assert_eq!(none_ron, "None");

        // Test that Figment layering works with these values
        let test_config = Config {
            barrier: BarrierConfig {
                audio_feedback: AudioFeedbackConfig {
                    on_barrier_hit: none_option.clone(),
                    on_barrier_entry: file_option.clone(),
                },
                ..Config::default().barrier
            },
            ..Config::default()
        };

        // Use Figment to layer the config (simulating load_from_file logic)
        let defaults = Config::default();
        let layered_config: Config = Figment::new()
            .merge(Serialized::defaults(&defaults))
            .merge(Serialized::from(test_config, Profile::Default))
            .extract()
            .unwrap();

        // Verify the layered config preserves the enum values correctly
        match layered_config.barrier.audio_feedback.on_barrier_hit {
            AudioOption::None => {}
            _ => panic!("Expected None after Figment layering"),
        }

        match layered_config.barrier.audio_feedback.on_barrier_entry {
            AudioOption::File(path) => assert_eq!(path, "test.wav"),
            _ => panic!("Expected File after Figment layering"),
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

            let ron_string = ron::to_string(&config).unwrap();
            let restored: Config = ron::from_str(&ron_string).unwrap();

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

    // Property test generators
    fn arb_audio_option() -> impl Strategy<Value = AudioOption> {
        // Use safe file paths that won't break RON parsing
        let safe_paths = prop_oneof![
            Just("sound.wav".to_string()),
            Just("beep.mp3".to_string()),
            Just("alert.ogg".to_string()),
            Just("test_audio.wav".to_string()),
        ];

        prop_oneof![
            Just(AudioOption::None),
            safe_paths.prop_map(AudioOption::File),
        ]
    }

    fn arb_overlay_color() -> impl Strategy<Value = OverlayColor> {
        (any::<u8>(), any::<u8>(), any::<u8>()).prop_map(|(r, g, b)| OverlayColor { r, g, b })
    }

    fn arb_audio_feedback_config() -> impl Strategy<Value = AudioFeedbackConfig> {
        (arb_audio_option(), arb_audio_option()).prop_map(|(on_barrier_hit, on_barrier_entry)| {
            AudioFeedbackConfig {
                on_barrier_hit,
                on_barrier_entry,
            }
        })
    }

    fn arb_barrier_config() -> impl Strategy<Value = BarrierConfig> {
        (
            any::<i32>(),
            any::<i32>(),
            any::<i32>(),
            any::<i32>(),
            any::<i32>(),
            any::<i32>(),
            arb_overlay_color(),
            any::<u8>(),
            arb_audio_feedback_config(),
        )
            .prop_map(
                |(
                    x,
                    y,
                    width,
                    height,
                    buffer_zone,
                    push_factor,
                    overlay_color,
                    overlay_alpha,
                    audio_feedback,
                )| BarrierConfig {
                    x,
                    y,
                    width,
                    height,
                    buffer_zone,
                    push_factor,
                    overlay_color,
                    overlay_alpha,
                    audio_feedback,
                },
            )
    }

    fn arb_hud_position() -> impl Strategy<Value = HudPosition> {
        prop_oneof![
            Just(HudPosition::TopLeft),
            Just(HudPosition::TopRight),
            Just(HudPosition::BottomLeft),
            Just(HudPosition::BottomRight),
        ]
    }

    fn arb_hud_config() -> impl Strategy<Value = HudConfig> {
        (any::<bool>(), arb_hud_position(), any::<u8>()).prop_map(
            |(enabled, position, background_alpha)| HudConfig {
                enabled,
                position,
                background_alpha,
            },
        )
    }

    fn arb_hotkey_config() -> impl Strategy<Value = HotkeyConfig> {
        // Use only valid key names that won't break RON parsing
        let valid_keys = prop_oneof![
            Just("F1".to_string()),
            Just("F2".to_string()),
            Just("F12".to_string()),
            Just("A".to_string()),
            Just("B".to_string()),
            Just("Z".to_string()),
            Just("0".to_string()),
            Just("9".to_string()),
        ];

        (any::<bool>(), any::<bool>(), any::<bool>(), valid_keys).prop_map(
            |(ctrl, alt, shift, key)| HotkeyConfig {
                ctrl,
                alt,
                shift,
                key,
            },
        )
    }

    fn arb_config() -> impl Strategy<Value = Config> {
        (
            arb_hotkey_config(),
            arb_barrier_config(),
            arb_hud_config(),
            any::<bool>(),
        )
            .prop_map(|(hotkey, barrier, hud, debug)| Config {
                hotkey,
                barrier,
                hud,
                debug,
            })
    }

    proptest! {
        #[test]
        fn prop_config_roundtrip_serialization(config in arb_config()) {
            // Serialize to RON
            let ron_string = ron::to_string(&config).unwrap();

            // Deserialize back
            let restored: Config = ron::from_str(&ron_string).unwrap();

            // Verify all fields are preserved
            prop_assert_eq!(restored.hotkey.ctrl, config.hotkey.ctrl);
            prop_assert_eq!(restored.hotkey.alt, config.hotkey.alt);
            prop_assert_eq!(restored.hotkey.shift, config.hotkey.shift);
            prop_assert_eq!(restored.hotkey.key, config.hotkey.key);

            prop_assert_eq!(restored.barrier.x, config.barrier.x);
            prop_assert_eq!(restored.barrier.y, config.barrier.y);
            prop_assert_eq!(restored.barrier.width, config.barrier.width);
            prop_assert_eq!(restored.barrier.height, config.barrier.height);
            prop_assert_eq!(restored.barrier.buffer_zone, config.barrier.buffer_zone);
            prop_assert_eq!(restored.barrier.push_factor, config.barrier.push_factor);
            prop_assert_eq!(restored.barrier.overlay_color.r, config.barrier.overlay_color.r);
            prop_assert_eq!(restored.barrier.overlay_color.g, config.barrier.overlay_color.g);
            prop_assert_eq!(restored.barrier.overlay_color.b, config.barrier.overlay_color.b);
            prop_assert_eq!(restored.barrier.overlay_alpha, config.barrier.overlay_alpha);

            prop_assert_eq!(restored.hud.enabled, config.hud.enabled);
            prop_assert_eq!(restored.hud.position, config.hud.position);
            prop_assert_eq!(restored.hud.background_alpha, config.hud.background_alpha);

            prop_assert_eq!(restored.debug, config.debug);

            // Verify audio feedback options
            match (&config.barrier.audio_feedback.on_barrier_hit, &restored.barrier.audio_feedback.on_barrier_hit) {
                (AudioOption::None, AudioOption::None) => {},
                (AudioOption::File(orig), AudioOption::File(rest)) => prop_assert_eq!(orig, rest),
                _ => prop_assert!(false, "Audio option mismatch for on_barrier_hit"),
            }

            match (&config.barrier.audio_feedback.on_barrier_entry, &restored.barrier.audio_feedback.on_barrier_entry) {
                (AudioOption::None, AudioOption::None) => {},
                (AudioOption::File(orig), AudioOption::File(rest)) => prop_assert_eq!(orig, rest),
                _ => prop_assert!(false, "Audio option mismatch for on_barrier_entry"),
            }
        }

        #[test]
        fn prop_config_fallback_to_defaults(
            (complete_config, subset_fields) in (arb_config(), any::<u8>())
        ) {
            // Use a bitmask to determine which fields to include in the "user" config
            // This simulates a partial user config file by creating a partial config that
            // only includes selected fields
            let include_hotkey_ctrl = (subset_fields & 0x01) != 0;
            let include_barrier_x = (subset_fields & 0x10) != 0;
            let include_hud_enabled = (subset_fields & 0x40) != 0;
            let include_debug = (subset_fields & 0x80) != 0;

            let default_config = Config::default();

            // Create a serde_json::Value that represents a partial config file
            let mut user_config_value = serde_json::json!({});

            if include_hotkey_ctrl {
                user_config_value["hotkey"] = serde_json::json!({
                    "ctrl": complete_config.hotkey.ctrl
                });
            }

            if include_barrier_x {
                user_config_value["barrier"] = serde_json::json!({
                    "x": complete_config.barrier.x
                });
            }

            if include_hud_enabled {
                user_config_value["hud"] = serde_json::json!({
                    "enabled": complete_config.hud.enabled
                });
            }

            if include_debug {
                user_config_value["debug"] = serde_json::Value::Bool(complete_config.debug);
            }

            // Create Figment with defaults, then layer the partial user config
            let mut figment = Figment::new().merge(Serialized::defaults(&default_config));

            // Only merge if we have any user overrides
            if user_config_value.as_object().unwrap().len() > 0 {
                figment = figment.merge(Serialized::from(user_config_value, Profile::Default));
            }

            let layered_config: Config = figment.extract().unwrap();

            // Verify that included fields match the user values, missing fields use defaults
            if include_hotkey_ctrl {
                prop_assert_eq!(layered_config.hotkey.ctrl, complete_config.hotkey.ctrl);
                // Other hotkey fields should be defaults since we only set ctrl
                prop_assert_eq!(layered_config.hotkey.alt, default_config.hotkey.alt);
                prop_assert_eq!(layered_config.hotkey.shift, default_config.hotkey.shift);
                prop_assert_eq!(layered_config.hotkey.key, default_config.hotkey.key);
            } else {
                // All hotkey fields should be defaults
                prop_assert_eq!(layered_config.hotkey.ctrl, default_config.hotkey.ctrl);
                prop_assert_eq!(layered_config.hotkey.alt, default_config.hotkey.alt);
                prop_assert_eq!(layered_config.hotkey.shift, default_config.hotkey.shift);
                prop_assert_eq!(layered_config.hotkey.key, default_config.hotkey.key);
            }

            if include_barrier_x {
                prop_assert_eq!(layered_config.barrier.x, complete_config.barrier.x);
                // Other barrier fields should be defaults since we only set x
                prop_assert_eq!(layered_config.barrier.y, default_config.barrier.y);
                prop_assert_eq!(layered_config.barrier.width, default_config.barrier.width);
                prop_assert_eq!(layered_config.barrier.height, default_config.barrier.height);
            } else {
                // All barrier fields should be defaults
                prop_assert_eq!(layered_config.barrier.x, default_config.barrier.x);
                prop_assert_eq!(layered_config.barrier.y, default_config.barrier.y);
                prop_assert_eq!(layered_config.barrier.width, default_config.barrier.width);
                prop_assert_eq!(layered_config.barrier.height, default_config.barrier.height);
            }

            if include_hud_enabled {
                prop_assert_eq!(layered_config.hud.enabled, complete_config.hud.enabled);
                // Other hud fields should be defaults since we only set enabled
                prop_assert_eq!(layered_config.hud.position, default_config.hud.position);
                prop_assert_eq!(layered_config.hud.background_alpha, default_config.hud.background_alpha);
            } else {
                // All hud fields should be defaults
                prop_assert_eq!(layered_config.hud.enabled, default_config.hud.enabled);
                prop_assert_eq!(layered_config.hud.position, default_config.hud.position);
                prop_assert_eq!(layered_config.hud.background_alpha, default_config.hud.background_alpha);
            }

            if include_debug {
                prop_assert_eq!(layered_config.debug, complete_config.debug);
            } else {
                prop_assert_eq!(layered_config.debug, default_config.debug);
            }

            // All other fields should always be defaults since we never override them
            prop_assert_eq!(layered_config.barrier.buffer_zone, default_config.barrier.buffer_zone);
            prop_assert_eq!(layered_config.barrier.push_factor, default_config.barrier.push_factor);
            prop_assert_eq!(layered_config.barrier.overlay_alpha, default_config.barrier.overlay_alpha);
            prop_assert_eq!(layered_config.barrier.overlay_color.r, default_config.barrier.overlay_color.r);
            prop_assert_eq!(layered_config.barrier.overlay_color.g, default_config.barrier.overlay_color.g);
            prop_assert_eq!(layered_config.barrier.overlay_color.b, default_config.barrier.overlay_color.b);
        }
    }
}
