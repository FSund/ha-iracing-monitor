use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;

pub fn get_config_dir() -> PathBuf {
    // First try executable directory
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe_path| exe_path.parent().map(|p| p.to_path_buf()));

    // If exe directory exists, use it
    if let Some(path) = exe_dir.filter(|p| p.exists()) {
        log::info!("Using executable directory: {:?}", path);
        return path;
    }

    // Otherwise, use ProjectDirs
    let proj_dirs = ProjectDirs::from("com", "FSund", "iracing-ha-monitor")
        .expect("Failed to determine project directories");
    
    // Create config directory if it doesn't exist
    fs::create_dir_all(proj_dirs.config_dir())
        .expect("Failed to create config directory");

    log::info!("Using user config directory: {:?}", proj_dirs.config_dir());
    proj_dirs.config_dir().to_path_buf()
}