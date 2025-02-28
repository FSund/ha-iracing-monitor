use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::RwLock;

use anyhow::Error;
use anyhow::{Context, Result};
use config::{Config, File};
use futures::channel::mpsc;
use futures::prelude::stream::StreamExt;
use futures::stream::Stream;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use serde::Serialize;

use crate::sim_monitor::MqttConfig;

#[derive(Debug)]
enum ConfigError {
    Deserialize,
    Serialize,
    FileEmpty,
    FileNotFound(PathBuf),
    FileRead(PathBuf),
    FileWrite(PathBuf),
    LockError,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppConfig {
    pub gui: bool,
    pub mqtt: MqttConfig,
    pub mqtt_enabled: bool,
}

impl AppConfig {
    pub fn save(&self) -> anyhow::Result<()> {
        let path = get_config_path();
        let toml_string = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(path, toml_string).context("Failed to toml string to config file")?;
        Ok(())
    }
}

#[cfg(debug_assertions)]
fn get_toml_path() -> PathBuf {
    // Use current directory for config in debug mode
    std::env::current_dir().unwrap().join("config.toml")
}

#[cfg(not(debug_assertions))]
fn get_toml_path() -> PathBuf {
    use crate::helpers::get_project_dir;

    // First try executable directory
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe_path| exe_path.parent().map(|p| p.to_path_buf()));

    // If toml file exists in exe dir, use it
    if let Some(path) = exe_dir.filter(|p| p.join("config.toml").exists()) {
        log::debug!("Using executable directory for config: {:?}", path);
        return path.join("config.toml");
    }

    // Otherwise, use ProjectDirs
    let proj_dirs = get_project_dir();
    
    // Create config directory if it doesn't exist
    fs::create_dir_all(proj_dirs.config_dir())
        .expect("Failed to create config directory");

    log::info!("Using user config directory: {:?}", proj_dirs.config_dir());
    proj_dirs.config_dir().join("config.toml")
}

fn config_path() -> &'static RwLock<PathBuf> {
    static CONFIG_PATH: OnceLock<RwLock<PathBuf>> = OnceLock::new();
    CONFIG_PATH.get_or_init(|| {
        let path = get_toml_path();
        log::info!("Using config path: {:?}", path);
        RwLock::new(path)
    })
}

pub fn get_config_path() -> PathBuf {
    config_path().read().unwrap().clone()
}

pub fn get_app_config() -> AppConfig {
    // // refresh if config has been initialized
    // if OnceLock::get(&CONFIG).is_some() {
    //     if let Err(e) = refresh() {
    //         log::warn!("Failed to refresh config: {:?}", e);
    //     };
    // }
    // let config = config()
    //     .read()
    //     .expect("Locking config failed, this indicates a serious threading issue");

    // log::debug!("Got config: {:?}", config);

    // let app_config = config
    //     .clone()
    //     .try_deserialize::<AppConfig>()
    //     .unwrap_or_else(|c| {
    //         log::warn!("Failed to deserialize Config to AppConfig ({c:?})! Using default AppConfig");
    //         AppConfig::default()
    //     });

    // log::debug!("Got app config: {:?}", app_config.clone());
    // app_config

    match get_app_config_with_error() {
        Ok(app_config) => app_config,
        Err(e) => {
            log::warn!("Failed to get app config: {:?}, returning default config", e);
            AppConfig::default()
        }
    }
}

fn get_app_config_with_error() -> Result<AppConfig, ConfigError> {
    if OnceLock::get(&CONFIG).is_some() {
        // refresh if config has been initialized
        refresh()?;
    }
    let config = config()
        .read()
        .map_err(|_| ConfigError::LockError)?;

    log::debug!("Config: {:?}", config);

    let app_config = config
        .clone()
        .try_deserialize::<AppConfig>()
        .map_err(|_| ConfigError::Deserialize)?;

    log::debug!("App config: {:?}", app_config.clone());
    Ok(app_config)
}

static CONFIG: OnceLock<RwLock<Config>> = OnceLock::new();

