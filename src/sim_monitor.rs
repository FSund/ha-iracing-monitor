use crate::iracing_client;

use anyhow::{Context, Result};
use chrono::Utc;
use iced::futures::channel::mpsc;
use iced::futures::StreamExt;
use iced::futures::{SinkExt, Stream};
use iced::stream;
use iracing_client::SimClient;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct SimMonitorState {
    connected: bool,
    // in_session: bool,
    current_session_type: String,
    // session_state: String,
    timestamp: String,
}

impl Default for SimMonitorState {
    fn default() -> Self {
        Self {
            connected: false,
            // in_session: false,
            current_session_type: "None".to_string(), // "None" registers as "Unknown" in Home Assistant
            // session_state: "None".to_string(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

pub struct SimMonitor {
    iracing: iracing_client::Client,
    mqtt: Option<AsyncClient>,
    last_state: Option<SimMonitorState>,
    mqtt_topic: String,
}

impl SimMonitor {
    pub fn new() -> Self {
        SimMonitor::new_with_config(None)
    }

    pub fn new_with_config(mqtt_client: Option<AsyncClient>) -> Self {
        Self {
            iracing: iracing_client::Client::new(),
            mqtt: mqtt_client,
            last_state: None,
            mqtt_topic: "homeassistant/sensor/iracing/state".to_string(),
        }
    }

    async fn update_mqtt_config(&mut self, mqtt_config: MqttConfig) {
        self.mqtt = {
            let mut mqtt_options =
                MqttOptions::new("iracing-monitor", mqtt_config.host, mqtt_config.port);
            mqtt_options.set_keep_alive(Duration::from_secs(5));
            mqtt_options.set_credentials(mqtt_config.user, mqtt_config.password);
            let (mqtt_client, mut mqtt_eventloop) = AsyncClient::new(mqtt_options, 10);

            // Start MQTT event loop
            tokio::spawn(async move {
                while let Ok(_notification) = mqtt_eventloop.poll().await {
                    // Handle MQTT events if needed
                }
            });
            log::info!("MQTT client set up.");
            Some(mqtt_client)
        };

        if let Some(mqtt) = self.mqtt.as_mut() {
            if register_device(mqtt).await.is_err() {
                log::warn!("Failed to register MQTT device");
            }
        }
    }

    async fn publish_state(&mut self, state: &SimMonitorState) -> Result<()> {
        if Some(state) != self.last_state.as_ref() {
            if let Some(mqtt) = self.mqtt.as_mut() {
                let payload = serde_json::to_string(&state)?;
                mqtt.publish(&self.mqtt_topic, QoS::AtLeastOnce, false, payload)
                    .await
                    .context("Failed to publish MQTT message")?;

                log::debug!("Published state: {:?}", state);
            } else {
                log::debug!("Unable to publish state, not connected to MQTT");
                // anyhow::bail!("No MQTT connection set up");
            }
            self.last_state = Some(state.clone());
        }
        Ok(())
    }

    async fn get_current_state(&mut self) -> SimMonitorState {
        // let connected = self.iracing.is_connected();
        match self.iracing.get_current_session_type().await {
            Some(session_type) => {
                log::debug!("Found session_type: {}", session_type);
                SimMonitorState {
                    connected: true,
                    current_session_type: session_type,
                    timestamp: Utc::now().to_rfc3339(),
                }
            }
            None => SimMonitorState {
                connected: false,
                current_session_type: "Disconnected".to_string(),
                timestamp: Utc::now().to_rfc3339(),
            },
        }
    }
}

async fn register_device(mqtt: &mut AsyncClient) -> Result<()> {
    // homeassistant/sensor/hp_1231232/config
    // <discovery_prefix>/<component>/[<node_id>/]<object_id>/config
    // Best practice for entities with a unique_id is to set <object_id> to unique_id and omit the <node_id>.

    let configuration_topic = "homeassistant/sensor/iracing/config";
    let config = serde_json::json!({
        "name": "Session type",
        "state_topic": "homeassistant/sensor/iracing/state",
        // "device_class": "timestamp",  // optional, will use "None: Generic sensor. (This is the default and doesn’t need to be set.)" if not set
        "value_template": "{{ value_json.current_session_type }}",
        "unique_id": "iracing_session_type",
        "expire_after": 30,
        "device": {
            "identifiers": "my_unique_id",
            "name": "iRacing Simulator",
        }
    });

    mqtt.publish(
        configuration_topic,
        QoS::AtLeastOnce,
        true, // retain flag set to true for discovery
        serde_json::to_string(&config)?,
    )
    .await
    .context("Failed to publish MQTT discovery configuration")?;

    log::info!("Registered device with Home Assistant.");
    Ok(())
}

// messages to SimMonitor
#[derive(Debug, Clone)]
pub enum Message {
    UpdateConfig(MqttConfig),
}

#[derive(Debug, Clone)]
pub struct Connection(mpsc::Sender<Message>);

impl Connection {
    pub fn send(&mut self, message: Message) {
        self.0.try_send(message).expect("Send message SimMonitor");
    }
}

// events from SimMonitor
#[derive(Debug, Clone)]
pub enum Event {
    Ready(Connection),
    ConnectedToSim,
    DisconnectedFromSim,
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Event::Ready(_) => write!(f, "Ready to connect to iRacing"),
            Event::Ready(_connection) => write!(f, "Ready"),
            Event::DisconnectedFromSim => write!(f, "iRacing Disconnected"),
            Event::ConnectedToSim => write!(f, "iRacing Connected"),
        }
    }
}

pub fn connect() -> impl Stream<Item = Event> {
    let mut monitor = SimMonitor::new();

    stream::channel(100, |mut output| async move {
        // Create channel
        let (sender, mut receiver) = mpsc::channel(100);

        // Send the sender back to the application
        // output.send(Event::Ready(sender)).await;
        output
            .send(Event::Ready(Connection(sender)))
            .await
            .expect("Unable to send");

        loop {
            tokio::select! {
                // Handle incoming messages
                Some(input) = receiver.next() => {
                    match input {
                        Message::UpdateConfig(mqtt_config) => {
                            log::info!("Updating mqtt config");
                            monitor.update_mqtt_config(mqtt_config).await;
                        }
                    }
                }
                // Handle the periodic state update
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    let state = monitor.get_current_state().await;
                    if let Err(e) = monitor.publish_state(&state).await {
                        log::warn!("Failed to publish state: {}", e);
                    }
                    if state.connected {
                        output.send(Event::ConnectedToSim).await.expect("Unable to send");
                    } else {
                        output.send(Event::DisconnectedFromSim).await.expect("Unable to send");
                    }
                }
            }
        }
    })
}
