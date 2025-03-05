use crate::platform;
use directories::ProjectDirs;

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

// Re-export platform-specific functions
pub use platform::{get_run_on_startup_state, set_run_at_startup, toggle_run_on_boot};
