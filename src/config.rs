use std::sync::OnceLock;
use std::sync::RwLock;
use std::time::Duration;
use std::path::{Path, PathBuf};
use std::fs;

// use tokio::sync::mpsc;
use futures::channel::mpsc;
use config::{Config, File};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use futures::stream::Stream;
use futures::prelude::sink::SinkExt;
use futures::prelude::stream::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use anyhow::{Context, Result};
use iced::stream as iced_stream;

use crate::sim_monitor::MqttConfig;

// use rumqttc::{AsyncClient, MqttOptions, QoS};
// use serde::Serialize;
// use thiserror::Error;

// #[derive(Error, Debug)]
// pub enum ConfigError {
//     #[error("Failed to load config: {0}")]
//     LoadError(#[from] config::ConfigError),
//     #[error("Failed to watch config: {0}")]
//     WatchError(#[from] notify::Error),
//     #[error("Failed to access config directory: {0}")]
//     IoError(#[from] std::io::Error),
// }

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppConfig {
    pub mqtt: MqttConfig,
    pub mqtt_enabled: bool,
}

// impl Default for AppConfig {
//     fn default() -> Self {
//         Self {
//             mqtt: MqttConfig::default(),
//         }
//     }
// }

impl AppConfig {
    // fn load_or_create(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
    //     // Create default config if file doesn't exist
    //     if !path.exists() {
    //         let config = Self::default();
    //         config.save(path)?;
    //         return Ok(config);
    //     }

    //     // Read existing config
    //     let content = fs::read_to_string(path)?;
    //     let existing_table: toml::Table = toml::from_str(&content)?;

    //     // Create a new config with default values
    //     let default_config = Self::default();
    //     let default_table = toml::to_string(&default_config)?.parse::<toml::Table>()?;

    //     // Merge existing values with defaults
    //     let (merged_table, missing_values) = Self::merge_tables(existing_table, default_table);
        
    //     // Convert merged table back to config
    //     let merged_config: Self = toml::from_str(&toml::to_string(&merged_table)?)?;
        
    //     // Save the merged config back to file to add any missing fields
    //     if missing_values {
    //         merged_config.save(path)?;
    //     }

    //     Ok(merged_config)
    // }

    // fn wat() {
    //     let app_config = 
    //         settings()
    //         .read()
    //         .unwrap()
    //         .clone()
    //         .try_deserialize::<AppConfig>()
    //         .unwrap();
        
    //     // convert to toml
    //     let toml_string = toml::to_string_pretty(&app_config).unwrap();
    // }

    // fn merge_tables(existing: toml::Table, default: toml::Table) -> (toml::Table, bool) {
    //     let mut merged = toml::Table::new();

    //     // Add all fields from default config
    //     let mut missing_values = false;
    //     for (key, default_value) in default.iter() {
    //         if let Some(existing_value) = existing.get(key) {
    //             // Use existing value if present
    //             merged.insert(key.clone(), existing_value.clone());
    //         } else {
    //             // Use default value if field is missing
    //             log::debug!("Adding default value for {key}");
    //             missing_values = true;
    //             merged.insert(key.clone(), default_value.clone());
    //         }
    //     }

    //     (merged, missing_values)
    // }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = get_config_path();
        let toml_string = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(path, toml_string).context("Failed to toml string to config file")?;
        Ok(())
    }
}

fn get_config_path() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .expect("Failed to get executable path")
        .parent()
        .expect("Failed to get executable directory")
        .to_path_buf();
    
    exe_dir.join("config.toml")
}

pub fn get_app_config() -> AppConfig {
    let settings = settings()
        .read()
        .expect("Failed to read settings")
        .clone()
        .try_deserialize::<AppConfig>()
        .unwrap_or_else(|_| {
            log::warn!("Failed to deserialize settings! Using default config");
            AppConfig::default()
        });

    log::debug!("Settings: {:?}", settings.clone());
    settings
}

