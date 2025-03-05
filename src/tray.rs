use crate::helpers;
use crate::resources;
use crate::sim_monitor;

use anyhow::Result;
use futures::channel::mpsc;
use futures::prelude::stream::StreamExt;
use futures::stream::Stream;
use tray_icon::menu::{MenuId, PredefinedMenuItem};
use tray_icon::{menu::MenuEvent, TrayIcon, TrayIconBuilder};

struct SimTrayIcon {
    tray_icon: TrayIcon,
    session_type: Option<sim_monitor::SessionType>,
}

impl SimTrayIcon {
    fn new() -> Self {
        Self {
            tray_icon: new_tray_icon(),
            session_type: None,
        }
    }

    fn update_tray_icon(&mut self, session_type: &sim_monitor::SessionType) {
        let icon = match session_type {
            sim_monitor::SessionType::Disconnected => load_icon_disconnected(),
            _ => load_icon_connected(),
        };
        if let Ok(icon) = icon {
            if let Err(e) = self.tray_icon.set_icon(Some(icon)) {
                log::warn!("Failed to set tray icon: {}", e);
            }
        } else {
            log::warn!("Failed to load connected tray icon");
        }
    }

    fn update_menu(&mut self, session_type: &sim_monitor::SessionType) {
        let new_menu = make_menu(Some(session_type.to_string()));
        self.tray_icon.set_menu(Some(Box::new(new_menu)));
    }

    fn update_session_state(&mut self, new_state: sim_monitor::SessionType) {
        let old_state = self.session_type.replace(new_state.clone());
        if old_state.as_ref() != Some(&new_state) {
            log::debug!("Received new session state: {:?}", new_state);
            self.update_tray_icon(&new_state);
            self.update_menu(&new_state);
        }
    }
}

pub trait TrayIconInterface {
    fn update_state(&mut self, state: sim_monitor::SimMonitorState);
    fn shutdown(&mut self);
}

// Implement for MyTrayIcon (Windows/macOS)
impl TrayIconInterface for SimTrayIcon {
    fn update_state(&mut self, state: sim_monitor::SimMonitorState) {
        self.update_session_state(state.current_session_type);
    }

    fn shutdown(&mut self) {
        // Nothing special needed for direct implementation
    }
}

// Add a new struct for Linux GTK implementation
pub struct GtkTrayIcon {
    sender: std::sync::mpsc::Sender<sim_monitor::SimMonitorState>,
}

impl GtkTrayIcon {
    pub fn new(sender: std::sync::mpsc::Sender<sim_monitor::SimMonitorState>) -> Self {
        Self { sender }
    }
}

impl TrayIconInterface for GtkTrayIcon {
    fn update_state(&mut self, state: sim_monitor::SimMonitorState) {
        if let Err(e) = self.sender.send(state) {
            log::error!("Failed to send state to GTK tray: {}", e);
        }
    }

    fn shutdown(&mut self) {
        // Channel will be closed when dropped
    }
}

