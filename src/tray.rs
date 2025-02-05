use crate::resources;

use futures::channel::mpsc;
use futures::stream::Stream;
use futures::prelude::sink::SinkExt;
use futures::prelude::stream::StreamExt;
use iced::stream as iced_stream;
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
    // Create a channel for events from the tray
    let (menu_sender, mut menu_receiver) = mpsc::channel(100);

    // Set up the event handler with a move closure that sends to the channel
    MenuEvent::set_event_handler(Some(move |event| {
        let mut sender = menu_sender.clone();

        // Use a blocking channel send instead of spawning a task (no tokio runtime available?)
        log::debug!("Sending menu event {event:?} to channel");
        if let Ok(()) = sender.try_send(event) {
            log::debug!("Menu event sent to channel");
        } else {
            log::error!("Failed to send menu event to channel");
        }
    }));

    iced_stream::channel(100, |mut output| async move {
        // Create channels for events from frontend
        let (frontend_sender, mut frontend_receiver) = mpsc::channel(100);

        // Send the sender back to the application
        output
            .send(TrayEventType::Connected(Connection(frontend_sender)))
            .await
            .expect("Unable to send");

        loop {
            tokio::select! {
                // Process events from tray
                Some(menu_event) = menu_receiver.next() => {
                    log::debug!("Received MenuEvent");
                    // Send tray events to frontend
                    output.send(TrayEventType::MenuItemClicked(menu_event.id))
                        .await
                        .expect("Unable to send event");
                }

                // Process events from frontend
                Some(message) = frontend_receiver.next() => {
                    match message {
                        Message::Quit => {
                            log::info!("Quitting tray icon");
                            break;
                        }
                    }
                }
                else => {
                    // break out of loop if no more events
                    log::debug!("Both receivers are closed");
                    break;
                }
            }
        }
    })
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