fn settings() -> &'static RwLock<Config> {
    static CONFIG: OnceLock<RwLock<Config>> = OnceLock::new();
    CONFIG.get_or_init(|| {
        log::debug!("Initializing settings");

        // Ensure config file exists
        let config_path = get_config_path();
        if !config_path.exists() {
            log::debug!("Creating config file {config_path:?} with default config");
            std::fs::write(&config_path, "").expect("Failed to create config file");

            // Write default config to file
            let default_app_config = AppConfig::default();
            default_app_config.save().expect("Failed to save default config");
        }
        
        let settings = load();
        RwLock::new(settings)
    })
}

fn refresh() {
    *settings().write().unwrap() = load();
}

fn load() -> Config {
    Config::builder()
        .add_source(File::from(get_config_path()))
        .build()
        .unwrap()
}

fn show() {
    log::debug!("Current settings: {:?}", settings().read().unwrap().clone());
}

// #[derive(Debug, Clone)]
// pub enum Event {
//     ConfigUpdated(AppConfig),
//     WatchError(String),
// }

// pub fn watch_config() -> impl Stream<Item = Event> {
//     iced_stream::channel(100, |mut output| async move {
//         // Create a channel to receive the file system events
//         let (tx, mut rx) = mpsc::channel(100);

//         // Create the file system watcher
//         let mut watcher: RecommendedWatcher = Watcher::new(
//             move |event| {
//                 let mut sender = tx.clone();
        
//                 // Create a new Tokio runtime for handling the async send
//                 let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
//                 log::debug!("Sending config event {event:?} to channel");
//                 rt.spawn(async move {
//                     if let Err(e) = sender.send(event).await {
//                         log::error!("Failed to send config event: {e}");
//                     } else {
//                         log::debug!("Config event sent to channel");
//                     }
//                 });
//             },
//             notify::Config::default().with_poll_interval(Duration::from_secs(2)),
//         )
//         .expect("Failed to create watcher");

//         // Watch the config file
//         let config_path = get_config_path();
//         log::debug!("Watching config file {config_path:?}");
//         watcher
//             .watch(
//                 &config_path,
//                 RecursiveMode::NonRecursive,
//             )
//             .expect("Failed to watch path");

//         loop {
//             tokio::select! {
//                 Some(res) = rx.next() => {
//                     match res {
//                         Ok(notify::Event {
//                             kind: notify::event::EventKind::Modify(_),
//                             ..
//                         }) => {
//                             log::info!("Config file modified; refreshing configuration ...");
//                             refresh();
//                             show();
//                             output
//                                 .send(Event::ConfigUpdated(get_app_config()))
//                                 .await
//                                 .expect("Failed to send event");
//                         }

//                         Err(e) => {
//                             log::error!("Watch error: {e:?}");
//                             output
//                                 .send(Event::WatchError(e.to_string()))
//                                 .await
//                                 .expect("Failed to send error event");
//                         }
                        
//                         _ => {
//                             // Ignore other events
//                         }
//                     }
//                 }
//                 else => {
//                     log::error!("Channel closed");
//                     break;
//                 }
//             }
//         }
//     })
// }





#[derive(Debug, Clone)]
pub enum Event {
    Created(AppConfig),
    Modified(AppConfig),
    Deleted(PathBuf),
}

pub fn watch() -> impl Stream<Item = Event> {
    let (tx, rx) = mpsc::unbounded(); // not sure why unbounded is used here (I don't know what it does)
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
                        Create(_) => Some(Event::Created(get_app_config())),
                        Modify(_) => Some(Event::Modified(get_app_config())),
                        Remove(_) => Some(Event::Deleted(path)),
                        _ => None,
                    }
                });

                for event in events {
                    let _ = tx.unbounded_send(event);
                }
            }
        },
        notify::Config::default(),
    ).expect("Failed to create file watcher");

    // Start watching the path, panic on failure
    watcher.watch(&file_path, RecursiveMode::Recursive)
        .expect("Failed to watch path");

    // Keep watcher alive by storing it in the stream
    futures::stream::unfold((watcher, rx), |(watcher, mut rx)| async move {
        let event = rx.next().await.expect("File watcher channel closed");
        Some((event, (watcher, rx)))
    })
}