// use std::io::Cursor;
use tray_icon::{TrayIconBuilder, menu::Menu};
use tray_icon::menu::{MenuItem, PredefinedMenuItem};
use anyhow::Context;

fn main() {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();

    // Load icon from binary data
    let icon = include_bytes!("../resources/icon.png");
    let icon = image::ImageReader::new(std::io::Cursor::new(icon));
    let icon_with_format = icon.with_guessed_format().context("Failed to guess image format").unwrap();
    let pixels = icon_with_format.decode().context("Failed to decode image").unwrap().to_rgba8();
    let icon = tray_icon::Icon::from_rgba(pixels.to_vec(), pixels.width(), pixels.height()).unwrap();

    // Create tray icon menu
    let menu = tray_icon::menu::Menu::new();
    let options_item = tray_icon::menu::MenuItem::with_id("options", "Options", true, None);
    let quit_item = tray_icon::menu::MenuItem::with_id("quit", "Quit", true, None);
    menu.append_items(&[
        &options_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ])
    .expect("Failed to append items");

    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("system-tray - tray icon library!")
        .with_icon(icon)
        .build()
        .unwrap();

    // Keep the program running
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
