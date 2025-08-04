use crate::config::{vk_code_from_string, HotkeyConfig};
use winapi::um::winuser::*;

pub struct HotkeyDetector {
    config: HotkeyConfig,
    target_vk: u32,
    ctrl_pressed: bool,
    alt_pressed: bool,
    shift_pressed: bool,
}

impl HotkeyDetector {
    pub fn new(config: HotkeyConfig) -> Option<Self> {
        let target_vk = vk_code_from_string(&config.key)?;

        Some(Self {
            config,
            target_vk,
            ctrl_pressed: false,
            alt_pressed: false,
            shift_pressed: false,
        })
    }

    pub fn handle_key(&mut self, vk_code: u32, is_down: bool) -> bool {
        match vk_code {
            x if x == VK_CONTROL as u32 || x == VK_LCONTROL as u32 || x == VK_RCONTROL as u32 => {
                self.ctrl_pressed = is_down;
            }
            x if x == VK_MENU as u32 || x == VK_LMENU as u32 || x == VK_RMENU as u32 => {
                self.alt_pressed = is_down;
            }
            x if x == VK_SHIFT as u32 || x == VK_LSHIFT as u32 || x == VK_RSHIFT as u32 => {
                self.shift_pressed = is_down;
            }
            _ => {
                if vk_code == self.target_vk && is_down {
                    return self.is_hotkey_pressed();
                }
            }
        }

        false
    }

    pub fn update_config(&mut self, new_config: HotkeyConfig) -> Option<()> {
        let target_vk = vk_code_from_string(&new_config.key)?;

        self.config = new_config;
        self.target_vk = target_vk;

        // Reset modifier states to avoid confusion
        self.ctrl_pressed = false;
        self.alt_pressed = false;
        self.shift_pressed = false;

        Some(())
    }

