use crate::resources;

use futures::channel::mpsc;
use futures::stream::Stream;
use futures::prelude::stream::StreamExt;
use tray_icon::menu::{MenuId, PredefinedMenuItem};
use tray_icon::{
    menu::MenuEvent,
    TrayIcon, TrayIconBuilder,
};
use anyhow::Result;

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
            Err(err) => log::error!("Failed to send menu event to channel: {}", err)
        }
    }));

    // Create the initial connection event stream
    let init_stream = futures::stream::once(async move {
        TrayEventType::Connected(Connection(frontend_sender))
    });

    // Convert the frontend receiver into a stream that ends on Quit message
    let frontend_stream = frontend_receiver.take_while(|msg| {
        let continue_running = !matches!(msg, Message::Quit);
        if !continue_running {
            log::info!("Quitting tray icon");
        }
        futures::future::ready(continue_running)
    }).filter_map(|_| futures::future::ready(None));

    // Merge all streams together
    futures::stream::select(
        init_stream,
        futures::stream::select(
            rx,
            frontend_stream
        )
    )
}

fn load_icon() -> Result<tray_icon::Icon> {
    let pixels = resources::load_as_rgba(resources::ICON_BYTES)?;
    let icon = tray_icon::Icon::from_rgba(pixels.to_vec(), pixels.width(), pixels.height())?;
    Ok(icon)
}

pub fn new_tray_icon() -> TrayIcon {
    // Create tray icon menu
    let menu = tray_icon::menu::Menu::new();
    // TODO: don't hardcode the menu item ids using strings
    let options_item = tray_icon::menu::MenuItem::with_id("options", "Options", true, None);
    let quit_item = tray_icon::menu::MenuItem::with_id("quit", "Quit", true, None);
    menu.append_items(&[
        &options_item,
        &PredefinedMenuItem::separator(),
        // &PredefinedMenuItem::about(
        //     None,
        //     Some(AboutMetadata {
        //         name: Some("tao".to_string()),
        //         copyright: Some("Copyright tao".to_string()),
        //         ..Default::default()
        //     }),
        // ),
        // &PredefinedMenuItem::separator(),
        &quit_item,
    ]).expect("Failed to append items");

    // Add menu and tooltip
    let mut builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("iRacing HA Monitor");

    // Add icon
    if let Ok(icon) = load_icon() {
        builder = builder.with_icon(icon);
    } else {
        log::warn!("Failed to load tray icon, continuing without icon");
    }

    // Build the tray icon
    builder
        .build()
        .unwrap()
}
