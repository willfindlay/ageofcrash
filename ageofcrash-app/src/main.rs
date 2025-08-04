mod config;
mod config_watcher;
mod hotkey;
mod hud;

use config::{AudioOption, Config};
use config_watcher::{ConfigEvent, ConfigWatcher};
use hotkey::HotkeyDetector;
use hud::Hud;
use mouse_barrier::{process_hook_requests, set_mouse_position_callback, KeyboardHook, MouseBarrier};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn, Level};
use winapi::um::winuser::*;

enum AppEvent {
    HotkeyPressed,
    ConfigReloaded(Config),
    ConfigError(String),
}

struct AppState {
    config: Config,
    barrier_enabled: bool,
    mouse_barrier: Option<MouseBarrier>,
    keyboard_hook: Option<KeyboardHook>,
    hud: Option<Hud>,
    startup_time: std::time::Instant,
}

impl AppState {
    fn new(config: Config) -> Self {
        Self {
            config,
            barrier_enabled: false,
            mouse_barrier: None,
            keyboard_hook: None,
            hud: None,
            startup_time: std::time::Instant::now(),
        }
    }

    fn initialize_barrier(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.mouse_barrier = Some(MouseBarrier::new(
            self.config.barrier.x,
            self.config.barrier.y,
            self.config.barrier.width,
            self.config.barrier.height,
            self.config.barrier.buffer_zone,
            self.config.barrier.push_factor,
            (
                self.config.barrier.overlay_color.r,
                self.config.barrier.overlay_color.g,
                self.config.barrier.overlay_color.b,
            ),
            self.config.barrier.overlay_alpha,
            match &self.config.barrier.audio_feedback.on_barrier_hit {
                AudioOption::None => None,
                AudioOption::File(path) => Some(path.clone()),
            },
            match &self.config.barrier.audio_feedback.on_barrier_entry {
                AudioOption::None => None,
                AudioOption::File(path) => Some(path.clone()),
            },
        ));

        if self.barrier_enabled {
            if let Some(barrier) = &mut self.mouse_barrier {
                barrier.enable()?;
            }
        }

        Ok(())
    }

    fn initialize_hud(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.hud = Some(Hud::new(self.config.hud.clone())?);
        self.update_hud_state();
        Ok(())
    }

    fn update_hud_state(&self) {
        hud::update_global_hud_state(
            self.barrier_enabled,
            self.config.barrier.x,
            self.config.barrier.y,
            self.config.barrier.width,
            self.config.barrier.height,
            self.config.barrier.buffer_zone,
            self.config.barrier.push_factor,
        );
    }

    fn cleanup_hooks(&mut self) {
        // Disable mouse barrier
        if let Some(mut barrier) = self.mouse_barrier.take() {
            let _ = barrier.disable();
        }

        // Disable keyboard hook
        if let Some(mut hook) = self.keyboard_hook.take() {
            let _ = hook.disable();
        }
    }

    fn reload_config(&mut self, new_config: Config) -> Result<(), Box<dyn std::error::Error>> {
        // Skip reloads within first 2 seconds of startup to avoid deployment triggers
        if self.startup_time.elapsed() < std::time::Duration::from_secs(2) {
            info!("Skipping config reload during startup grace period");
            return Ok(());
        }

        info!("Reloading configuration...");

        // Check if barrier is currently enabled before updating
        let was_enabled = self.barrier_enabled;

        // Update the barrier configuration using the existing global state
        if let Some(barrier) = &mut self.mouse_barrier {
            barrier.update_barrier(
                new_config.barrier.x,
                new_config.barrier.y,
                new_config.barrier.width,
                new_config.barrier.height,
                new_config.barrier.buffer_zone,
                new_config.barrier.push_factor,
                (
                    new_config.barrier.overlay_color.r,
                    new_config.barrier.overlay_color.g,
                    new_config.barrier.overlay_color.b,
                ),
                new_config.barrier.overlay_alpha,
                match &new_config.barrier.audio_feedback.on_barrier_hit {
                    AudioOption::None => None,
                    AudioOption::File(path) => Some(path.clone()),
                },
                match &new_config.barrier.audio_feedback.on_barrier_entry {
                    AudioOption::None => None,
                    AudioOption::File(path) => Some(path.clone()),
                },
            );

            // If barrier was enabled, toggle it off and back on to refresh overlay windows
            if was_enabled {
                info!("Refreshing overlay windows with new barrier dimensions");
                barrier.disable()?;
                barrier.enable()?;
            }
        }

        // Check if debug flag changed
        if self.config.debug != new_config.debug {
            if new_config.debug {
                info!("Debug mode enabled (some debug output may require restart to take full effect)");
            } else {
                info!("Debug mode disabled (some debug output may require restart to take full effect)");
            }
        }

        // Update HUD if configuration changed
        if let Some(hud) = &mut self.hud {
            if let Err(e) = hud.update_config(new_config.hud.clone()) {
                warn!("Failed to update HUD configuration: {}", e);
            }
        }

        // Update config
        self.config = new_config;

        // Update HUD state with new barrier configuration
        self.update_hud_state();

        info!("Configuration reloaded successfully");
        log_config(&self.config);

        Ok(())
    }