    fn is_hotkey_pressed(&self) -> bool {
        self.ctrl_pressed == self.config.ctrl
            && self.alt_pressed == self.config.alt
            && self.shift_pressed == self.config.shift
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HotkeyConfig;

    fn create_test_config(ctrl: bool, alt: bool, shift: bool, key: &str) -> HotkeyConfig {
        HotkeyConfig {
            ctrl,
            alt,
            shift,
            key: key.to_string(),
        }
    }

    #[test]
    fn test_hotkey_detector_creation_valid_key() {
        let config = create_test_config(true, false, false, "F12");
        let detector = HotkeyDetector::new(config.clone());
        
        assert!(detector.is_some());
        let detector = detector.unwrap();
        assert_eq!(detector.config, config);
        assert_eq!(detector.target_vk, VK_F12 as u32);
        assert!(!detector.ctrl_pressed);
        assert!(!detector.alt_pressed);
        assert!(!detector.shift_pressed);
    }

    #[test]
    fn test_hotkey_detector_creation_invalid_key() {
        let config = create_test_config(true, false, false, "INVALID_KEY");
        let detector = HotkeyDetector::new(config);
        
        assert!(detector.is_none());
    }

    #[test]
    fn test_handle_key_ctrl_press_release() {
        let config = create_test_config(true, false, false, "F12");
        let mut detector = HotkeyDetector::new(config).unwrap();

        // Press Ctrl
        let result = detector.handle_key(VK_CONTROL as u32, true);
        assert!(!result); // Should not trigger hotkey yet
        assert!(detector.ctrl_pressed);

        // Release Ctrl
        let result = detector.handle_key(VK_CONTROL as u32, false);
        assert!(!result); // Should not trigger hotkey
        assert!(!detector.ctrl_pressed);
    }

    #[test]
    fn test_handle_key_alt_press_release() {
        let config = create_test_config(false, true, false, "F12");
        let mut detector = HotkeyDetector::new(config).unwrap();

        // Press Alt (VK_MENU)
        let result = detector.handle_key(VK_MENU as u32, true);
        assert!(!result);
        assert!(detector.alt_pressed);

        // Release Alt
        let result = detector.handle_key(VK_MENU as u32, false);
        assert!(!result);
        assert!(!detector.alt_pressed);
    }

    #[test]
    fn test_handle_key_shift_press_release() {
        let config = create_test_config(false, false, true, "F12");
        let mut detector = HotkeyDetector::new(config).unwrap();

        // Press Shift
        let result = detector.handle_key(VK_SHIFT as u32, true);
        assert!(!result);
        assert!(detector.shift_pressed);

        // Release Shift
        let result = detector.handle_key(VK_SHIFT as u32, false);
        assert!(!result);
        assert!(!detector.shift_pressed);
    }

    #[test]
    fn test_handle_key_left_right_modifiers() {
        let config = create_test_config(true, true, true, "F12");
        let mut detector = HotkeyDetector::new(config).unwrap();

        // Test left modifiers
        detector.handle_key(VK_LCONTROL as u32, true);
        assert!(detector.ctrl_pressed);

        detector.handle_key(VK_LMENU as u32, true);
        assert!(detector.alt_pressed);

        detector.handle_key(VK_LSHIFT as u32, true);
        assert!(detector.shift_pressed);

        // Test right modifiers (should also work)
        detector.handle_key(VK_LCONTROL as u32, false);
        detector.handle_key(VK_RCONTROL as u32, true);
        assert!(detector.ctrl_pressed);

        detector.handle_key(VK_LMENU as u32, false);
        detector.handle_key(VK_RMENU as u32, true);
        assert!(detector.alt_pressed);

        detector.handle_key(VK_LSHIFT as u32, false);
        detector.handle_key(VK_RSHIFT as u32, true);
        assert!(detector.shift_pressed);
    }

    #[test]
    fn test_simple_hotkey_trigger() {
        let config = create_test_config(true, false, false, "F12");
        let mut detector = HotkeyDetector::new(config).unwrap();

        // Press Ctrl
        detector.handle_key(VK_CONTROL as u32, true);
        
        // Press F12 - should trigger hotkey
        let result = detector.handle_key(VK_F12 as u32, true);
        assert!(result);
    }

    #[test]
    fn test_complex_hotkey_trigger() {
        let config = create_test_config(true, true, true, "A");
        let mut detector = HotkeyDetector::new(config).unwrap();

        // Press all modifiers
        detector.handle_key(VK_CONTROL as u32, true);
        detector.handle_key(VK_MENU as u32, true);
        detector.handle_key(VK_SHIFT as u32, true);
        
        // Press A - should trigger hotkey
        let result = detector.handle_key(0x41, true); // 'A' key
        assert!(result);
    }

    #[test]
    fn test_hotkey_not_triggered_wrong_modifiers() {
        let config = create_test_config(true, false, false, "F12");
        let mut detector = HotkeyDetector::new(config).unwrap();

        // Press Alt instead of Ctrl
        detector.handle_key(VK_MENU as u32, true);
        
        // Press F12 - should NOT trigger hotkey
        let result = detector.handle_key(VK_F12 as u32, true);
        assert!(!result);
    }

    #[test]
    fn test_hotkey_not_triggered_missing_modifier() {
        let config = create_test_config(true, true, false, "F12");
        let mut detector = HotkeyDetector::new(config).unwrap();

        // Press only Ctrl (missing Alt)
        detector.handle_key(VK_CONTROL as u32, true);
        
        // Press F12 - should NOT trigger hotkey
        let result = detector.handle_key(VK_F12 as u32, true);
        assert!(!result);
    }

    #[test]
    fn test_hotkey_not_triggered_extra_modifier() {
        let config = create_test_config(true, false, false, "F12");
        let mut detector = HotkeyDetector::new(config).unwrap();

        // Press Ctrl and Shift (extra modifier)
        detector.handle_key(VK_CONTROL as u32, true);
        detector.handle_key(VK_SHIFT as u32, true);
        
        // Press F12 - should NOT trigger hotkey because Shift is pressed but not required
        let result = detector.handle_key(VK_F12 as u32, true);
        assert!(!result);
    }

    #[test]
    fn test_hotkey_not_triggered_on_key_release() {
        let config = create_test_config(true, false, false, "F12");
        let mut detector = HotkeyDetector::new(config).unwrap();

        // Press Ctrl
        detector.handle_key(VK_CONTROL as u32, true);
        
        // Release F12 - should NOT trigger hotkey
        let result = detector.handle_key(VK_F12 as u32, false);
        assert!(!result);
    }

    #[test]
    fn test_hotkey_not_triggered_wrong_key() {
        let config = create_test_config(true, false, false, "F12");
        let mut detector = HotkeyDetector::new(config).unwrap();

        // Press Ctrl
        detector.handle_key(VK_CONTROL as u32, true);
        
        // Press F11 instead of F12 - should NOT trigger hotkey
        let result = detector.handle_key(VK_F11 as u32, true);
        assert!(!result);
    }

    #[test]
    fn test_update_config_valid_key() {
        let initial_config = create_test_config(true, false, false, "F12");
        let mut detector = HotkeyDetector::new(initial_config).unwrap();

        let new_config = create_test_config(false, true, false, "F1");
        let result = detector.update_config(new_config.clone());
        
        assert!(result.is_some());
        assert_eq!(detector.config, new_config);
        assert_eq!(detector.target_vk, VK_F1 as u32);
        
        // Modifier states should be reset
        assert!(!detector.ctrl_pressed);
        assert!(!detector.alt_pressed);
        assert!(!detector.shift_pressed);
    }

    #[test]
    fn test_update_config_invalid_key() {
        let initial_config = create_test_config(true, false, false, "F12");
        let mut detector = HotkeyDetector::new(initial_config.clone()).unwrap();

        let new_config = create_test_config(false, true, false, "INVALID_KEY");
        let result = detector.update_config(new_config);
        
        assert!(result.is_none());
        // Config should remain unchanged
        assert_eq!(detector.config, initial_config);
        assert_eq!(detector.target_vk, VK_F12 as u32);
    }

    #[test]
    fn test_update_config_resets_modifier_states() {
        let initial_config = create_test_config(true, false, false, "F12");
        let mut detector = HotkeyDetector::new(initial_config).unwrap();

        // Set some modifier states
        detector.handle_key(VK_CONTROL as u32, true);
        detector.handle_key(VK_SHIFT as u32, true);
        assert!(detector.ctrl_pressed);
        assert!(detector.shift_pressed);

        // Update config
        let new_config = create_test_config(false, true, false, "F1");
        detector.update_config(new_config);
        
        // Modifier states should be reset
        assert!(!detector.ctrl_pressed);
        assert!(!detector.alt_pressed);
        assert!(!detector.shift_pressed);
    }

    #[test]
    fn test_no_modifier_hotkey() {
        let config = create_test_config(false, false, false, "F12");
        let mut detector = HotkeyDetector::new(config).unwrap();

        // Press F12 without any modifiers - should trigger hotkey
        let result = detector.handle_key(VK_F12 as u32, true);
        assert!(result);
    }

    #[test]
    fn test_alphabet_keys() {
        for (letter, vk_code) in [
            ("A", 0x41), ("B", 0x42), ("C", 0x43), ("Z", 0x5A)
        ] {
            let config = create_test_config(true, false, false, letter);
            let mut detector = HotkeyDetector::new(config).unwrap();
            
            detector.handle_key(VK_CONTROL as u32, true);
            let result = detector.handle_key(vk_code, true);
            assert!(result, "Hotkey should trigger for {}", letter);
        }
    }

    #[test]
    fn test_function_keys() {
        for (key_name, vk_code) in [
            ("F1", VK_F1), ("F5", VK_F5), ("F10", VK_F10), ("F12", VK_F12)
        ] {
            let config = create_test_config(true, false, false, key_name);
            let mut detector = HotkeyDetector::new(config).unwrap();
            
            detector.handle_key(VK_CONTROL as u32, true);
            let result = detector.handle_key(vk_code as u32, true);
            assert!(result, "Hotkey should trigger for {}", key_name);
        }
    }

    #[test]
    fn test_number_keys() {
        for (digit, vk_code) in [
            ("0", 0x30), ("1", 0x31), ("5", 0x35), ("9", 0x39)
        ] {
            let config = create_test_config(true, false, false, digit);
            let mut detector = HotkeyDetector::new(config).unwrap();
            
            detector.handle_key(VK_CONTROL as u32, true);
            let result = detector.handle_key(vk_code, true);
            assert!(result, "Hotkey should trigger for {}", digit);
        }
    }
}
