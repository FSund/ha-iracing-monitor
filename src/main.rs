#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod iracing_client;
mod frontend;
mod sim_monitor;
mod resources;
mod config;

#[cfg(feature = "tray")]
mod tray;

use env_logger::{Builder, Target};
use log::LevelFilter;
use frontend::IracingMonitorGui;
use std::fs;
use chrono::Local;

pub fn main() -> iced::Result {
    // Optional tray icon
    #[cfg(feature = "tray")]
    {
        // Since winit doesn't use gtk on Linux, and we need gtk for
        // the tray icon to show up, we need to spawn a thread
        // where we initialize gtk and create the tray_icon
        #[cfg(target_os = "linux")]
        std::thread::spawn(|| {
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
    }

    // env_logger::init();
    let mut builder = Builder::from_default_env();
    
    // Set external crates to INFO level
    // builder.filter_module("rumqttc", LevelFilter::Info);

    // filter messages from iracing_ha_monitor::sim_monitor
    builder.filter_module("iracing_ha_monitor::sim_monitor", LevelFilter::Info);
    
    // Keep your application at DEBUG level
    builder.filter_module("iracing_ha_monitor", LevelFilter::Debug);

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
    builder
        .target(target)
        .init();

    // using a daemon is overkill for a plain iced application, but might come in
    // handy when trying to implement a tray icon
    iced::daemon(IracingMonitorGui::title, IracingMonitorGui::update, IracingMonitorGui::view)
        .subscription(IracingMonitorGui::subscription)
        .theme(IracingMonitorGui::theme)
        .run_with(IracingMonitorGui::new)
}
