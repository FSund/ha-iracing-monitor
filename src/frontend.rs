use crate::sim_monitor;

use iced::keyboard;
use iced::widget::{button, column, text, text_input, Column};
use iced::window;
use iced::{Subscription, Task};

#[derive(Debug, Clone)]
pub enum Message {
    MqttHostChanged(String),
    MqttPortChanged(String),
    MqttUserChanged(String),
    MqttPasswordChanged(String),
    Connect,
    WindowOpened(window::Id),
    WindowClosed(window::Id),
    SimState(sim_monitor::Event),
    Quit,
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
    mqtt_host: String,
    mqtt_port: String,
    mqtt_user: String,
    mqtt_password: String,
    state: State,
    port_is_valid: bool,
    connected_to_sim: bool,
}

impl IracingMonitorGui {
    pub fn new() -> (Self, Task<Message>) {
        let (_id, open) = window::open(window::Settings::default());

        (
            Self {
                mqtt_host: String::from("localhost"),
                mqtt_port: String::from("1883"),
                mqtt_user: String::new(),
                mqtt_password: String::new(),
                state: State::WaitingForBackendConnection,
                port_is_valid: true,
                connected_to_sim: false,
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
            Message::Connect => {
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
                log::info!("Window closed, closing application!");
                iced::exit()
            }
            Message::SimState(event) => {
                log::debug!("SimState event received! ({event})");
                // self.iracing_connection_status = format!("{event}");

                match event {
                    sim_monitor::Event::Ready(connection) => {
                        self.state = State::ConnectedToBackend(connection);
                        log::info!("Backend ready, waiting for game");
                    }
                    sim_monitor::Event::DisconnectedFromSim => {
                        if self.connected_to_sim {
                            log::info!("Disconnected from game");
                            self.connected_to_sim = false;
                        }
                    }
                    sim_monitor::Event::ConnectedToSim => {
                        if !self.connected_to_sim {
                            log::info!("Connected to game");
                            self.connected_to_sim = true;
                        }
                    }
                }
                Task::none()
            }
            Message::Quit => {
                log::info!("Quit application!");
                iced::exit()
            }
        }
    }

    pub fn view(&self, _window_id: iced::window::Id) -> Column<Message> {
        let connect_button =
            if self.port_is_valid && matches!(self.state, State::ConnectedToBackend(_)) {
                button("Connect").padding(10).on_press(Message::Connect)
            } else {
                button("Connect").padding(10)
            };
        column![
            text("iRacing Home Assistant Monitor").size(28),
            text_input("MQTT Host", &self.mqtt_host)
                .on_input(Message::MqttHostChanged)
                .padding(10)
                .size(20),
            text_input("MQTT Port", &self.mqtt_port)
                .on_input(Message::MqttPortChanged)
                .padding(10)
                .size(20),
            text_input("MQTT User", &self.mqtt_user)
                .on_input(Message::MqttUserChanged)
                .padding(10)
                .size(20),
            text_input("MQTT Password", &self.mqtt_password)
                .on_input(Message::MqttPasswordChanged)
                .padding(10)
                .size(20)
                .secure(true),
            connect_button,
            text(self.state.to_string()).size(16)
        ]
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
            Subscription::run(sim_monitor::connect).map(Message::SimState),
        ])
    }
}
