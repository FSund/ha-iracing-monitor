use std::path::PathBuf;
use anyhow::{Context, Result};
use chrono::Utc;
use iced::futures::channel::mpsc;
use iced::futures::StreamExt;
use iced::futures::{SinkExt, Stream};
use iced::stream;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Serialize;
use tray_icon::menu::{AboutMetadata, MenuId, PredefinedMenuItem};
use std::time::Duration;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    TrayIcon, TrayIconBuilder, TrayIconEvent,
};

// Events from tray (to frontend)
#[derive(Debug, Clone)]
pub enum TrayEventType {
    IconClicked,
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
    // On Windows and Linux, an event loop must be running on the thread, on Windows, a win32 event
    // loop and on Linux, a gtk event loop. It doesn't need to be the main thread but you have to
    // create the tray icon on the same thread as the event loop.
    // #[cfg(target_os = "linux")]
    // tokio::spawn(async move {
    //     gtk::init().unwrap();
    //     let _tray_icon = new_tray_icon();
    //     gtk::main();
    // });

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

    let (menu_sender, mut menu_receiver) = mpsc::channel(100);

    // Set up the event handler with a move closure that sends to the channel
    MenuEvent::set_event_handler(Some(move |event| {
        let mut sender = menu_sender.clone();

        // // Since the channel sender is async, we need to spawn a task to send
        // tokio::spawn(async move {
        //     let _ = sender.send(event).await;
        // });

        // Use a blocking channel send instead of spawning a task
        log::debug!("Sending menu event {event:?} to channel");
        if let Ok(()) = sender.try_send(event) {
            log::debug!("Menu event sent to channel");
        } else {
            log::error!("Failed to send menu event to channel");
        }
    }));
    
    stream::channel(100, |mut output| async move {
        // Create channels for events
        // let tray_channel = TrayIconEvent::receiver(); // not supported on Linux, so skip it for now
        // let menu_channel = MenuEvent::receiver();

        

        let (frontend_sender, mut frontend_receiver) = mpsc::channel(100);

        // Send the sender back to the application
        output
            .send(TrayEventType::Connected(Connection(frontend_sender)))
            .await
            .expect("Unable to send");

        // // Wait for events from either channel
        // loop {
        //     tokio::select! {
        //         Some(menu_event) = receiver.next() => {
        //             log::debug!("Received MenuEvent");
        //             output.send(TrayEventType::MenuItemClicked(menu_event.id.0)).await.expect("Unable to send event");
        //         }
        //     }
        // }

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
                // else => {
                //     // break out of loop if no more events
                //     log::debug!("No events");
                //     break;
                // }
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

    // Load the icon
    let icon = load_icon();

    // Build the tray icon
    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("iRacing HA Monitor")
        .with_icon(icon)
        .build()
        .unwrap()
}

pub fn create_tray_icon() -> TrayIcon {
    let tray_icon = new_tray_icon();
    
    let (menu_sender, mut menu_receiver) = mpsc::channel(100);

    // Set up the event handler with a move closure that sends to the channel
    MenuEvent::set_event_handler(Some(move |event| {
        let mut sender = menu_sender.clone();

        // Since the channel sender is async, we need to spawn a task to send
        tokio::spawn(async move {
            log::debug!("Sending menu event {event:?} to channel");
            let _ = sender.send(event).await;
        });

        // Use a blocking channel send instead of spawning a task
        // log::debug!("Sending menu event {event:?} to channel");
        // if let Ok(()) = sender.try_send(event) {
        //     log::debug!("Menu event sent to channel");
        // } else {
        //     log::error!("Failed to send menu event to channel");
        // }
    }));

    tray_icon
}