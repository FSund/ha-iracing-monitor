// Use windows_subsystem for release builds to hide console
// This also disables stdout logging
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backend;
mod config;
mod helpers;
mod iracing_client;
mod logging;
mod platform;
mod resources;
mod sim_monitor;
mod tray;

#[cfg(feature = "iced_gui")]
mod frontend;

use anyhow::Context;
use futures::prelude::stream::StreamExt;
use logging::setup_logging;
use winit::{application::ApplicationHandler, event_loop::EventLoop};

#[cfg(feature = "iced_gui")]
use frontend::IracingMonitorGui;

struct Application {
    tray_icon: Box<dyn tray::TrayIconInterface>,
    // Store runtime reference to keep it alive
    #[allow(dead_code)]
    runtime: tokio::runtime::Runtime,
}

impl Application {
    fn new(runtime: tokio::runtime::Runtime) -> Self {
        Self {
            tray_icon: tray::create_tray_icon(),
            runtime,
        }
    }
}

impl ApplicationHandler<backend::Event> for Application {
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
        _cause: winit::event::StartCause,
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

    fn user_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: backend::Event,
    ) {
        log::debug!("Application received user event: {:?}", event);
        match event {
            backend::Event::Sim(
                sim_monitor::Event::ConnectedToSim(state)
                | sim_monitor::Event::DisconnectedFromSim(state),
            ) => {
                self.tray_icon.update_state(state);
            }
            backend::Event::Tray(tray_event) => match tray_event {
                tray::TrayEventType::MenuItemClicked(menu_item) => match menu_item {
                    tray::MenuItem::Quit => {
                        log::info!("Shutting down tray icon and winit event loop");
                        self.tray_icon.shutdown();
                        event_loop.exit();
                    }
                    _ => {}
                },
            },
            backend::Event::Shutdown => {
                self.tray_icon.shutdown();
                event_loop.exit();
            }
            _ => {}
        }
    }
}

fn run_application() -> anyhow::Result<()> {
    // Create a tokio runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("Failed to create Tokio runtime")?;

    // create the winit event loop
    let event_loop = EventLoop::<backend::Event>::with_user_event()
        .build()
        .unwrap();
    let event_loop_proxy = event_loop.create_proxy();

    // run the connect() stream and handle the events
    let stream = Box::pin(backend::connect());
    runtime.spawn(async move {
        stream
            .for_each(|event| async {
                match event.clone() {
                    backend::Event::Sim(_sim_event) => {
                        // Send sim events to winit event loop
                        if let Err(e) = event_loop_proxy.send_event(event) {
                            log::warn!("Failed to send sim event to winit: {}", e);
                        } else {
                            log::debug!("Sent sim event to winit");
                        }
                    }
                    backend::Event::Tray(_tray_event) => {
                        // Send to winit event loop
                        if let Err(e) = event_loop_proxy.send_event(event) {
                            panic!("Failed to send tray event to winit event loop: {}", e);
                        }
                    }
                    backend::Event::ConfigFile(_) => {
                        // Handle config file events if needed
                    }
                    backend::Event::Shutdown => {
                        if let Err(e) = event_loop_proxy.send_event(event) {
                            panic!("Failed to send shutdown event to winit event loop: {}", e);
                        }
                    }
                }
            })
            .await;
    });

    // run the application
    let mut app = Application::new(runtime);
    event_loop
        .run_app(&mut app)
        .context("Failed to run application")?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    setup_logging().context("Failed to setup logging")?;
    tracing::info!("Starting iRacing HA Monitor");
    // let config = config::get_app_config();

    #[cfg(feature = "iced_gui")]
    {
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
    }

    #[cfg(not(feature = "iced_gui"))]
    {
        if let Err(err) = run_application() {
            log::error!("App error: {:?}", err);
        }
    }

    Ok(())
}
