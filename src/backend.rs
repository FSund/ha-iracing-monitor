use crate::config;
use crate::sim_monitor;
use crate::tray;
use crate::UserEvent;

use config::AppConfig;

use futures::prelude::sink::SinkExt;
use futures::prelude::stream::StreamExt;
use futures::stream::Stream;
use iced::stream as iced_stream;
use winit::event_loop::EventLoopProxy;

#[derive(Debug, Clone)]
pub enum Event {
    Sim(sim_monitor::Event),
    Tray(tray::TrayEventType),
    ConfigFile(config::Event),
}

pub fn connect(
    winit_event_loop_proxy: Option<EventLoopProxy<UserEvent>>,
) -> impl Stream<Item = Event> {
    let config = Some(config::get_app_config());
    iced_stream::channel(100, |mut output| async move {
        // pin the streams to the stack
        let mut sim_events = Box::pin(sim_monitor::connect(config.clone()));
        let mut tray_events = Box::pin(tray::tray_subscription());
        let mut config_events = Box::pin(config::watch());

        let mut sim_monitor_connection = None;

        loop {
            tokio::select! {
                Some(event) = sim_events.next() => {
                    log::debug!("sim event: {:?}", event);
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

                    // forward to winit/tray icon
                    log::debug!("Sending sim event to winit");
                    if let Some(ref event_loop_proxy) = winit_event_loop_proxy {
                        if let Err(e) = event_loop_proxy.send_event(UserEvent::SimMonitorEvent(event)) {
                            log::warn!("Failed to send event to winit: {}", e);
                        }
                    }
                }
                Some(event) = tray_events.next() => {
                    log::debug!("tray event: {:?}", event);
                    if let tray::TrayEventType::MenuItemClicked(menu_id) = event.clone() {
                        log::debug!("menu_id: {:?}", menu_id);
                        match menu_id.0.as_str() {
                            "quit" => {
                                log::debug!("Quitting");
                                if let Some(ref event_loop_proxy) = winit_event_loop_proxy {
                                    if let Err(e) = event_loop_proxy.send_event(UserEvent::Shutdown) {
                                        panic!("Failed to send shutdown event to winit: {}", e);
                                    }
                                }
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
