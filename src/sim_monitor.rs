use crate::config;
use crate::config::AppConfig;
use crate::iracing_client;

use anyhow::{Context, Result};
use chrono::Utc;
use futures::channel::mpsc;
use futures::prelude::sink::SinkExt;
use futures::prelude::stream::StreamExt;
use futures::stream::Stream;
use iced::stream as iced_stream;
use iracing_client::SimClient;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::time::Duration;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Debug, Serialize, Clone, PartialEq, EnumIter)]
pub enum SessionType {
    // Unknown,
    Disconnected,
    Practice,
    Qualify,
    Race,
    LoneQualify,
    OfflineTesting,
}

impl Display for SessionType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            SessionType::Disconnected => write!(f, "Disconnected"),
            SessionType::Practice => write!(f, "Practice"),
            SessionType::Qualify => write!(f, "Qualify"),
            SessionType::Race => write!(f, "Race"),
            SessionType::LoneQualify => write!(f, "Lone Qualify"),
            SessionType::OfflineTesting => write!(f, "Offline Testing"),
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct SimMonitorState {
    pub connected: bool,
    // in_session: bool,
    pub current_session_type: SessionType,
    // session_state: String,
    pub timestamp: String,
}

impl Default for SimMonitorState {
    fn default() -> Self {
        Self {
            connected: false,
            // in_session: false,
            current_session_type: SessionType::Disconnected,
            // session_state: "None".to_string(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 1883,
            user: "".to_string(),
            password: "".to_string(),
        }
    }
}

pub struct SimMonitor {
    iracing: iracing_client::Client,
    mqtt: Option<AsyncClient>,
    last_state: Option<SimMonitorState>,
    mqtt_topic: String,

    mqtt_eventloop_handle: Option<tokio::task::JoinHandle<()>>,
    mqtt_eventloop: Option<rumqttc::EventLoop>,
}

impl SimMonitor {
    pub fn new(mqtt_config: Option<MqttConfig>) -> Self {
        let mut monitor = Self {
            iracing: iracing_client::Client::new(),
            mqtt: None,
            last_state: None,
            mqtt_topic: "homeassistant/sensor/iracing/state".to_string(),
            mqtt_eventloop_handle: None,
            mqtt_eventloop: None,
        };
        monitor.set_mqtt_config(mqtt_config);
        monitor
    }

    fn set_mqtt_config(&mut self, mqtt_config: Option<MqttConfig>) {
        // If we have an existing event loop, abort it before creating a new one
        if let Some(handle) = self.mqtt_eventloop_handle.take() {
            log::debug!("Aborting MQTT event loop");
            handle.abort();
        }

        let Some(mqtt_config) = mqtt_config else {
            log::debug!("Disabling MQTT");
            self.mqtt = None;
            return;
        };

        let mut mqtt_options =
            MqttOptions::new("iracing-monitor", mqtt_config.host, mqtt_config.port);
        mqtt_options.set_keep_alive(Duration::from_secs(5));
        mqtt_options.set_credentials(mqtt_config.user, mqtt_config.password);
        let (mqtt_client, mqtt_eventloop) = AsyncClient::new(mqtt_options, 10);

        // Store the client and event loop
        self.mqtt = Some(mqtt_client);
        self.mqtt_eventloop = Some(mqtt_eventloop); // Store the event loop without starting it yet
    }

