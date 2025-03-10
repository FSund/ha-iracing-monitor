use crate::backend::Event;
use crate::resources;

use futures::prelude::sink::SinkExt;
use futures::stream::Stream;
use iced_futures::stream as iced_stream;
use std::io;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

pub fn set_run_at_startup(enable: bool, exe_path: &str) -> io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key =
        hkcu.open_subkey_with_flags(r"Software\Microsoft\Windows\CurrentVersion\Run", KEY_WRITE)?;

    let app_name = resources::APP_NAME;
    if enable {
        // Add application to startup
        log::debug!(
            "Adding application {} to startup with path {}",
            app_name,
            &exe_path
        );
        run_key.set_value(app_name, &exe_path)?;
    } else {
        // Remove application from startup
        log::debug!("Removing application {} from startup", app_name);
        run_key.delete_value(app_name)?;
    }

    Ok(())
}

pub fn get_run_on_startup_state() -> io::Result<bool> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key =
        hkcu.open_subkey_with_flags(r"Software\Microsoft\Windows\CurrentVersion\Run", KEY_READ)?;

    // Check if key exists
    let app_name = resources::APP_NAME;
    match run_key.get_value::<String, _>(app_name) {
        Ok(_) => Ok(true),
        Err(e) => {
            log::debug!("Error while getting registry value: {e}");
            Ok(false)
        }
    }
}

// Then in your settings handler:
pub fn toggle_run_on_boot() {
    let exe_path = std::env::current_exe()
        .unwrap()
        .to_string_lossy()
        .to_string();

    match get_run_on_startup_state() {
        Ok(current_value) => {
            log::debug!("Toggling run on boot (new value: {})", !current_value);
            if let Err(e) = set_run_at_startup(!current_value, &exe_path) {
                // Handle error, perhaps show in UI
                println!("Failed to update startup setting: {}", e);
            }
        }
        Err(e) => {
            log::warn!("Unable to get current run on startup state: {e}");
        }
    }
}

pub fn shutdown_signals() -> impl Stream<Item = Event> {
    iced_stream::channel(100, |mut output| async move {
        let mut ctrl_c = tokio::signal::windows::ctrl_c().unwrap();
        let mut ctrl_break = tokio::signal::windows::ctrl_break().unwrap();
        let mut ctrl_close = tokio::signal::windows::ctrl_close().unwrap();
        let mut ctrl_shutdown = tokio::signal::windows::ctrl_shutdown().unwrap();

        loop {
            tokio::select! {
                result = ctrl_c.recv() => {
                    if result.is_some() {
                        log::info!("Received Ctrl+C signal");
                        let _ = output.send(Event::Shutdown).await;
                    }
                }
                result = ctrl_break.recv() => {
                    if result.is_some() {
                        log::info!("Received Ctrl+Break signal");
                        let _ = output.send(Event::Shutdown).await;
                    }
                }
                result = ctrl_close.recv() => {
                    if result.is_some() {
                        log::info!("Received Close signal");
                        let _ = output.send(Event::Shutdown).await;
                    }
                }
                result = ctrl_shutdown.recv() => {
                    if result.is_some() {
                        log::info!("Received Shutdown signal");
                        let _ = output.send(Event::Shutdown).await;
                    }
                }
            }
        }
    })
}
