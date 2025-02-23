#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backend;
mod iracing_client;
mod frontend;
mod sim_monitor;
mod tray;
mod resources;
mod config;

use frontend::IracingMonitorGui;
use futures::prelude::stream::StreamExt;
use std::fs;
use chrono::Local;
use tracing_subscriber::{
    fmt,
    prelude::*,
};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

use tray_icon::{
    menu::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder, TrayIconEvent, TrayIconEventReceiver,
};
use winit::{
    application::ApplicationHandler,
    event::Event,
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder},
};

#[derive(Debug)]
enum UserEvent {
    TrayIconEvent(tray_icon::TrayIconEvent),
    MenuEvent(tray_icon::menu::MenuEvent),
    Shutdown,
}

struct Application {
    tray_icon: Option<TrayIcon>,
}

impl Application {
    fn new() -> Application {
        Application { tray_icon: None }
    }

    fn new_tray_icon() -> TrayIcon {
        tray::new_tray_icon()
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
            _ => println!("{event:?}"),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create logs directory if it doesn't exist
    fs::create_dir_all("logs").expect("Failed to create logs directory");

    // Generate timestamp for log file
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let log_file = format!("logs/iracing_ha_monitor_{}.log", timestamp);

    // Create file appender
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        "logs",
        "iracing_ha_monitor.log",
    );

    // Create stdout layer
    let stdout_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_file(true)
        .with_filter(tracing_subscriber::filter::LevelFilter::DEBUG);

    // Create file layer
    let file_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_file(true)
        .with_ansi(false)
        .with_writer(file_appender)
        .with_filter(tracing_subscriber::filter::LevelFilter::DEBUG);

    // Combine layers and set as global default
    tracing_subscriber::registry()
        .with(
            stdout_layer
            // .with_filter(
            //     tracing_subscriber::filter::EnvFilter::new("iracing_ha_monitor=debug")
            // )
        )
        .with(
            file_layer
            // .with_filter(
            //     tracing_subscriber::filter::EnvFilter::new("iracing_ha_monitor=debug")
            // )
        )
        .init();

    tracing::info!("Starting iRacing HA Monitor");
    let config = config::get_app_config();
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
        let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();
        let event_loop_proxy = event_loop.create_proxy();

        // run the connect() stream
        let stream = Box::pin(backend::connect(Some(event_loop_proxy.clone())));
        let _stream_handle = tokio::spawn(async move {
            stream.for_each(|_event| async move {
                // log::debug!("event: {:?}", event);
            }).await;
        });
        // handle.await.expect("Stream task failed");

        

        // set a tray event handler that forwards the event and wakes up the event loop
        // let proxy = event_loop.create_proxy();
        // TrayIconEvent::set_event_handler(Some(move |event| {
        //     proxy.send_event(UserEvent::TrayIconEvent(event));
        // }));
        // let proxy = event_loop.create_proxy();
        // MenuEvent::set_event_handler(Some(move |event| {
        //     proxy.send_event(UserEvent::MenuEvent(event));
        // }));

        let mut app = Application::new();

        // let menu_channel = MenuEvent::receiver();
        // let tray_channel = TrayIconEvent::receiver();
    
        // Since winit doesn't use gtk on Linux, and we need gtk for
        // the tray icon to show up, we need to spawn a thread
        // where we initialize gtk and create the tray_icon
        #[cfg(target_os = "linux")]
        std::thread::spawn(|| {
            gtk::init().unwrap();
    
            let _tray_icon = Application::new_tray_icon();
    
            gtk::main();
        });
    
        if let Err(err) = event_loop.run_app(&mut app) {
            println!("Error: {:?}", err);
        }
    }

    Ok(())
}
