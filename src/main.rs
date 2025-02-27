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

use anyhow::{Context, Result};
use frontend::IracingMonitorGui;
use futures::prelude::stream::StreamExt;
use logging::setup_logging;
use std::fs;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{filter::Targets, fmt, prelude::*, Registry};
use tray_icon::TrayIcon;
use winit::{application::ApplicationHandler, event_loop::EventLoop};

#[derive(Debug)]
enum UserEvent {
    TrayIconEvent(tray_icon::TrayIconEvent),
    MenuEvent(tray_icon::menu::MenuEvent),
    Shutdown,
    SimMonitorEvent(sim_monitor::Event),
}

struct Application {
    tray_icon: Option<TrayIcon>,
    session_type: sim_monitor::SessionType,
}

impl Application {
    fn new() -> Application {
        Application {
            tray_icon: None,
            session_type: sim_monitor::SessionType::Disconnected,
        }
    }

    fn new_tray_icon() -> TrayIcon {
        tray::new_tray_icon()
    }

    fn update_tray_icon(&mut self, session_type: &sim_monitor::SessionType) {
        if let Some(tray) = self.tray_icon.as_mut() {
            let icon = match session_type {
                sim_monitor::SessionType::Disconnected => tray::load_icon_disconnected(),
                _ => tray::load_icon_connected(),
            };
            if let Ok(icon) = icon {
                if let Err(e) = tray.set_icon(Some(icon)) {
                    log::warn!("Failed to set tray icon: {}", e);
                }
            } else {
                log::warn!("Failed to load connected tray icon");
            }
        }
    }

    fn update_menu(&mut self, session_type: &sim_monitor::SessionType) {
        if let Some(tray) = self.tray_icon.as_mut() {
            let new_menu = tray::make_menu(Some(session_type.to_string()));
            tray.set_menu(Some(Box::new(new_menu)));
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
        if winit::event::StartCause::Init == cause {
            #[cfg(not(target_os = "linux"))]
            {
                self.tray_icon = Some(Self::new_tray_icon());
            }

            // // We have to request a redraw here to have the icon actually show up.
            // // Winit only exposes a redraw method on the Window so we use core-foundation directly.
            // #[cfg(target_os = "macos")]
            // unsafe {
            //     use objc2_core_foundation::{CFRunLoopGetMain, CFRunLoopWakeUp};

            //     let rl = CFRunLoopGetMain().unwrap();
            //     CFRunLoopWakeUp(&rl);
            // }
        }
    }

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Shutdown => {
                event_loop.exit();
            }
            UserEvent::SimMonitorEvent(
                sim_monitor::Event::ConnectedToSim(state)
                | sim_monitor::Event::DisconnectedFromSim(state),
            ) => {
                // Store the new session type in a temporary variable
                let new_session_type = state.current_session_type;
                // Compare with current session type
                if new_session_type != self.session_type {
                    self.session_type = new_session_type.clone();
                    self.update_tray_icon(&new_session_type);
                    self.update_menu(&new_session_type);
                }
            }
            _ => log::debug!("{event:?}"),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_logging().context("Failed to setup logging")?;

    // Since winit doesn't use gtk on Linux, and we need gtk for
    // the tray icon to show up, we need to spawn a thread
    // where we initialize gtk and create the tray_icon
    #[cfg(target_os = "linux")]
    std::thread::spawn(|| {
        gtk::init().unwrap();

        let _tray_icon = Application::new_tray_icon();

        gtk::main();
    });

    tracing::info!("Starting iRacing HA Monitor");
    let config = config::get_app_config();
    if config.gui {
        // Tray icon on Windows
        #[cfg(not(target_os = "linux"))]
        let _tray_icon = Application::new_tray_icon();

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

        // run the connect() stream
        let stream = Box::pin(backend::connect(Some(event_loop_proxy.clone())));
        let _stream_handle = tokio::spawn(stream.for_each(|_| futures::future::ready(())));

        // run the application (only contains the tray icon)
        let mut app = Application::new();
        if let Err(err) = event_loop.run_app(&mut app) {
            log::error!("App error: {:?}", err);
        }
    }

    Ok(())
}
