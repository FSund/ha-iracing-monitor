use std::default;

use crate::sim_monitor::{self, SimMonitor};

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
    SimState(sim_monitor::Event),
    Quit,
}

enum State {
    Disconnected,
    Connected(sim_monitor::Connection),
}


pub struct IracingMonitorGui {
    mqtt_host: String,
    mqtt_port: String,
    mqtt_user: String,
    mqtt_password: String,
    // connection_status: String,
    // iracing_connection_status: String,
    // sim_monitor: SimMonitor,
    state: State,
    port_is_valid: bool,
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
                // connection_status: String::from("Disconnected"),
                // iracing_connection_status: String::from("Disconnected"),
                // sim_monitor: SimMonitor::new(None),
                state: State::Disconnected,
                port_is_valid: true,
            },
            open.map(Message::WindowOpened),
        )
    }

    pub fn title(&self, window_id: iced::window::Id) -> String {
        // "IRacingMonitor - Iced".to_string()
        format!("IRacingMonitor {:?}", window_id)
    }

    // fn get_config_message(&self) -> sim_monitor::Message {
    //     if let Ok(mqtt_port) = self.mqtt_port.parse() { 
    //         // self.mqtt_port = val; 

    //         let mqtt_config = sim_monitor::MqttConfig {
    //             host: self.mqtt_host.clone(),
    //             port: mqtt_port,
    //             user: self.mqtt_user.clone(),
    //             password: self.mqtt_password.clone(),
    //         };
    //         sim_monitor::Message::UpdateConfig(mqtt_config)
    //     }
    // }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MqttHostChanged(value) => {
                self.mqtt_host = value;

                // let msg = self.get_config_message();
                // match &mut self.state {
                //     State::Connected(connection) => {
                //         connection.send(msg);
                //         Task::none()
                //     }
                //     State::Disconnected => Task::none()
                // }
                Task::none()
            }
            Message::MqttPortChanged(value) => {
                log::debug!("Port is {value}");
                if let Ok(_val) = value.parse::<u16>() {
                    // self.mqtt_port = Some(val);
                    self.port_is_valid = true;
                } else {
                    // self.mqtt_port = None;
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
                // Here you would implement the actual connection logic
                // self.connection_status = String::from("Connecting...");
                todo!("Connect not implemented yet");
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
                        self.state = State::Connected(connection);
                    }
                    sim_monitor::Event::Disconnected => {
                        self.state = State::Disconnected;
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
        // let content =
        let connect_button = if self.port_is_valid {
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
            // text(&self.connection_status).size(16),
            // text(&self.iracing_connection_status).size(16),
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
            // Subscription::run(sim_monitor::run_the_stuff).map(Message::SimState),
            Subscription::run(sim_monitor::connect).map(Message::SimState),
        ])
    }
}
