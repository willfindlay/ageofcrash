mod config;
mod hotkey;

use config::Config;
use hotkey::HotkeyDetector;
use mouse_barrier::{MouseBarrier, KeyboardHook};
use std::sync::{Arc, Mutex};
use tracing::{info, Level};
use tracing_subscriber;
use winapi::um::winuser::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Age of Crash Mouse Barrier v0.1.0");
    println!("Loading configuration...");

    let config = Config::load_or_create("config.ron")?;
    
    // Initialize tracing based on debug flag
    let level = if config.debug { Level::DEBUG } else { Level::INFO };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    info!(
        barrier.width = config.barrier.width,
        barrier.height = config.barrier.height,
        barrier.x = config.barrier.x,
        barrier.y = config.barrier.y,
        barrier.buffer_zone = config.barrier.buffer_zone,
        "Barrier area configured"
    );
    info!(push_factor = config.barrier.push_factor, "Push factor configured");
    info!(
        hotkey = format!("{}{}{}{}",
            if config.hotkey.ctrl { "Ctrl+" } else { "" },
            if config.hotkey.alt { "Alt+" } else { "" },
            if config.hotkey.shift { "Shift+" } else { "" },
            config.hotkey.key),
        "Hotkey configured"
    );
    info!(debug = config.debug, "Debug mode");

    let mouse_barrier = MouseBarrier::new(
        config.barrier.x,
        config.barrier.y,
        config.barrier.width,
        config.barrier.height,
        config.barrier.buffer_zone,
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
                            info!(enabled = enabled, "Mouse barrier toggled");
                        }
                        Err(e) => tracing::error!(error = %e, "Failed to toggle barrier"),
                    }
                }
            }
        }
    });

    keyboard_hook.enable()?;
    info!("Keyboard hook enabled. Press the hotkey to toggle the mouse barrier.");
    info!("Press Ctrl+C to exit.");

    unsafe {
        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}