    fn toggle_barrier(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        if let Some(barrier) = &mut self.mouse_barrier {
            self.barrier_enabled = barrier.toggle()?;

            // Update HUD with new barrier state
            self.update_hud_state();

            // Force HUD refresh
            if let Some(hud) = &mut self.hud {
                if let Err(e) = hud.update_barrier_state(
                    self.barrier_enabled,
                    self.config.barrier.x,
                    self.config.barrier.y,
                    self.config.barrier.width,
                    self.config.barrier.height,
                    self.config.barrier.buffer_zone,
                    self.config.barrier.push_factor,
                ) {
                    warn!("Failed to update HUD barrier state: {}", e);
                }
            }

            Ok(self.barrier_enabled)
        } else {
            Err("Mouse barrier not initialized".into())
        }
    }
}

fn log_config(config: &Config) {
    info!(
        barrier.width = config.barrier.width,
        barrier.height = config.barrier.height,
        barrier.x = config.barrier.x,
        barrier.y = config.barrier.y,
        barrier.buffer_zone = config.barrier.buffer_zone,
        "Barrier area configured"
    );
    info!(
        push_factor = config.barrier.push_factor,
        "Push factor configured"
    );
    info!(
        hotkey = format!(
            "{}{}{}{}",
            if config.hotkey.ctrl { "Ctrl+" } else { "" },
            if config.hotkey.alt { "Alt+" } else { "" },
            if config.hotkey.shift { "Shift+" } else { "" },
            config.hotkey.key
        ),
        "Hotkey configured"
    );
    info!(debug = config.debug, "Debug mode");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Age of Crash Mouse Barrier v0.1.0");
    println!("Loading configuration...");

    let config = Config::load_or_create("config.ron")?;

    // Initialize tracing based on debug flag
    let level = if config.debug {
        Level::DEBUG
    } else {
        Level::INFO
    };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    log_config(&config);

    // Create app state
    let mut state = AppState::new(config.clone());
    state.initialize_barrier()?;
    state.initialize_hud()?;

    // Set up mouse position callback for HUD updates
    set_mouse_position_callback(|x, y| {
        hud::update_mouse_position(x, y);
    });

    // Create event channel for hotkey and config events
    let (tx, rx): (Sender<AppEvent>, Receiver<AppEvent>) = mpsc::channel();

    // Set up config watcher
    let (mut config_watcher, config_rx) = ConfigWatcher::new("config.ron")?;
    config_watcher.start()?;

    // Keep config_watcher alive
    let _config_watcher = Arc::new(Mutex::new(config_watcher));

    // Spawn thread to forward config events to main event channel
    let config_tx = tx.clone();
    std::thread::spawn(move || {
        loop {
            match config_rx.recv() {
                Ok(ConfigEvent::Modified(new_config)) => {
                    if config_tx
                        .send(AppEvent::ConfigReloaded(new_config))
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(ConfigEvent::Error(err)) => {
                    if config_tx.send(AppEvent::ConfigError(err)).is_err() {
                        break;
                    }
                }
                Err(_) => break, // Channel closed
            }
        }
    });

    // Set up keyboard hook
    let hotkey_detector = Arc::new(Mutex::new(
        HotkeyDetector::new(config.hotkey.clone()).ok_or("Failed to create hotkey detector")?,
    ));

    let hotkey_tx = tx.clone();
    let hotkey_detector_clone = hotkey_detector.clone();
    let mut keyboard_hook = KeyboardHook::new(move |vk_code, is_down| {
        if let Ok(mut detector) = hotkey_detector_clone.lock() {
            if detector.handle_key(vk_code, is_down) {
                let _ = hotkey_tx.send(AppEvent::HotkeyPressed);
            }
        }
    });

    keyboard_hook.enable()?;
    state.keyboard_hook = Some(keyboard_hook);

    info!("Keyboard hook enabled. Press the hotkey to toggle the mouse barrier.");
    info!("Config file monitoring enabled. Changes will be applied automatically.");
    info!("Press Ctrl+C to exit.");

    // Windows message loop with integrated event processing
    unsafe {
        loop {
            // Process hook requests from middle mouse monitoring thread
            process_hook_requests();
            
            // Process all pending application events first
            while let Ok(event) = rx.try_recv() {
                match event {
                    AppEvent::HotkeyPressed => match state.toggle_barrier() {
                        Ok(enabled) => {
                            info!(enabled = enabled, "Mouse barrier toggled");
                        }
                        Err(e) => error!(error = %e, "Failed to toggle barrier"),
                    },
                    AppEvent::ConfigReloaded(new_config) => {
                        // Update hotkey detector if hotkey changed
                        if new_config.hotkey != state.config.hotkey {
                            if let Ok(mut detector) = hotkey_detector.lock() {
                                if detector.update_config(new_config.hotkey.clone()).is_some() {
                                    info!("Hotkey updated successfully");
                                } else {
                                    warn!("Failed to update hotkey - invalid key specified");
                                }
                            }
                        }

                        if let Err(e) = state.reload_config(new_config) {
                            error!(error = %e, "Failed to reload configuration");
                        }
                    }
                    AppEvent::ConfigError(err) => {
                        warn!(error = %err, "Config file error");
                    }
                }
            }

            // Handle Windows messages
            let mut msg = std::mem::zeroed();
            let result = PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE);
            if result > 0 {
                if msg.message == WM_QUIT {
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                // No messages, sleep briefly to avoid busy waiting
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }

    // Cleanup hooks
    state.cleanup_hooks();

    Ok(())
}
