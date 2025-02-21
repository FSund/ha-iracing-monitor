#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod iracing_client;
mod frontend;
mod sim_monitor;
mod tray;
mod resources;
mod config;

use config::AppConfig;
use env_logger::{Builder, Target};
use log::LevelFilter;
use frontend::IracingMonitorGui;
use std::fs;
use chrono::Local;

use futures::prelude::stream::StreamExt;
use futures::prelude::sink::SinkExt;
use futures::stream::Stream;
use iced::stream as iced_stream;

#[tokio::main]
async fn main() -> iced::Result {
    // env_logger::init();
    let mut logger_builder = Builder::from_default_env();
    
    // Set external crates to INFO level
    // builder.filter_module("rumqttc", LevelFilter::Info);

    // filter messages from iracing_ha_monitor::sim_monitor
    // logger_builder.filter_module("iracing_ha_monitor::sim_monitor", LevelFilter::Info);
    
    // Keep your application at DEBUG level
    logger_builder.filter_module("iracing_ha_monitor", LevelFilter::Debug);

    // Create logs directory if it doesn't exist
    fs::create_dir_all("logs").expect("Failed to create logs directory");

    // Generate timestamp for log file
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let log_file = format!("logs/iracing_ha_monitor_{}.log", timestamp);

    // Open log file
    let _file = fs::File::create(&log_file)
        .expect("Failed to create log file");
    
    // Apply the configuration
    // let target = Target::Pipe(Box::new(file));
    let target = Target::Stdout;
    logger_builder
        .target(target)
        .init();

    let config = config::get_app_config();

        // Since winit doesn't use gtk on Linux, and we need gtk for
    // the tray icon to show up, we need to spawn a thread
    // where we initialize gtk and create the tray_icon
    #[cfg(target_os = "linux")]
    std::thread::spawn(move || {
        gtk::init().unwrap();

        // On Windows and Linux, an event loop must be running on the thread, on Windows, a win32
        // event loop and on Linux, a gtk event loop. It doesn't need to be the main thread but you
        // have to create the tray icon on the same thread as the event loop.
        // let _tray_event_receiver = tray::create_tray_icon();
        let _tray_icon = tray::new_tray_icon();

        gtk::main();
    });

    // this might lead to duplicate tray icons on Windows
    // try this approach in that case: https://github.com/tauri-apps/tray-icon/blob/b94b96f2df36acfef38d8fda28e4cf2858338eeb/examples/winit.rs#L71-L77
    #[cfg(not(target_os = "linux"))]
    let _tray_icon = tray::new_tray_icon();

    if config.gui {
        // using a daemon is overkill for a plain iced application, but might come in
        // handy when trying to implement a tray icon
        iced::daemon(IracingMonitorGui::title, IracingMonitorGui::update, IracingMonitorGui::view)
            .subscription(IracingMonitorGui::subscription)
            .theme(IracingMonitorGui::theme)
            .run_with(IracingMonitorGui::new)
            .expect("Iced frontend failed");

        // Notes for myself
        // - the frontend should not worry about the config file or config events, it should only keep an internal state that gets
        //   saved to the config file when the user changes something, and updated (via message from the backend) when the config file changes
        // - sim status and tray events should be sent to the frontend via messages from the backend
    } else {
        // run the connect() stream
        let mut stream = Box::pin(connect(config));
        while let Some(event) = stream.next().await {
            log::debug!("event: {:?}", event);
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub enum Event {
    Sim(sim_monitor::Event),
    Tray(tray::TrayEventType),
    ConfigFile(config::Event),
}

pub fn connect(config: AppConfig) -> impl Stream<Item = Event> {
    iced_stream::channel(100, |mut output| async move {
        // pin the streams to the stack
        let mut sim_events = Box::pin(sim_monitor::connect());
        let mut tray_events = Box::pin(tray::tray_subscription());
        let mut config_events = Box::pin(config::watch());

        let mut sim_monitor_connection = None;

        loop {
            tokio::select! {
                Some(event) = sim_events.next() => {
                    log::debug!("sim event: {:?}", event);
                    match event.clone() {
                        sim_monitor::Event::Ready(mut connection) => {
                            // send the current config to the sim monitor
                            connection.send(sim_monitor::Message::UpdateConfig(config.clone()));
                            sim_monitor_connection = Some(connection);
                        }
                        sim_monitor::Event::ConnectedToSim(_state) => {
                            log::debug!("Connected to sim");
                        }
                        sim_monitor::Event::DisconnectedFromSim(_state) => {
                            log::debug!("Disconnected from sim");
                        }
                    }
                    output.send(Event::Sim(event)).await.unwrap();
                }
                Some(event) = tray_events.next() => {
                    log::debug!("tray event: {:?}", event);
                    if let tray::TrayEventType::MenuItemClicked(menu_id) = event.clone() {
                        log::debug!("menu_id: {:?}", menu_id);
                        match menu_id.0.as_str() {
                            "quit" => {
                                log::debug!("Quitting");
                                break;
                            }
                            "options" => {
                                log::debug!("Opening config file");
                                let config_file = config::get_config_path();
                                match open::that(config_file) {
                                    Ok(()) => log::debug!("Opened settings toml"),
                                    Err(err) => log::warn!("Error opening settings toml: {}", err),
                                }
                            }
                            _ => {
                                log::debug!("Unknown menu item clicked: {:?}", menu_id);
                            }
                        }
                    }
                    output.send(Event::Tray(event)).await.unwrap();
                }
                Some(event) = config_events.next() => {
                    log::debug!("config event: {:?}", event);
                    match event.clone() {
                        // config::Event::Created(app_config) => {
                        //     log::debug!("Config created");
                        // }
                        config::Event::Created(app_config) | config::Event::Modified(app_config) => {
                            log::debug!("Config created or modified");
                            
                            // store update config
                            let config = app_config;

                            // send the updated config to the sim monitor
                            if let Some(ref mut connection) = sim_monitor_connection {
                                connection.send(sim_monitor::Message::UpdateConfig(config.clone()));
                            }
                        }
                        config::Event::Deleted(_path) => {
                            log::debug!("Config deleted");
                        }
                    }
                    output.send(Event::ConfigFile(event)).await.unwrap();
                }
            }
        }
    })
}
