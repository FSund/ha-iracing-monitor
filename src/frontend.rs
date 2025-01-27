use iced::widget::{button, column, text, text_input, Column};
use iced::Task;

#[derive(Debug, Clone)]
pub enum Message {
    MqttHostChanged(String),
    MqttPortChanged(String),
    MqttUserChanged(String),
    MqttPasswordChanged(String),
    Connect,
}

pub struct IracingMonitorGui {
    mqtt_host: String,
    mqtt_port: String,
    mqtt_user: String,
    mqtt_password: String,
    connection_status: String,
}

impl IracingMonitorGui {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                mqtt_host: String::from("localhost"),
                mqtt_port: String::from("1883"),
                mqtt_user: String::new(),
                mqtt_password: String::new(),
                connection_status: String::from("Disconnected"),
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MqttHostChanged(value) => {
                self.mqtt_host = value;
            }
            Message::MqttPortChanged(value) => {
                self.mqtt_port = value;
            }
            Message::MqttUserChanged(value) => {
                self.mqtt_user = value;
            }
            Message::MqttPasswordChanged(value) => {
                self.mqtt_password = value;
            }
            Message::Connect => {
                // Here you would implement the actual connection logic
                self.connection_status = String::from("Connecting...");
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Column<Message> {
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
        ]
    }
}
