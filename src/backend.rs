use crate::config;
use crate::helpers;
use crate::sim_monitor;
use crate::tray;

use futures::prelude::sink::SinkExt;
use futures::prelude::stream::StreamExt;
use futures::stream::Stream;
use iced::stream as iced_stream;

#[derive(Debug, Clone)]
pub enum Event {
    Sim(sim_monitor::Event),
    Tray(tray::TrayEventType),
    ConfigFile(config::Event),
    Shutdown,
}

pub fn connect() -> impl Stream<Item = Event> {
    let config = Some(config::get_app_config());
    iced_stream::channel(100, |mut output| async move {
        // pin the streams to the stack
        let mut sim_events = Box::pin(sim_monitor::connect(config.clone()));
        let mut tray_events = Box::pin(tray::tray_subscription());
        let mut config_events = Box::pin(config::watch());
        let mut shutdown_events = Box::pin(helpers::shutdown_signals());

        let mut sim_monitor_connection = None;

        // Don't break this loop
        // The connection will be closed by the frontend or the winit/tray-icon event loop as needed
        loop {
            tokio::select! {
                Some(event) = sim_events.next() => {
                    log::debug!("Sim event: {:?}", event);
                    match event.clone() {
                        sim_monitor::Event::Ready(connection) => {
                            // // send the current config to the sim monitor
                            // connection.send(sim_monitor::Message::UpdateConfig(config.clone()));
                            sim_monitor_connection = Some(connection);
                        }
                        sim_monitor::Event::ConnectedToSim(_state) => {
                            log::debug!("Connected to sim");
                        }
                        sim_monitor::Event::DisconnectedFromSim(_state) => {
                            log::debug!("Disconnected from sim");
                        }
                    }
                    output.send(Event::Sim(event.clone())).await.unwrap();
                }
                Some(event) = tray_events.next() => {
                    log::debug!("Tray event: {:?}", event);
                    if let tray::TrayEventType::MenuItemClicked(menu_item) = event.clone() {
                        log::debug!("menu item: {:?}", menu_item);
                        match menu_item {
                            tray::MenuItem::Quit => {
                                log::debug!("Quitting");
                            }
                            tray::MenuItem::Options => {
                                log::debug!("Opening config file");
                                let config_file = config::get_config_path();
                                match open::that(config_file) {
                                    Ok(()) => log::debug!("Opened settings toml"),
                                    Err(err) => log::warn!("Error opening settings toml: {}", err),
                                }
                            }
                            tray::MenuItem::RunOnBoot => {
                                // todo!("Run on boot");
                                helpers::toggle_run_on_boot();
                            }
                        }
                    }
                    output.send(Event::Tray(event)).await.unwrap();
                }
                Some(event) = config_events.next() => {
                    log::debug!("Config event: {:?}", event);
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
                Some(event) = shutdown_events.next() => {
                    log::info!("Received shutdown signal");
                    output.send(event).await.unwrap();
                    // break; // Exit the loop on shutdown
                }
            }
        }
    })
}
