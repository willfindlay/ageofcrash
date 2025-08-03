use crate::config::Config;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tracing::{error, info, warn};

pub enum ConfigEvent {
    Modified(Config),
    Error(String),
}

pub struct ConfigWatcher {
    path: PathBuf,
    tx: Sender<ConfigEvent>,
    watcher_thread: Option<thread::JoinHandle<()>>,
    should_stop: Arc<AtomicBool>,
    poll_interval: Duration,
}

impl ConfigWatcher {
    pub fn new<P: AsRef<Path>>(
        config_path: P,
    ) -> Result<(Self, Receiver<ConfigEvent>), Box<dyn std::error::Error>> {
        let path = config_path.as_ref().to_path_buf();

        // Verify the config file exists and is readable
        if !path.exists() {
            return Err(format!("Config file not found: {:?}", path).into());
        }

        // Try to load it once to verify it's valid
        Config::load_from_file(&path)?;

        let (tx, rx) = mpsc::channel();

        Ok((
            ConfigWatcher {
                path,
                tx,
                watcher_thread: None,
                should_stop: Arc::new(AtomicBool::new(false)),
                poll_interval: Duration::from_millis(500),
            },
            rx,
        ))
    }

    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let path = self.path.clone();
        let tx = self.tx.clone();
        let should_stop = self.should_stop.clone();
        let poll_interval = self.poll_interval;

        let handle = thread::spawn(move || {
            let mut last_modified = None;
            let mut last_change_time = std::time::Instant::now();

            while !should_stop.load(Ordering::Relaxed) {
                match std::fs::metadata(&path) {
                    Ok(metadata) => {
                        if let Ok(modified) = metadata.modified() {
                            if last_modified != Some(modified) {
                                // Debounce rapid changes
                                let now = std::time::Instant::now();
                                if now.duration_since(last_change_time) < Duration::from_millis(100)
                                {
                                    thread::sleep(Duration::from_millis(50));
                                    continue;
                                }

                                last_modified = Some(modified);
                                last_change_time = now;

                                // Small delay to ensure write is complete
                                thread::sleep(Duration::from_millis(50));

                                match Config::load_from_file(&path) {
                                    Ok(config) => {
                                        info!("Config file changed, reloading");
                                        if tx.send(ConfigEvent::Modified(config)).is_err() {
                                            break; // Receiver dropped
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse config file: {}", e);
                                        if tx.send(ConfigEvent::Error(e.to_string())).is_err() {
                                            break; // Receiver dropped
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // Check if it's a sharing violation (common on Windows)
                        #[cfg(windows)]
                        if e.raw_os_error() == Some(32) {
                            // ERROR_SHARING_VIOLATION - file is locked, retry later
                            thread::sleep(Duration::from_millis(100));
                            continue;
                        }

                        error!("Failed to check config file: {}", e);
                        // Send error event for persistent failures
                        if tx
                            .send(ConfigEvent::Error(format!("File access error: {}", e)))
                            .is_err()
                        {
                            break;
                        }
                    }
                }

                thread::sleep(poll_interval);
            }

            info!("Config watcher thread stopping");
        });

        self.watcher_thread = Some(handle);
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(handle) = self.watcher_thread.take() {
            self.should_stop.store(true, Ordering::Relaxed);
            if let Err(e) = handle.join() {
                error!("Failed to join watcher thread: {:?}", e);
            }
        }
    }
}

impl Drop for ConfigWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}
