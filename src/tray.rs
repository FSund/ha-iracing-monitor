use crate::resources;

use anyhow::Result;
use futures::channel::mpsc;
use futures::prelude::stream::StreamExt;
use futures::stream::Stream;
use tray_icon::menu::{MenuId, PredefinedMenuItem};
use tray_icon::{menu::MenuEvent, TrayIcon, TrayIconBuilder};

// Events from tray (to frontend)
#[derive(Debug, Clone)]
pub enum TrayEventType {
    // IconClicked,
    MenuItemClicked(MenuId),
    Connected(Connection),
}

// messages to tray (from frontend)
#[derive(Debug, Clone)]
pub enum Message {
    Quit,
}

#[derive(Debug, Clone)]
pub struct Connection(mpsc::Sender<Message>);

impl Connection {
    pub fn send(&mut self, message: Message) {
        self.0.try_send(message).expect("Send message SimMonitor");
    }
}

// Create the tray subscription
pub fn tray_subscription() -> impl Stream<Item = TrayEventType> {
    let (tx, rx) = mpsc::channel(100);
    let (frontend_sender, frontend_receiver) = mpsc::channel(100);

    // Set up the menu event handler
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let mut sender = tx.clone();
        let message = TrayEventType::MenuItemClicked(event.id.clone());

        log::debug!("Sending menu event {event:?} to channel");
        match sender.try_send(message) {
            Ok(()) => log::debug!("Menu event sent to channel"),
            Err(err) => log::error!("Failed to send menu event to channel: {}", err),
        }
    }));

    // Create the initial connection event stream
    let init_stream =
        futures::stream::once(async move { TrayEventType::Connected(Connection(frontend_sender)) });

    // Convert the frontend receiver into a stream that ends on Quit message
    let frontend_stream = frontend_receiver
        .take_while(|msg| {
            let continue_running = !matches!(msg, Message::Quit);
            if !continue_running {
                log::info!("Quitting tray icon");
            }
            futures::future::ready(continue_running)
        })
        .filter_map(|_| futures::future::ready(None));

    // Merge all streams together
    futures::stream::select(init_stream, futures::stream::select(rx, frontend_stream))
}

fn load_icon(icon_bytes: &[u8]) -> Result<tray_icon::Icon> {
    let pixels = resources::load_as_rgba(icon_bytes)?;
    let icon = tray_icon::Icon::from_rgba(pixels.to_vec(), pixels.width(), pixels.height())?;
    Ok(icon)
}

pub fn load_icon_connected() -> Result<tray_icon::Icon> {
    load_icon(resources::ICON_BYTES)
}

pub fn load_icon_disconnected() -> Result<tray_icon::Icon> {
    load_icon(resources::ICON_DISCONNECTED_BYTES)
}

pub fn make_menu(current_session: Option<String>) -> tray_icon::menu::Menu {
    // Create tray icon menu
    let menu = tray_icon::menu::Menu::new();
    let options_item = tray_icon::menu::MenuItem::with_id("options", "Options", true, None);
    let quit_item = tray_icon::menu::MenuItem::with_id("quit", "Quit", true, None);

    // Add options item first
    menu.append_items(&[&options_item, &PredefinedMenuItem::separator()])
        .expect("Failed to append options item");

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

pub fn new_tray_icon() -> TrayIcon {
    let menu = make_menu(None);

    // Add menu and tooltip
    let mut builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("iRacing HA Monitor");

    // Add icon
    if let Ok(icon) = load_icon_disconnected() {
        builder = builder.with_icon(icon);
    } else {
        log::warn!("Failed to load tray icon, continuing without icon");
    }

    // Build the tray icon
    builder.build().unwrap()
}
