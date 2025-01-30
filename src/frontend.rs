use crate::sim_monitor;
use crate::tray;
use crate::resources;
use crate::config;

use iced::Length::{self, Fill};
use iced::{keyboard, Element};
use iced::widget::{button, column, row, text, text_input, Column, Container, Space};
use iced::window;
use iced::{Subscription, Task};

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

    ConfigChanged(config::Event),
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

pub struct IracingMonitorGui {
    config: config::AppConfig,

    // mqtt_host: String,
    // mqtt_port: String,
    // mqtt_user: String,
    // mqtt_password: String,
    port_is_valid: bool,

    state: State,
    connected_to_sim: bool,
    sim_state: Option<sim_monitor::SimMonitorState>,

    // tray_icon: Option<TrayIcon>,
    tray: Option<tray::Connection>,

    window_id: Option<window::Id>,
}

impl IracingMonitorGui {
    pub fn new() -> (Self, Task<Message>) {
        let settings = Self::window_settings();
        let config = config::get_app_config();
        let (id, open) = window::open(settings);

        (
            Self {
                config: config.clone(),

                // mqtt_host: config.mqtt.host,
                // mqtt_port: config.mqtt.port.to_string(),
                // mqtt_user: config.mqtt.user,
                // mqtt_password: config.mqtt.password,
                port_is_valid: true,
                
                state: State::WaitingForBackendConnection,
                connected_to_sim: false,
                sim_state: None,

                // tray_icon: Some(new_tray_icon()), // panic, gtk has hot been initialized, call gtk::init first
                tray: None,

                window_id: Some(id),
            },
            open.map(Message::WindowOpened),
        )
    }

    fn window_settings() -> iced::window::Settings {
        iced::window::Settings {
            size: iced::Size {width: 400.0 * 1.618, height: 400.0 },
            min_size: Some(iced::Size {width: 300.0, height: 400.0 }),
            icon: load_icon(),
            ..Default::default()
        }
    }

    pub fn theme(&self, _window_id: iced::window::Id) -> iced::Theme {
        // iced::Theme::Oxocarbon
        iced::Theme::Dark
    }

    pub fn title(&self, _window_id: iced::window::Id) -> String {
        // "IRacingMonitor - Iced".to_string()
        // format!("IRacingMonitor {:?}", window_id)
        "iRacingMonitor".to_string()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MqttHostChanged(value) => {
                self.config.mqtt.host = value;
                Task::none()
            }
            Message::MqttPortChanged(value) => {
                log::debug!("Port is {value}");
                if let Ok(_val) = value.parse::<u16>() {
                    self.port_is_valid = true;
                } else {
                    self.port_is_valid = false;
                }
                self.config.mqtt.port = value;
                Task::none()
            }
            Message::MqttUserChanged(value) => {
                self.config.mqtt.user = value;
                Task::none()
            }
            Message::MqttPasswordChanged(value) => {
                self.config.mqtt.password = value;
                Task::none()
            }
            Message::ApplyMqttConfig => {
                if let Ok(_port) = self.config.mqtt.port.parse::<u16>() {
                    // self.config.mqtt.host = self.config.mqtt.host.clone();
                    // self.config.mqtt.port = port;
                    // self.config.mqtt.user = self.mqtt_user.clone();
                    // self.config.mqtt.password = self.mqtt_password.clone();

                    self.config.save().expect("Failed to save config to file");

                    let msg = sim_monitor::Message::UpdateConfig(self.config.mqtt.clone());
                    match &mut self.state {
                        State::ConnectedToBackend(connection) => {
                            connection.send(msg);
                        }
                        State::WaitingForBackendConnection => {
                            log::warn!("Invalid state, waiting for backend")
                        }
                    }
                } else {
                    log::warn!("Invalid MQTT port {} (must be a number)", self.config.mqtt.port);
                }

                Task::none()
            }
            Message::WindowOpened(id) => {
                if id != self.window_id.unwrap() {
                    log::warn!("Window ID mismatch");
                }
                Task::none()
            }
            Message::WindowClosed(_id) => {
                log::info!("Window closed");
                self.window_id = None;
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
                        // if id.0 == "quit" {
                        //     Task::done(Message::Quit)
                        // } else {
                        //     Task::none()
                        // }
                        match id.0.as_str() {
                            // TODO: matching on strings is bad and you should feel bad
                            "quit" => Task::done(Message::Quit),
                            "options"  => self.open_window(),
                            _ => Task::none(),
                        }
                    }
                    tray::TrayEventType::Connected(connection) => {
                        self.tray = Some(connection);
                        Task::none()
                    }
                    // tray::TrayEventType::IconClicked => {
                    //     self.open_window()
                    // }
                }
            }
            Message::ConfigChanged(event) => {
                log::info!("Config changed: {event:?}");
                Task::none()
            }
            Message::Quit => {
                log::info!("Quit application!");
                
                // kill tray
                if let Some(tray) = &mut self.tray {
                    tray.send(tray::Message::Quit);
                }

                iced::exit()
            }
        }
    }

    fn open_window(&mut self) -> Task<Message> {
        if self.window_id.is_none() {
            log::debug!("Opening settings window");
            let settings = Self::window_settings();
            let (id, open) = window::open(settings);
            self.window_id = Some(id);
            open.map(Message::WindowOpened)
        } else {
            log::info!("Settings window already open");
            Task::none()
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

                text_input("MQTT Host", &self.config.mqtt.host)
                    .on_input(Message::MqttHostChanged),
                text_input("MQTT Port", &self.config.mqtt.port)
                    .on_input(Message::MqttPortChanged),
                text_input("MQTT User", &self.config.mqtt.user)
                    .on_input(Message::MqttUserChanged),
                text_input("MQTT Password", &self.config.mqtt.password)
                    .on_input(Message::MqttPasswordChanged)
                    .secure(true),
                Space::new(Length::Shrink, Length::Fixed(16.)),

                apply_mqtt_config_button,
                Space::new(Length::Shrink, Length::Fill),
                
                row![
                    column![
                        text(self.state.to_string()).size(16),
                        text(format!("Session type: {}", if let Some(sim_state) = &self.sim_state { sim_state.current_session_type.clone() } else { "None".to_string() })),
                        text(format!("Last message: {last_message}")),
                    ],
                    Space::new(Length::Fill, Length::Shrink),
                    button("Quit").on_press(Message::Quit),
                ].align_y(iced::alignment::Vertical::Bottom)
                
                // Space::new(Length::Shrink, Length::Fixed(16.)),
                // row![
                    
                //     Space::new(Length::Fill, Length::Shrink),
                //     button("Quit").on_press(Message::Quit),
                // ],
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
            Subscription::run(config::watch_config).map(Message::ConfigChanged),
        ])
    }
}

fn load_icon() -> Option<iced::window::Icon> {
    match iced::window::icon::from_file_data(resources::ICON_BYTES, None) {
        Ok(icon) => Some(icon),
        Err(e) => {
            log::warn!("Failed to load icon: {e}");
            None
        }
    }   
}