    async fn start_mqtt_eventloop(&mut self) {
        if self.mqtt.is_none() {
            log::debug!("MQTT disabled, skipping event loop start");
            return;
        }

        if let Some(mut mqtt_eventloop) = self.mqtt_eventloop.take() {
            // Spawn and store the event loop handle
            log::debug!("Starting MQTT event loop");
            self.mqtt_eventloop_handle = Some(tokio::spawn(async move {
                loop {
                    match mqtt_eventloop.poll().await {
                        Ok(_notification) => {
                            // log::debug!("MQTT event: {:?}", notification);
                        }
                        Err(e) => {
                            // Just log the error but keep polling - the event loop will handle reconnection
                            // log::error!("MQTT error (will retry automatically): {:?}", e);
                            log::error!("MQTT error {e}");
                            tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;
                        }
                    }

                    // Small yield to prevent tight loop
                    // tokio::task::yield_now().await;

                    // TODO: is this the way?
                    // tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }));

            log::debug!("MQTT client set up.");

            // Register the device
            if let Some(mqtt) = self.mqtt.as_mut() {
                if let Err(e) = register_device(mqtt).await {
                    log::warn!("Failed to register MQTT device ({e})");
                }
                // Add a small delay to ensure registration is processed
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        } else {
            log::error!("Failed to start MQTT event loop, missing event loop");
        }
    }

    async fn publish_state(&mut self, state: &SimMonitorState) -> Result<()> {
        if Some(state) != self.last_state.as_ref() {
            if let Some(mqtt) = self.mqtt.as_mut() {
                let payload = serde_json::to_string(&state)?;
                let topic = self.mqtt_topic.clone();
                log::debug!(
                    "Attempting to publish to topic: {} with payload: {}",
                    &self.mqtt_topic,
                    &payload
                );

                // Spawn MQTT publish in separate task
                let mqtt_clone = mqtt.clone();
                let state_clone = state.clone();
                tokio::spawn(async move {
                    match tokio::time::timeout(
                        Duration::from_secs(5), // 5 second timeout
                        mqtt_clone.publish(&topic, QoS::AtLeastOnce, false, payload),
                    )
                    .await
                    {
                        Ok(result) => match result {
                            Ok(_) => {
                                // this isn't really true, it just means the connection
                                // hasn't timed out yet
                                log::debug!("Payload delivered to MQTT event loop");
                            }
                            Err(e) => {
                                log::warn!("Failed to publish state via MQTT: {}", e);
                            }
                        },
                        Err(_) => {
                            log::warn!("MQTT publish operation timed out after 5 seconds");
                        }
                    }
                });
                self.last_state = Some(state_clone);

                // Publish attributes
                // let attributes_json = serde_json::json!({
                //     // "icon_color": "#FF0000",
                //     // "color": "red",
                //     // "entity-color": "#FF0000",
                //     "icon": "mdi:racing-helmet",
                // });
                // let attributes_topic = "homeassistant/sensor/iracing/attributes";
                // let mqtt_clone = mqtt.clone();
                // tokio::spawn(async move {
                //     match mqtt_clone.publish(
                //         attributes_topic,
                //         QoS::AtLeastOnce,
                //         false,
                //         serde_json::to_string(&attributes_json).unwrap(),
                //     ).await {
                //         Ok(_) => {
                //             log::debug!("Successfully published attributes via MQTT");
                //         }
                //         Err(e) => {
                //             log::warn!("Failed to publish attributes via MQTT: {}", e);
                //         }
                //     }
                // });
            } else {
                log::debug!("Unable to publish state to MQTT, missing MQTT config");
            }
        }
        Ok(())
    }

    async fn get_current_state(&mut self) -> SimMonitorState {
        match self.iracing.get_current_session_type().await {
            Some(session_type) => {
                log::debug!("Found session_type: {}", session_type);

                // Convert the string to SessionType
                let session_type_enum = match session_type.as_str() {
                    "Practice" => SessionType::Practice,
                    "Qualify" => SessionType::Qualify,
                    "Race" => SessionType::Race,
                    "Lone Qualify" => SessionType::LoneQualify,
                    "Offline Testing" => SessionType::OfflineTesting,
                    unknown => {
                        log::warn!("Unknown session type received: {}", unknown);
                        SessionType::Disconnected
                    }
                };

                SimMonitorState {
                    connected: true,
                    current_session_type: session_type_enum,
                    timestamp: Utc::now().to_rfc3339(),
                }
            }
            None => SimMonitorState {
                connected: false,
                current_session_type: SessionType::Disconnected,
                timestamp: Utc::now().to_rfc3339(),
            },
        }
    }
    // // Add a cleanup method
    // pub async fn cleanup(&mut self) {
    //     if let Some(handle) = self.mqtt_eventloop_handle.take() {
    //         handle.abort();
    //     }
    //     if let Some(mqtt) = self.mqtt.take() {
    //         if let Err(e) = mqtt.disconnect().await {
    //             log::warn!("Error disconnecting MQTT client: {}", e);
    //         }
    //     }
    // }
}

impl Drop for SimMonitor {
    fn drop(&mut self) {
        if let Some(handle) = self.mqtt_eventloop_handle.take() {
            handle.abort();
        }
        // For MQTT client, just force close without trying to do a clean disconnect
        self.mqtt = None;
        log::info!("SimMonitor cleanup completed");
    }
}

async fn register_device(mqtt: &mut AsyncClient) -> Result<()> {
    // homeassistant/sensor/hp_1231232/config
    // <discovery_prefix>/<component>/[<node_id>/]<object_id>/config
    // Best practice for entities with a unique_id is to set <object_id> to unique_id and omit the <node_id>.

    let configuration_topic = "homeassistant/sensor/iracing/config";

    // Get all session types as strings
    let options: Vec<String> = SessionType::iter().map(|st| st.to_string()).collect();

    let config = serde_json::json!({
        "name": "Session type",
        "state_topic": "homeassistant/sensor/iracing/state",
        "value_template": "{{ value_json.current_session_type }}",
        "unique_id": "iracing_session_type",
        "expire_after": 30,
        "icon": "mdi:racing-helmet",
        "device_class": "enum",
        "options": options,
        "device": {
            "identifiers": "my_unique_id",
            "name": "iRacing Simulator",
        },
    });

    mqtt.publish(
        configuration_topic,
        QoS::AtLeastOnce,
        true,
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
    UpdateConfig(config::AppConfig),
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
    ConnectedToSim(SimMonitorState),
    DisconnectedFromSim(SimMonitorState),
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Event::Ready(_) => write!(f, "Ready to connect to iRacing"),
            Event::Ready(_connection) => write!(f, "Ready"),
            Event::DisconnectedFromSim(_) => write!(f, "iRacing Disconnected"),
            Event::ConnectedToSim(_) => write!(f, "iRacing Connected"),
        }
    }
}

pub fn connect(config: Option<AppConfig>) -> impl Stream<Item = Event> {
    // Create the monitor
    let mqtt_config = config.and_then(|c| if c.mqtt_enabled { Some(c.mqtt) } else { None });
    let mut monitor = SimMonitor::new(mqtt_config);

    iced_stream::channel(100, |mut output| async move {
        // Create channel
        let (sender, mut receiver) = mpsc::channel(100);

        // Start the MQTT event loop
        monitor.start_mqtt_eventloop().await;

        // Sim state update interval
        const UPDATE_INTERVAL: Duration = Duration::from_secs(1);
        let mut interval = tokio::time::interval(UPDATE_INTERVAL);

        // Get the initial state
        // let mut previous_state = monitor.get_current_state().await;
        let mut previous_state = SimMonitorState::default();

        // Send the sender back to the application
        output
            .send(Event::Ready(Connection(sender)))
            .await
            .expect("Unable to send");

        loop {
            tokio::select! {
                // Handle incoming messages
                Some(input) = receiver.next() => {
                    match input {
                        Message::UpdateConfig(config) => {
                            log::debug!("Received config update");
                            if config.mqtt_enabled {
                                log::info!("Updating mqtt config");
                                monitor.set_mqtt_config(Some(config.mqtt));
                                monitor.start_mqtt_eventloop().await;
                            } else if monitor.mqtt.is_some() {
                                log::info!("Disabling MQTT");
                                monitor.set_mqtt_config(None);
                            }
                        }
                    }
                }
                // Periodic state update
                _ = interval.tick() => {
                    let state = monitor.get_current_state().await;
                    log::debug!("Latest state: {:?}", state);
                    if let Err(e) = monitor.publish_state(&state).await {
                        log::warn!("Failed to publish state to MQTT: {}", e);
                    }

                    // Publish state event
                    let event = if state.connected {
                        Event::ConnectedToSim(state.clone())
                    } else {
                        Event::DisconnectedFromSim(state.clone())
                    };
                    if let Err(e) = output.send(event).await {
                        log::error!("Failed to send state event: {}", e);
                        // break; // Consider breaking the loop if we can't send events
                    }

                    // Check if the state has changed
                    if state.current_session_type != previous_state.current_session_type || state.connected != previous_state.connected {
                        log::info!("State changed, new state: {:?}", state);
                    }
                    previous_state = state;
                }
            }
        }
    })
}
