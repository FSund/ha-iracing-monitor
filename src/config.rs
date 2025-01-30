use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::OnceLock;
use std::sync::RwLock;
use std::time::Duration;
use std::path::{Path, PathBuf};
use std::fs;

use config::{Config, File};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use iced::futures::{SinkExt, Stream};
use serde::Deserialize;
use serde::Serialize;
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub mqtt_host: String,
    pub mqtt_port: u16,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mqtt_host: "localhost".to_string(),
            mqtt_port: 1883,
        }
    }
}

impl AppConfig {
    pub fn load_or_create(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        // Create default config if file doesn't exist
        if !path.exists() {
            let config = Self::default();
            config.save(path)?;
            return Ok(config);
        }

        // Read existing config
        let content = fs::read_to_string(path)?;
        let existing_table: toml::Table = toml::from_str(&content)?;

        // Create a new config with default values
        let default_config = Self::default();
        let default_table = toml::to_string(&default_config)?.parse::<toml::Table>()?;

        // Merge existing values with defaults
        let merged_table = Self::merge_tables(existing_table, default_table);
        
        // Convert merged table back to config
        let merged_config: Self = toml::from_str(&toml::to_string(&merged_table)?)?;
        
        // Save the merged config back to file to add any missing fields
        merged_config.save(path)?;

        Ok(merged_config)
    }

    fn merge_tables(existing: toml::Table, default: toml::Table) -> toml::Table {
        let mut merged = toml::Table::new();

        // Add all fields from default config
        for (key, default_value) in default.iter() {
            if let Some(existing_value) = existing.get(key) {
                // Use existing value if present
                merged.insert(key.clone(), existing_value.clone());
            } else {
                // Use default value if field is missing
                merged.insert(key.clone(), default_value.clone());
            }
        }

        merged
    }

    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let toml_string = toml::to_string_pretty(self)?;
        fs::write(path, toml_string)?;
        Ok(())
    }
}

fn get_config_path() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .expect("Failed to get executable path")
        .parent()
        .expect("Failed to get executable directory")
        .to_path_buf();
        
    // // Create the directory if it doesn't exist
    // std::fs::create_dir_all(&exe_dir).unwrap_or_else(|e| {
    //     log::warn!("Failed to create config directory: {}", e);
    // });
    
    exe_dir.join("config.toml")
}

pub fn get_app_config() -> AppConfig {
    settings()
        .read()
        .unwrap()
        .clone()
        .try_deserialize::<AppConfig>()
        .unwrap()
}

fn settings() -> &'static RwLock<Config> {
    static CONFIG: OnceLock<RwLock<Config>> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let settings = load();

        RwLock::new(settings)
    })
}

fn refresh() {
    *settings().write().unwrap() = load();
}

fn load() -> Config {
    let config_path = get_config_path();

    // create file if it doesn't exist
    if !config_path.exists() {
        log::debug!("Creating config file {config_path:?}");
        std::fs::write(&config_path, "").expect("Failed to create config file");

        // Write default config to file
        // let default_app_config = AppConfig::default();
        // toml::to_string(&default_app_config).expect("Failed to serialize default config");
        AppConfig::load_or_create(&config_path).expect("Failed to load or create config");
    }

    Config::builder()
        .add_source(File::from(config_path))
        .build()
        .unwrap()
}

fn show() {
    println!(
        " * Settings :: \n\x1b[31m{:?}\x1b[0m",
        settings()
            .read()
            .unwrap()
            .clone()
            .try_deserialize::<HashMap<String, String>>()
            .unwrap()
    );
}

#[derive(Debug, Clone)]
pub enum Event {
    ConfigurationUpdated(AppConfig),
    WatchError(String),
}

pub fn watch_config() -> impl Stream<Item = Event> {
    iced::stream::channel(100, |mut output| async move {
        // Create a channel to receive the file system events
        let (tx, rx) = mpsc::channel();

        // Create the file system watcher
        let mut watcher: RecommendedWatcher = Watcher::new(
            tx,
            notify::Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .expect("Failed to create watcher");

        // Watch the config file
        let config_path = get_config_path();
        log::debug!("Watching config file {config_path:?}");
        watcher
            .watch(
                &config_path,
                RecursiveMode::NonRecursive,
            )
            .expect("Failed to watch path");

        loop {
            match rx.recv() {
                Ok(Ok(notify::Event {
                    kind: notify::event::EventKind::Modify(_),
                    ..
                })) => {
                    log::info!("Config file modified; refreshing configuration ...");
                    refresh();
                    show();
                    output
                        .send(Event::ConfigurationUpdated(get_app_config()))
                        .await
                        .expect("Failed to send event");
                }

                Ok(Err(e)) => {
                    log::error!("Watch error: {e:?}");
                    output
                        .send(Event::WatchError(e.to_string()))
                        .await
                        .expect("Failed to send error event");
                }

                Err(e) => {
                    log::error!("Channel error: {e:?}");
                    break;
                }

                _ => {
                    // Ignore other events
                }
            }
        }
    })
}