use crate::sim_monitor;
use crate::tray;

use iced::Length::{self, Fill};
use iced::{keyboard, Element};
use iced::widget::{button, column, row, text, text_input, Column, Container, Space};
use iced::window;
use iced::{Subscription, Task};
use tray_icon::{TrayIcon, TrayIconBuilder};

#[derive(Debug, Clone)]
pub enum Message {
    WindowOpened(window::Id),
    WindowClosed(window::Id),
    Quit,

    MqttHostChanged(String),
    MqttPortChanged(String),
    MqttUserChanged(String),
    MqttPasswordChanged(String),
    ApplyMqttConfig,

    SimUpdated(sim_monitor::Event),
    TrayEvent(tray::TrayEventType),
}

enum State {
    WaitingForBackendConnection,
    ConnectedToBackend(sim_monitor::Connection),
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            State::ConnectedToBackend(_connection) => write!(f, "Ready"),
            State::WaitingForBackendConnection => write!(f, "Waiting for backend connection"),
        }
    }
}

enum TrayState {
    WaitingFoConnection,
    Connected(tray::Connection),
}

pub struct IracingMonitorGui {
    mqtt_host: String,
    mqtt_port: String,
    mqtt_user: String,
    mqtt_password: String,
    port_is_valid: bool,

    state: State,
    connected_to_sim: bool,
    sim_state: Option<sim_monitor::SimMonitorState>,

    // tray_icon: Option<TrayIcon>,
    tray: Option<tray::Connection>,
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

impl IracingMonitorGui {
    pub fn new() -> (Self, Task<Message>) {
        let settings = iced::window::Settings {
            size: iced::Size {width: 400.0 * 1.618, height: 400.0 },
            min_size: Some(iced::Size {width: 300.0, height: 400.0 }),
            ..Default::default()
        };
        let (_id, open) = window::open(settings);

        // #[cfg(target_os = "linux")]
        // tokio::spawn(async move {
        //     gtk::init().unwrap();
    
        //     // let _tray_icon = new_tray_icon();
    
        //     // gtk::main();
        // });

        (
            Self {
                mqtt_host: String::from("localhost"),
                mqtt_port: String::from("1883"),
                mqtt_user: String::new(),
                mqtt_password: String::new(),
                port_is_valid: true,
                
                state: State::WaitingForBackendConnection,
                connected_to_sim: false,
                sim_state: None,

                // tray_icon: Some(new_tray_icon()), // panic, gtk has hot been initialized, call gtk::init first
                tray: None,
            },
            open.map(Message::WindowOpened),
        )
    }

