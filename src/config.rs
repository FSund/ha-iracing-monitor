use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::RwLock;

use anyhow::{Context, Result};
use config::{Config, File};
use futures::channel::mpsc;
use futures::prelude::stream::StreamExt;
use futures::stream::Stream;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use serde::Serialize;

use crate::sim_monitor::MqttConfig;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppConfig {
    pub mqtt: MqttConfig,
    pub mqtt_enabled: bool,
}

impl AppConfig {
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
            default_app_config
                .save()
                .expect("Failed to save default config");
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
    )
    .expect("Failed to create file watcher");

    // Start watching the path, panic on failure
    watcher
        .watch(&file_path, RecursiveMode::Recursive)
        .expect("Failed to watch path");

    // Keep watcher alive by storing it in the stream
    futures::stream::unfold((watcher, rx), |(watcher, mut rx)| async move {
        let event = rx.next().await.expect("File watcher channel closed");
        Some((event, (watcher, rx)))
    })
}
