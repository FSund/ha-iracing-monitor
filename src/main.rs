// Use windows_subsystem for release builds to hide console
// This also disables stdout logging
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backend;
mod config;
mod frontend;
mod helpers;
mod iracing_client;
mod logging;
mod resources;
mod sim_monitor;
mod tray;

use anyhow::Context;
use frontend::IracingMonitorGui;
use futures::prelude::stream::StreamExt;
use logging::setup_logging;
use winit::{application::ApplicationHandler, event_loop::EventLoop};

#[derive(Debug)]
enum UserEvent {
    TrayIconEvent(tray_icon::TrayIconEvent),
    MenuEvent(tray_icon::menu::MenuEvent),
    Shutdown,
    SimMonitorEvent(sim_monitor::Event),
}

struct Application {
    tray_icon: Box<dyn tray::TrayIconInterface>,
}

impl Application {
    fn new() -> Self {
        Self {
            tray_icon: tray::create_tray_icon(),
        }
    }
}

impl ApplicationHandler<UserEvent> for Application {
    // required
    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

    // required
    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
    }

    // required
    fn new_events(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        // We create the icon once the event loop is actually running
        // to prevent issues like https://github.com/tauri-apps/tray-icon/issues/90
        // if winit::event::StartCause::Init == cause {
        //     #[cfg(not(target_os = "linux"))]
        //     {
        //         self.tray_icon = Some(Self::new_tray_icon());
        //     }

        //     // // We have to request a redraw here to have the icon actually show up.
        //     // // Winit only exposes a redraw method on the Window so we use core-foundation directly.
        //     // #[cfg(target_os = "macos")]
        //     // unsafe {
        //     //     use objc2_core_foundation::{CFRunLoopGetMain, CFRunLoopWakeUp};

        //     //     let rl = CFRunLoopGetMain().unwrap();
        //     //     CFRunLoopWakeUp(&rl);
        //     // }
        // }
    }

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: UserEvent) {
        log::debug!("Application received user event: {:?}", event);
        match event {
            UserEvent::SimMonitorEvent(
                sim_monitor::Event::ConnectedToSim(state)
                | sim_monitor::Event::DisconnectedFromSim(state),
            ) => {
                self.tray_icon.update_state(state);
            }
            UserEvent::Shutdown => {
                log::info!("Shutting down tray icon and winit event loop");
                self.tray_icon.shutdown();
                event_loop.exit();
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_logging().context("Failed to setup logging")?;

    tracing::info!("Starting iRacing HA Monitor");
    let config = config::get_app_config();
    if config.gui {
        // // Tray icon on Windows
        // #[cfg(not(target_os = "linux"))]
        // let _tray_icon = Application::new_tray_icon();

        // using a daemon is overkill for a plain iced application, but might come in
        // handy when trying to implement a tray icon
        iced::daemon(
            IracingMonitorGui::title,
            IracingMonitorGui::update,
            IracingMonitorGui::view,
        )
        .subscription(IracingMonitorGui::subscription)
        .theme(IracingMonitorGui::theme)
        .run_with(IracingMonitorGui::new)
        .expect("Iced frontend failed");

        // Notes for myself
        // - the frontend should not worry about the config file or config events, it should only keep an internal state that gets
        //   saved to the config file when the user changes something, and updated (via message from the backend) when the config file changes
        // - sim status and tray events should be sent to the frontend via messages from the backend
    } else {
        // create the winit event loop
        let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();
        let event_loop_proxy = event_loop.create_proxy();

        // run the connect() stream and handle the events
        let stream = Box::pin(backend::connect());
        let _stream_handle = tokio::spawn(async move {
            stream
                .for_each(|event| async {
                    match event {
                        backend::Event::Sim(sim_event) => {
                            // Send sim events to winit event loop
                            if let Err(e) = event_loop_proxy.send_event(UserEvent::SimMonitorEvent(sim_event)) {
                                log::warn!("Failed to send event to winit: {}", e);
                            } else {
                                log::debug!("Sent sim event to winit");
                            }
                        }
                        backend::Event::Tray(tray_event) => {
                            // Send quit event to winit event loop
                            if let tray::TrayEventType::MenuItemClicked(menu_id) = tray_event.clone() {
                                log::debug!("menu_id: {:?}", menu_id);
                                match menu_id.0.as_str() {
                                    "quit" => {
                                        log::debug!("Quitting");
                                        if let Err(e) = event_loop_proxy.send_event(UserEvent::Shutdown) {
                                            panic!("Failed to send shutdown event to winit event loop: {}", e);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        backend::Event::ConfigFile(_) => {
                            // Handle config file events if needed
                        }
                    }
                })
                .await;
        });

        // run the application
        let mut app = Application::new();
        if let Err(err) = event_loop.run_app(&mut app) {
            log::error!("App error: {:?}", err);
        }
    }

    Ok(())
}
