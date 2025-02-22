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
// use futures::prelude::sink::SinkExt;
// use futures::stream::Stream;
// use iced::stream as iced_stream;

use std::fs;
use chrono::Local;
use tracing_subscriber::{
    fmt,
    prelude::*,
};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

#[tokio::main]
async fn main() -> iced::Result {
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
        .with_file(true);

    // Create file layer
    let file_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_file(true)
        .with_ansi(false)
        .with_writer(file_appender);

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
        let stream = Box::pin(backend::connect());
        let handle = tokio::spawn(async move {
            stream.for_each(|_event| async move {
                // log::debug!("event: {:?}", event);
            }).await;
        });
        handle.await.expect("Stream task failed");
    }

    Ok(())
}