// Create a platform-specific factory function
pub fn create_tray_icon() -> Box<dyn TrayIconInterface> {
    #[cfg(target_os = "linux")]
    {
        let (tx, rx) = std::sync::mpsc::channel::<sim_monitor::SimMonitorState>();

        // Since winit doesn't use gtk on Linux, and we need gtk for
        // the tray icon to show up, we need to spawn a thread
        // where we initialize gtk and create the tray_icon

        // Spawn GTK thread
        std::thread::spawn(move || {
            gtk::init().unwrap();
            let mut tray_icon = SimTrayIcon::new();

            loop {
                // Process GTK events
                while gtk::events_pending() {
                    gtk::main_iteration_do(false);
                }

                // Check for new states
                if let Ok(state) = rx.try_recv() {
                    tray_icon.update_session_state(state.current_session_type);
                }

                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });

        Box::new(GtkTrayIcon::new(tx))
    }

    #[cfg(not(target_os = "linux"))]
    {
        Box::new(SimTrayIcon::new())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MenuItem {
    Options,
    Quit,
    RunOnBoot,
}

impl ToString for MenuItem {
    fn to_string(&self) -> String {
        match self {
            MenuItem::Options => "options".to_string(),
            MenuItem::Quit => "quit".to_string(),
            MenuItem::RunOnBoot => "run_on_boot".to_string(),
        }
    }
}

impl MenuItem {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "options" => Some(MenuItem::Options),
            "quit" => Some(MenuItem::Quit),
            "run_on_boot" => Some(MenuItem::RunOnBoot),
            _ => None,
        }
    }
}

// Events from the tray icon
#[derive(Debug, Clone)]
pub enum TrayEventType {
    MenuItemClicked(MenuItem),
}

// Create the tray subscription
pub fn tray_subscription() -> impl Stream<Item = TrayEventType> {
    let (tx, rx) = mpsc::channel(100);

    // Set up the menu event handler
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let mut sender = tx.clone();

        // Convert the menu ID to our custom MenuItem enum
        if let Some(menu_item) = MenuItem::from_str(&event.id.0) {
            let message = TrayEventType::MenuItemClicked(menu_item);

            log::debug!("Sending menu event {event:?} to channel");
            match sender.try_send(message) {
                Ok(()) => log::debug!("Menu event sent to channel"),
                Err(err) => log::error!("Failed to send menu event to channel: {}", err),
            }
        } else {
            log::warn!("Unknown menu item ID: {}", event.id.0);
        }
    }));

    rx
}

fn load_icon(icon_bytes: &[u8]) -> Result<tray_icon::Icon> {
    let pixels = resources::load_as_rgba(icon_bytes)?;
    let icon = tray_icon::Icon::from_rgba(pixels.to_vec(), pixels.width(), pixels.height())?;
    Ok(icon)
}

fn load_icon_connected() -> Result<tray_icon::Icon> {
    load_icon(resources::ICON_BYTES)
}

fn load_icon_disconnected() -> Result<tray_icon::Icon> {
    load_icon(resources::ICON_DISCONNECTED_BYTES)
}

fn make_menu(current_session: Option<String>) -> tray_icon::menu::Menu {
    // Create tray icon menu
    let menu = tray_icon::menu::Menu::new();
    let options_item = tray_icon::menu::MenuItem::with_id(
        MenuItem::Options.to_string(),
        "Open config file",
        true,
        None,
    );

    let quit_item =
        tray_icon::menu::MenuItem::with_id(MenuItem::Quit.to_string(), "Quit", true, None);

    // Add options item first
    menu.append_items(&[&options_item, &PredefinedMenuItem::separator()])
        .expect("Failed to append options item");

    if let Ok(run_on_boot) = helpers::get_run_on_startup_state() {
        let run_on_boot_item = tray_icon::menu::CheckMenuItem::with_id(
            MenuItem::RunOnBoot.to_string(),
            "Run on boot",
            true,
            run_on_boot,
            None,
        );
        menu.append_items(&[&run_on_boot_item])
            .expect("Failed to append run on boot item");
    }

    menu.append_items(&[&PredefinedMenuItem::separator()])
        .expect("Failed to append separator");

    // Add session info in the middle if available
    if let Some(session) = current_session {
        menu.append_items(&[
            &tray_icon::menu::MenuItem::new(session, false, None),
            &PredefinedMenuItem::separator(),
        ])
        .expect("Failed to append session info");
    }

    // Add quit item last
    menu.append_items(&[&quit_item])
        .expect("Failed to append quit item");

    menu
}

fn new_tray_icon() -> TrayIcon {
    let menu = make_menu(None);

    // Add menu and tooltip
    let mut builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(resources::APP_NAME);

    // Add icon
    if let Ok(icon) = load_icon_disconnected() {
        builder = builder.with_icon(icon);
    } else {
        log::warn!("Failed to load tray icon, continuing without icon");
    }

    // Build the tray icon
    builder.build().unwrap()
}