    pub fn title(&self, window_id: iced::window::Id) -> String {
        // "IRacingMonitor - Iced".to_string()
        format!("IRacingMonitor {:?}", window_id)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MqttHostChanged(value) => {
                self.mqtt_host = value;
                Task::none()
            }
            Message::MqttPortChanged(value) => {
                log::debug!("Port is {value}");
                if let Ok(_val) = value.parse::<u16>() {
                    self.port_is_valid = true;
                } else {
                    self.port_is_valid = false;
                }
                self.mqtt_port = value;
                Task::none()
            }
            Message::MqttUserChanged(value) => {
                self.mqtt_user = value;
                Task::none()
            }
            Message::MqttPasswordChanged(value) => {
                self.mqtt_password = value;
                Task::none()
            }
            Message::ApplyMqttConfig => {
                if let Ok(port) = self.mqtt_port.parse() {
                    let mqtt_config = sim_monitor::MqttConfig {
                        host: self.mqtt_host.clone(),
                        port,
                        user: self.mqtt_user.clone(),
                        password: self.mqtt_password.clone(),
                    };
                    let msg = sim_monitor::Message::UpdateConfig(mqtt_config);
                    match &mut self.state {
                        State::ConnectedToBackend(connection) => {
                            connection.send(msg);
                        }
                        State::WaitingForBackendConnection => {
                            log::warn!("Invalid state, waiting for backend")
                        }
                    }
                } else {
                    log::warn!("Invalid MQTT config");
                }

                Task::none()
            }
            Message::WindowOpened(_id) => {
                // let window = Window::new(self.windows.len() + 1);
                // let focus_input = text_input::focus(format!("input-{id}"));

                // self.windows.insert(id, window);

                // focus_input
                Task::none()
            }
            Message::WindowClosed(_id) => {
                log::info!("Window closed");
                Task::none()
            }
            Message::SimUpdated(event) => {
                log::debug!("SimUpdated message received! ({event})");
                // self.iracing_connection_status = format!("{event}");

                match event {
                    sim_monitor::Event::Ready(connection) => {
                        self.state = State::ConnectedToBackend(connection);
                        log::info!("Backend ready, waiting for game");
                    }
                    sim_monitor::Event::DisconnectedFromSim(state) => {
                        if self.connected_to_sim {
                            log::info!("Disconnected from game");
                            self.connected_to_sim = false;
                        }
                        self.sim_state = Some(state);
                    }
                    sim_monitor::Event::ConnectedToSim(state) => {
                        if !self.connected_to_sim {
                            log::info!("Connected to game");
                            self.connected_to_sim = true;
                        }
                        self.sim_state = Some(state);
                    }
                }
                Task::none()
            }
            Message::TrayEvent(event) => {
                log::info!("Tray event: {event:?}");
                match event {
                    tray::TrayEventType::MenuItemClicked(id) => {
                        if id.0 == "quit" {
                            Task::done(Message::Quit)
                        } else {
                            Task::none()
                        }
                    }
                    tray::TrayEventType::Connected(connection) => {
                        self.tray = Some(connection);
                        Task::none()
                    }
                    tray::TrayEventType::IconClicked => {
                        Task::none()
                    }
                }
                // Task::none()
            }
            Message::Quit => {
                log::info!("Quit application!");
                
                // kill tray
                if let Some(tray) = &mut self.tray {
                    tray.send(tray::Message::Quit);
                }
                self.tray = None;

                iced::exit()
            }
        }
    }

    pub fn view(&self, _window_id: iced::window::Id) -> Element<Message> {
        let apply_mqtt_config_button =
            if self.port_is_valid && matches!(self.state, State::ConnectedToBackend(_)) {
                button("Apply MQTT config").on_press(Message::ApplyMqttConfig)
            } else {
                button("Apply MQTT config")
            };
        let last_message = if let Some(sim_state) = &self.sim_state {
            sim_state.timestamp.clone()
        } else {
            "None".to_string()
        };
        Container::new(
            column![
                text("iRacing Home Assistant Monitor").size(28),
                Space::new(Length::Shrink, Length::Fixed(16.)),

                text_input("MQTT Host", &self.mqtt_host)
                    .on_input(Message::MqttHostChanged),
                text_input("MQTT Port", &self.mqtt_port)
                    .on_input(Message::MqttPortChanged),
                text_input("MQTT User", &self.mqtt_user)
                    .on_input(Message::MqttUserChanged),
                text_input("MQTT Password", &self.mqtt_password)
                    .on_input(Message::MqttPasswordChanged)
                    .secure(true),
                Space::new(Length::Shrink, Length::Fixed(16.)),

                apply_mqtt_config_button,
                Space::new(Length::Shrink, Length::Fill),
                
                text(self.state.to_string()).size(16),
                text(format!("Last message: {last_message}")),
                button("Quit").on_press(Message::Quit)
                // Space::new(Length::Shrink, Length::Fixed(16.)),
            ]
        )
        .padding(10)
        .center_x(Fill)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        fn handle_hotkey(key: keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Message> {
            match (key.as_ref(), modifiers) {
                // quit on Ctrl+Q
                (keyboard::Key::Character("q"), modifiers) if modifiers.control() => {
                    Some(Message::Quit)
                }
                _ => None,
            }
        }

        Subscription::batch(vec![
            window::close_events().map(Message::WindowClosed),
            keyboard::on_key_press(handle_hotkey),
            Subscription::run(sim_monitor::connect).map(Message::SimUpdated),
            Subscription::run(tray::tray_subscription).map(Message::TrayEvent),
        ])
    }
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