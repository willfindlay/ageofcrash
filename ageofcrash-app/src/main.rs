mod config;
mod hotkey;

use config::Config;
use hotkey::HotkeyDetector;
use mouse_barrier::{MouseBarrier, KeyboardHook};
use std::sync::{Arc, Mutex};
use winapi::um::winuser::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Age of Crash Mouse Barrier v0.1.0");
    println!("Loading configuration...");

    let config = Config::load_or_create("config.ron")?;
    println!("Barrier area: {}x{} at ({}, {})", 
             config.barrier.width, config.barrier.height, 
             config.barrier.x, config.barrier.y);
    println!("Push factor: {}", config.barrier.push_factor);
    println!("Hotkey: {}{}{}{}",
             if config.hotkey.ctrl { "Ctrl+" } else { "" },
             if config.hotkey.alt { "Alt+" } else { "" },
             if config.hotkey.shift { "Shift+" } else { "" },
             config.hotkey.key);

    let mouse_barrier = MouseBarrier::new(
        config.barrier.x,
        config.barrier.y,
        config.barrier.width,
        config.barrier.height,
        config.barrier.push_factor,
    );

    let hotkey_detector = Arc::new(Mutex::new(
        HotkeyDetector::new(config.hotkey)
            .ok_or("Failed to create hotkey detector")?
    ));

    let barrier_handle = Arc::new(Mutex::new(mouse_barrier));
    let barrier_clone = barrier_handle.clone();

    let mut keyboard_hook = KeyboardHook::new(move |vk_code, is_down| {
        if let Ok(mut detector) = hotkey_detector.lock() {
            if detector.handle_key(vk_code, is_down) {
                if let Ok(mut barrier) = barrier_clone.lock() {
                    match barrier.toggle() {
                        Ok(enabled) => {
                            if enabled {
                                println!("Mouse barrier ENABLED");
                            } else {
                                println!("Mouse barrier DISABLED");
                            }
                        }
                        Err(e) => eprintln!("Failed to toggle barrier: {}", e),
                    }
                }
            }
        }
    });

    keyboard_hook.enable()?;
    println!("Keyboard hook enabled. Press the hotkey to toggle the mouse barrier.");
    println!("Press Ctrl+C to exit.");

    unsafe {
        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}