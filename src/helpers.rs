use crate::resources;

use directories::ProjectDirs;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;
// use std::fs;
use std::io;
// use std::path::PathBuf;

pub fn get_project_dir() -> ProjectDirs {
    ProjectDirs::from("com", "FSund", "iracing-ha-monitor")
        .expect("Failed to determine project directories")
}

// pub fn get_data_dir() -> PathBuf {
//     // First try executable directory
//     let exe_dir = std::env::current_exe()
//         .ok()
//         .and_then(|exe_path| exe_path.parent().map(|p| p.to_path_buf()));

//     // If exe directory exists, use it
//     if let Some(path) = exe_dir.filter(|p| p.exists()) {
//         log::info!("Using executable directory for data: {:?}", path);
//         return path;
//     }

//     // Otherwise, use ProjectDirs
//     let proj_dirs = get_project_dir();

//     // Create config directory if it doesn't exist
//     fs::create_dir_all(proj_dirs.data_dir()).expect("Failed to create data directory");

//     log::info!("Using user data directory: {:?}", proj_dirs.data_dir());
//     proj_dirs.data_dir().to_path_buf()
// }

// Function to enable/disable run at startup
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
