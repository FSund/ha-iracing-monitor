use std::default;

use crate::monitor;

use iced::widget::{button, column, text, text_input, Column};
use iced::window;
use iced::{Center, Element, Fill, Subscription, Task, Theme, Vector};
use iced::keyboard;

#[derive(Debug, Clone)]
pub enum Message {
    MqttHostChanged(String),
    MqttPortChanged(String),
    MqttUserChanged(String),
    MqttPasswordChanged(String),
    Connect,
    WindowOpened(window::Id),
    WindowClosed(window::Id),
    SimState(monitor::Event),
    Quit,
}

pub struct IracingMonitorGui {
    mqtt_host: String,
    mqtt_port: String,
    mqtt_user: String,
    mqtt_password: String,
    connection_status: String,
    iracing_connection_status: String,
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
                connection_status: String::from("Disconnected"),
                iracing_connection_status: String::from("Disconnected"),
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
                // Here you would implement the actual connection logic
                self.connection_status = String::from("Connecting...");
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
                self.iracing_connection_status = format!("{event}");
                Task::none()
            }
            Message::Quit => {
                log::info!("Quit application!");
                iced::exit()
            }
        }
    }

    pub fn view(&self, _window_id: iced::window::Id) -> Column<Message> {
        // let content =
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
            button("Connect").padding(10).on_press(Message::Connect),
            text(&self.connection_status).size(16),
            text(&self.iracing_connection_status).size(16),
        ]
    }

    pub fn subscription(&self) -> Subscription<Message> {
        fn handle_hotkey(
            key: keyboard::Key,
            modifiers: keyboard::Modifiers,
        ) -> Option<Message> {
            // use keyboard::key;

            match (key.as_ref(), modifiers) {
                // quit on Ctrl+Q
                (keyboard::Key::Character("q"), modifiers) if modifiers.control() => Some(Message::Quit),
                _ => None,
            }
        }

        Subscription::batch(vec![
            window::close_events().map(Message::WindowClosed),
            keyboard::on_key_press(handle_hotkey),
            Subscription::run(monitor::run_the_stuff).map(Message::SimState),
        ])
    }
}
