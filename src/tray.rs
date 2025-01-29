use std::path::PathBuf;
use anyhow::{Context, Result};
use chrono::Utc;
use iced::futures::channel::mpsc;
use iced::futures::StreamExt;
use iced::futures::{SinkExt, Stream};
use iced::stream;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Serialize;
use std::time::Duration;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    TrayIcon, TrayIconBuilder, TrayIconEvent,
};

// Message type for the application
#[derive(Debug, Clone)]
pub enum Message {
    TrayEvent(TrayEventType),
}

// Types of tray events we'll handle
#[derive(Debug, Clone)]
pub enum TrayEventType {
    IconClicked,
    MenuItemClicked(String),
}

// State for the subscription
// pub struct TrayState {
//     tray_icon: Option<TrayIcon>,
// }

// impl TrayState {
//     pub fn new() -> Self {
//         Self {
//             tray_icon: None,
//         }
//     }
// }

// Create the tray subscription
pub fn tray_subscription() -> impl Stream<Item = TrayEventType> {
    #[cfg(target_os = "linux")]
    tokio::spawn(async move {
        gtk::init().unwrap();
        let _tray_icon = new_tray_icon();
        gtk::main();
    });

    // let mut state = TrayState::new();
    // if state.tray_icon.is_none() {
    //     // Load the icon
    //     let icon = load_icon();

    //     // Create the tray menu
    //     let menu = Menu::new();
    //     let item1 = MenuItem::new("Item 1", true, None);
    //     let _ = menu.append(&item1);

    //     // Create the tray icon
    //     let tray_icon = TrayIconBuilder::new()
    //         .with_menu(Box::new(menu))
    //         .with_tooltip("Iced Application")
    //         .with_icon(icon)
    //         .build()
    //         .expect("Failed to create tray icon");

    //     state.tray_icon = Some(tray_icon);
    // }
    
    stream::channel(100, |mut output| async move {
        // Create channels for events
        // let tray_channel = TrayIconEvent::receiver(); // not supported on Linux, so skip it for now
        // let menu_channel = MenuEvent::receiver();

        let (sender, mut receiver) = mpsc::channel(100);

        // Set up the event handler with a move closure that sends to the channel
        MenuEvent::set_event_handler(Some(move |event| {
            let mut sender = sender.clone();

            // Since the channel sender is async, we need to spawn a task to send
            tokio::spawn(async move {
                let _ = sender.send(event).await;
            });
        }));

        // Wait for events from either channel
        loop {
            tokio::select! {
                Some(menu_event) = receiver.next() => {
                    log::debug!("Received MenuEvent");
                    output.send(TrayEventType::MenuItemClicked(menu_event.id.0)).await.expect("Unable to send event");
                }
            }
        }
    })
}

fn load_icon() -> tray_icon::Icon {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/icon.png");
    let path = std::path::Path::new(path);
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height)
        .expect("Failed to open icon")
}

pub fn new_tray_icon() -> TrayIcon {
    // Create tray icon menu
    let menu = tray_icon::menu::Menu::new();
    let quit_item = tray_icon::menu::MenuItem::new("Quit", true, None);
    menu.append(&quit_item).unwrap();

    // Load the icon
    let icon = load_icon();

    // Build the tray icon
    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("My Iced App")
        .with_icon(icon)
        .build()
        .unwrap()
}