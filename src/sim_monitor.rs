use crate::config::AppConfig;
use crate::iracing_client;
use crate::config;

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
use std::time::Duration;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct SimMonitorState {
    pub connected: bool,
    // in_session: bool,
    pub current_session_type: String,
    // session_state: String,
    pub timestamp: String,
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
}

impl SimMonitor {
    pub fn new() -> Self {
        SimMonitor::new_with_mqtt_client(None)
    }

    pub fn new_with_mqtt_client(mqtt_client: Option<AsyncClient>) -> Self {
        Self {
            iracing: iracing_client::Client::new(),
            mqtt: mqtt_client,
            last_state: None,
            mqtt_topic: "homeassistant/sensor/iracing/state".to_string(),
            mqtt_eventloop_handle: None,
        }
    }

    async fn update_mqtt_config(&mut self, mqtt_config: MqttConfig) {
        // If we have an existing event loop, abort it before creating a new one
        if let Some(handle) = self.mqtt_eventloop_handle.take() {
            log::debug!("Aborting MQTT event loop");
            handle.abort();
        }

        let mut mqtt_options =
            MqttOptions::new("iracing-monitor", mqtt_config.host, mqtt_config.port);
        mqtt_options.set_keep_alive(Duration::from_secs(5));
        mqtt_options.set_credentials(mqtt_config.user, mqtt_config.password);
        let (mqtt_client, mut mqtt_eventloop) = AsyncClient::new(mqtt_options, 10);

        // Store the client
        self.mqtt = Some(mqtt_client);

        // Spawn and store the event loop handle
        log::info!("Starting MQTT event loop");
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

        log::info!("MQTT client set up.");

        // Register the device
        if let Some(mqtt) = self.mqtt.as_mut() {
            if let Err(e) = register_device(mqtt).await {
                log::warn!("Failed to register MQTT device ({e})");
            }
            // Add a small delay to ensure registration is processed
            tokio::time::sleep(Duration::from_secs(1)).await;
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
                        mqtt_clone.publish(&topic, QoS::AtLeastOnce, false, payload)
                    ).await {
                        Ok(result) => match result {
                            Ok(_) => {
                                // this isn't really true, it just means the connection
                                // hasn't timed out yet
                                log::debug!("Successfully published state via MQTT");
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
        // Disconnect MQTT client if it exists
        if let Some(mqtt) = self.mqtt.as_mut() {
            // Use blocking disconnect since we're in drop
            if let Err(e) = futures::executor::block_on(mqtt.disconnect()) {
                log::warn!("Error disconnecting MQTT client during cleanup: {}", e);
            }
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
        "icon": "mdi:racing-helmet",
        "device_class": "enum",
        "options": [
            "Disconnected",
            "Practice",
            "Qualifying",
            "Race",
            "Lone Qualifying",
        ],
        "device": {
            "identifiers": "my_unique_id",
            "name": "iRacing Simulator",
        },
        // "json_attributes_topic": "homeassistant/sensor/iracing/attributes",
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
    let mut monitor = SimMonitor::new();

    iced_stream::channel(100, |mut output| async move {
        if let Some(config) = config {
            monitor.update_mqtt_config(config.mqtt).await;
        }
        
        // Create channel
        let (sender, mut receiver) = mpsc::channel(100);

        // Sim state update interval
        const UPDATE_INTERVAL: Duration = Duration::from_secs(1);
        let mut interval = tokio::time::interval(UPDATE_INTERVAL);

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
                                monitor.update_mqtt_config(config.mqtt).await;
                            } else if monitor.mqtt.is_some() {
                                log::info!("Disabling MQTT");
                                if let Some(handle) = monitor.mqtt_eventloop_handle.take() {
                                    handle.abort();
                                }
                                monitor.mqtt = None;
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
                        Event::ConnectedToSim(state)
                    } else {
                        Event::DisconnectedFromSim(state)
                    };
                    if let Err(e) = output.send(event).await {
                        log::error!("Failed to send state event: {}", e);
                        // break; // Consider breaking the loop if we can't send events
                    }
                }
            }
        }
    })
}