fn config() -> &'static RwLock<Config> {
    CONFIG.get_or_init(|| {
        log::debug!("Initializing Config");

        // Ensure config file exists
        let config_path = get_config_path();
        if !config_path.exists() {
            log::debug!("Creating config file {config_path:?} with default config");
            std::fs::write(&config_path, "").expect("Failed to create config file");

            // Write default config to file
            let default_app_config = AppConfig::default();
            default_app_config
                .save()
                .expect("Failed to save default config");
        }

        let config = match load() {
            Ok(config) => config,
            Err(e) => {
                log::error!("Failed to load config: {:?}, using default config", e);
                Config::default()
            }
        };
        log::debug!("Config initialized: {:?}", config.clone());
        RwLock::new(config)
    })
}

fn refresh() -> Result<(), ConfigError> {
    log::debug!("Refreshing Config");
    match load() {
        Ok(new_config) => {
            log::debug!("Config refreshed: {:?}", new_config.clone());
            *config().write().unwrap() = new_config;
        }
        Err(e) => {
            // log::warn!("Failed to refresh config: {:?}", e);
            return Err(e);
        }
    }
    Ok(())
}

fn load() -> Result<Config, ConfigError> {
    let path = get_config_path();
    log::debug!("Loading config from {}", path.display());

    // First verify the file exists and has content
    if !path.exists() {
        log::warn!("Config file does not exist at {}", path.display());
        return Err(ConfigError::FileNotFound(path));
    }

    // Read and log the raw content
    let _content = match fs::read_to_string(&path) {
        Ok(content) => {
            if content.trim().is_empty() {
                log::warn!("Config file is empty at {}", path.display());
                return Err(ConfigError::FileEmpty);
            } else {
                log::debug!("Raw config content:\n{}", content);
            }
            content
        }
        Err(e) => {
            log::error!("Failed to read config file: {}", e);
            return Err(ConfigError::FileRead(path));
        }
    };

    Config::builder()
        .add_source(File::from(path))
        .build()
        .map_err(|_| ConfigError::Deserialize)
}

fn show() {
    log::debug!("Current config: {:?}", config().read().unwrap().clone());
}

#[derive(Debug, Clone)]
pub enum Event {
    Created(AppConfig),
    Modified(AppConfig),
    Deleted(PathBuf),
}

pub fn watch() -> impl Stream<Item = Event> {
    let (mut tx, rx) = mpsc::channel(100);
    let file_path = get_config_path();

    // Create the watcher upfront, panic on failure
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                let events = event.paths.into_iter().filter_map(|path| {
                    use notify::EventKind::*;
                    match event.kind {
                        // Create(_) => Some(FileEvent::Created(path)),
                        // Modify(_) => Some(FileEvent::Modified(path)),
                        Create(_) => {
                            log::debug!("Config file created");
                            match get_app_config_with_error() {
                                Ok(app_config) => Some(Event::Created(app_config)),
                                Err(e) => {
                                    log::warn!("Failed to get app config: {:?}", e);
                                    None
                                }
                            }
                        }
                        Modify(_) => {
                            log::debug!("Config file modified");
                            match get_app_config_with_error() {
                                Ok(app_config) => Some(Event::Modified(app_config)),
                                Err(e) => {
                                    log::warn!("Failed to get app config: {:?}", e);
                                    None
                                }
                            }
                        }
                        Remove(_) => Some(Event::Deleted(path)),
                        _ => None,
                    }
                });

                for event in events {
                    let _ = tx.try_send(event);
                }
            }
        },
        notify::Config::default(),
    )
    .expect("Failed to create file watcher");

    // Start watching the path, panic on failure
    watcher
        .watch(&file_path, RecursiveMode::NonRecursive) // use non-recursive since we watch a single file
        .expect("Failed to watch path");

    // Keep watcher alive by storing it in the stream
    futures::stream::unfold((watcher, rx), |(watcher, mut rx)| async move {
        let event = rx.next().await.expect("File watcher channel closed");
        Some((event, (watcher, rx)))
    })
}
