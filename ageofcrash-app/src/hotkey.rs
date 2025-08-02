use crate::config::{HotkeyConfig, vk_code_from_string};
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